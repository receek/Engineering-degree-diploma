use clap::{Arg, ArgMatches, App, Error};
use configparser::ini::Ini;
use hostname_validator;
use postgres::{Config, NoTls};
//use r2d2::ManageConnection;
use rumqttc::{Client, MqttOptions};
use std::{collections::{HashSet, HashMap}, str::FromStr};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use yaml_rust::{Yaml, YamlLoader};

mod system;
use system::*;

// mod database;
// use database::*;

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

    let mut system_config = load_yaml_config(config_file).unwrap_or_else(|error_msg| {
        eprintln!("{}", error_msg);
        std::process::exit(1);
    });

    // println!("{:?}", system_config);
    
    /* Config is validated and loaded */

    /* Try to connect with database */
    match db_config.connect(NoTls) {
        Ok(conn) => {
            if let Err(error) = conn.close() {
                eprintln!("Database closing test connection: {}", error.to_string());
                std::process::exit(1)    
            }
        },
        Err(error) => {
            eprintln!("Database test connection: {}", error.to_string());
            std::process::exit(1)
        },
    }

    /* Try to connect with MQTT server */
    let (mut client, mut connection) = Client::new(mqtt_config.clone(), 1);
    let msg = connection.iter().next();
    if let Some(Err(error)) = msg {
        eprintln!("MQTT server test connection: {}", error.to_string());
        std::process::exit(1);
    }
    
    if let Err(error ) = client.disconnect() {
        eprintln!("MQTT server closing test connection: {}", error.to_string());
        std::process::exit(1);
    }

    /* Start system */
    system_config.init(mqtt_config.clone());

    /* Run MQTT switchboard and plugs messages handler */

    /* Run MQTT miners command handler and executor */

    /* Run power consumption analyzer */
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

    if let Yaml::BadValue = conf["switchboard"]["id"] {
        return None;
    }
    let switchboard_id = conf["switchboard"]["id"].as_str().unwrap();

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
        switchboard: Switchboard {
            id: String::from(switchboard_id),
            state: DeviceState::Unknown,
        },
        guards: HashMap::new(),
        miners: HashMap::new(),
        plugs: HashMap::new(),
    };

    let mut guard_ids = HashSet::new();
    let mut plug_ids = HashSet::new();
    let mut miner_ids = HashSet::new();

    for guard in guards {
        let guard_id = if let Some(guard_id) = guard["id"].as_str() {
            /* Guards id are uniqe */
            if guard_ids.contains(guard_id) {
                return None;
            }
            guard_ids.insert(guard_id);
            guard_id
        } else {
            return None;
        };

        /* Check guard type is supported */
        let guard_type = if let Some(board_id) = guard["type"].as_str() {
            if let Ok(guard_type) = GuardType::from_str(board_id) {
                guard_type
            } else {
                return None;
            }
        } else {
            return None;
        };

        let pinset_limit = guard_type.get_pinset_limit();

        let miners = if let Some(array) = guard["miners"].as_vec() {
            array
        } else {
            return None;
        };

        let mut local_guard = Guard {
            id: String::from(guard_id),
            miners: Vec::new(),
            board_type: guard_type,
            state: DeviceState::Available,
        };
        /* Check all miners under this guard */
        for miner in miners {
            let mut pinsets = HashSet::new();
            
            let miner_id = if let Some(miner_id) = miner["id"].as_str() {
                /* Miner ids are uniqe */
                if miner_ids.contains(miner_id) {
                    return None;
                }
                miner_ids.insert(miner_id);
                miner_id
            } else {
                return None;
            };

            let miner_pinset = if let Some(pinset) = miner["pinset"].as_i64() {
                if pinsets.contains(&pinset) || pinset < 0 || pinset as u32 >= pinset_limit {
                    return None;
                }
                pinsets.insert(pinset);
                pinset
            } else {
                return None;
            };

            let plug_id = if let Some(plug_id) = miner["plug"].as_str() {
                if plug_ids.contains(plug_id) {
                    return None;
                }
                plug_ids.insert(plug_id);
                plug_id
            } else {
                return None;
            };

            local_guard.miners.push(String::from(miner_id));
            system.miners.insert(
                String::from(miner_id),
                Miner {
                    id: String::from(miner_id),
                    guard: String::from(guard_id),
                    plug: String::from(plug_id),
                    pinset: miner_pinset as u32,
                    power_consumption: None,
                }    
            );
            system.plugs.insert(
                String::from(plug_id),
                Plug {
                    id: String::from(plug_id),
                    state: DeviceState::Unknown,
                }
            );
        }

        system.guards.insert(String::from(guard_id), local_guard);
    }

    Some(system)
}