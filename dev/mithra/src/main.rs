use clap::{Arg, ArgMatches, App, Error};
use configparser::ini::Ini;
use hostname_validator;
use postgres::Config;
use rumqttc::MqttOptions;
use std::{collections::{HashSet, HashMap}, hash::Hash, str::FromStr};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

mod system;
use system::*;

fn main() {
    let params = get_cli_parameters().unwrap_or_else(
        |e| e.exit()
    );
    let servers_file = params.value_of("servers").unwrap();
    let config_file = params.value_of("config").unwrap();

    let mut servers_config = Ini::new();
    servers_config.load(servers_file).unwrap_or_else(|error_msg| {
        eprintln!("{}", error_msg);
        std::process::exit(1);
    });

    let db_config = get_db_config(&servers_config).unwrap_or_else(|error_msg| {
        eprintln!("{}", error_msg);
        std::process::exit(1);
    });

    let mqtt_config = get_mqtt_config(&servers_config).unwrap_or_else(|error_msg| {
        eprintln!("{}", error_msg);
        std::process::exit(1);
    });

    let system_config = load_yaml_config(config_file).unwrap_or_else(|error_msg| {
        eprintln!("{}", error_msg);
        std::process::exit(1);
    });

    println!("{:?}", system_config);
    

}

fn get_cli_parameters() -> Result<ArgMatches<'static>, Error> {
    fn validate_file(path: String) -> Result<(), String> {
        if Path::new(path.as_str()).exists() {
            Ok(())
        } else {
            Err(format!("File '{}' does not exist!", path))
        }
    }

    App::new("Mithra system")
        .version("0.0.1")
        .author("Krzysztof Juszczyk")
        .about("Profitable solar energy utilization system")
        .arg(Arg::with_name("servers")
            .short("s")
            .long("servers")
            .value_name("FILE")
            .help("Sets path to configuration file with PostagreSQL and MQTT server credentials")
            .takes_value(true)
            .required(true)
            .validator(validate_file)
        )
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Sets path to configuration yaml file with guards list and measuments points")
            .takes_value(true)
            .required(true)
            .validator(validate_file)
        )
    .get_matches_safe()
    
}

fn get_db_config(params: &Ini) -> Result<Config, &str> {
    let host = if let Some(value) = params.get("Database", "Host") {
        if hostname_validator::is_valid(&value) {
            value
        } else {
            return Err("Database host link invalid!");
        }
    } else {
        return Err("Database host configuration missing!");
    };

    let port = if let Ok(Some(value)) = params.getint("Database", "Port") {
        if let Ok(value) = u16::try_from(value) {
            value
        } else {
            return Err("Database port configuration improper value");
        }
    } else {
        return Err("Database port configuration invalid!");
    };

    let db_name = if let Some(value) = params.get("Database", "DatabaseName") {
        value
    } else {
        return Err("Database name configuration missing");
    };

    let user = if let Some(value) = params.get("Database", "User") {
        value
    } else {
        return Err("Database user configuration missing");
    };

    let password = if let Some(value) = params.get("Database", "Password") {
        value
    } else {
        return Err("Database password configuration missing");
    };

    let mut config = Config::new();
    config.host(&host);
    config.port(port);
    config.dbname(&db_name);
    config.user(&user);
    config.password(&password);
    
    return Ok(config);
}

fn get_mqtt_config(params: &Ini) -> Result<MqttOptions, &str> {
    let host = if let Some(value) = params.get("Mqtt_server", "Host") {
        if hostname_validator::is_valid(&value)  {
            value
        } else {
            return Err("MQTT host link invalid!");
        }
    } else {
        return Err("MQTT host configuration missing!");
    };

    let port = if let Ok(Some(value)) = params.getint("Mqtt_server", "Port") {
        if let Ok(value) = u16::try_from(value) {
            value
        } else {
            return Err("MQTT port configuration improper value");
        }
    } else {
        return Err("MQTT port configuration invalid!");
    };

    let user = if let Some(value) = params.get("Mqtt_server", "User") {
        value
    } else {
        return Err("MQTT user configuration missing");
    };

    let password = if let Some(value) = params.get("Mqtt_server", "Password") {
        value
    } else {
        return Err("MQTT password configuration missing");
    };

    let mut mqtt_options = MqttOptions::new("Mithra_mqtt_client", host, port);
    mqtt_options.set_credentials(user, password);
    mqtt_options.set_keep_alive(60);

    return Ok(mqtt_options);
} 


fn load_yaml_config(path: &str) -> Result<System, &str> {
    let content = if let Ok(mut file) = File::open(path) {
        let mut content = String::new();
        if let Ok(_) = file.read_to_string(&mut content) {
            content
        } else {
            return Err("Reading file error!");
        }
    } else {
        return Err("Cannot open yaml configuration file!");
    };

    let docs = if let Ok(docs) = YamlLoader::load_from_str(&content) {
        docs
    } else {
        return Err("Parsing yaml error!")
    };

    let doc = &docs[0];

    if let Some(system) = parse_yaml(doc) {
        Ok(system)
    } else {
        Err("Not valid yaml configuration!")
    }
}

fn parse_yaml(conf: &Yaml) -> Option<System> {
    /* Checking switchboard */
    if let Yaml::BadValue = conf["switchboard"] {
        return None;
    }

    if let Yaml::BadValue = conf["switchboard"]["name"] {
        return None;
    }

    /* Checking each guard */
    if let Yaml::BadValue = conf["guards"] {
        return None;
    } 

    /* Checking guards */
    let guards = if let Some(array) = conf["guards"].as_vec() {
        array
    } else {
        return None
    };

    let mut system = System {
        guards: HashMap::new(),
        miners: HashMap::new(),
    };

    let mut guard_names = HashSet::new();
    let mut plug_names = HashSet::new();
    let mut miner_names = HashSet::new();

    for guard in guards {
        let guard_name = if let Some(guard_name) = guard["name"].as_str() {
            /* Guards name are uniqe */
            if guard_names.contains(guard_name) {
                return None;
            }
            guard_names.insert(guard_name);
            guard_name
        } else {
            return None;
        };

        /* Check guard type is supported */
        let guard_type = if let Some(board_name) = guard["type"].as_str() {
            if let Ok(guard_type) = GuardType::from_str(board_name) {
                guard_type
            } else {
                return None;
            }
        } else {
            return None;
        };

        let pinout_limit = guard_type.get_pinout_limit();

        let miners = if let Some(array) = guard["miners"].as_vec() {
            array
        } else {
            return None;
        };

        let mut local_guard = Guard {
            name: String::from(guard_name),
            miners: Vec::new(),
            board_type: guard_type,
        };
        /* Check all miners under this guard */
        for miner in miners {
            let mut pinouts = HashSet::new();
            
            let miner_name = if let Some(miner_name) = miner["name"].as_str() {
                /* Miner names are uniqe */
                if miner_names.contains(miner_name) {
                    return None;
                }
                miner_names.insert(miner_name);
                miner_name
            } else {
                return None;
            };

            let miner_pinout = if let Some(pinout) = miner["pinout"].as_i64() {
                if pinouts.contains(&pinout) || pinout < 0 || pinout as u32 >= pinout_limit {
                    return None;
                }
                pinouts.insert(pinout);
                pinout
            } else {
                return None;
            };

            let plug_name = if let Some(plug_name) = miner["plug"].as_str() {
                if plug_names.contains(plug_name) {
                    return None;
                }
                plug_names.insert(plug_name);
                plug_name
            } else {
                return None;
            };

            local_guard.miners.push(String::from(miner_name));
            system.miners.insert(
                String::from(miner_name),
                Miner {
                    name: String::from(miner_name),
                    guard: String::from(guard_name),
                    plug: String::from(plug_name),
                    pinout: miner_pinout as u32,
                    power_consumption: None,
                }    
            );
        }

        system.guards.insert(String::from(guard_name), local_guard);
    }

    Some(system)
}