#![feature(deadline_api)]
#![feature(thread_is_running)]

use chrono::naive::MIN_DATETIME;
use clap::{Arg, ArgMatches, App, Error};
use configparser::ini::Ini;
use hostname_validator;
use postgres::Config;
use std::{
    collections::{HashSet, HashMap},
    fs::File,
    io::Read,
    path::Path,
    str::FromStr
};
use yaml_rust::{Yaml, YamlLoader};

mod system;
use system::{
    MqttConfig,
    System,
    structs::*

};


fn main() {
    let params = get_cli_parameters().unwrap_or_else(
        |e| e.exit()
    );

    let servers_file = params.value_of("servers").unwrap();
    let config_file = params.value_of("config").unwrap();

    loop {
        let mut servers_config = Ini::new();
        servers_config.load(servers_file).unwrap_or_else(|error_msg| {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        });
        
        let (start_year,start_month,billing_period, recovery_ratio) = get_contract_data(&servers_config).unwrap_or_else(|error_msg| {
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
    
        let (switchboard, guards, miners, plugs) = match load_yaml_config(config_file) {
            Ok(devices) => devices,
            Err(error_msg) => {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            }
        };
    
        let mut system = System {
            start_year,
            start_month,
            billing_period,
            recovery_ratio,
            db_config,
            mqtt_config,
            switchboard,
            guards,
            miners,
            plugs, 
        };
    
        /* Try to connect with database */
        if let Err(error_msg) = system.check_servers_connection() {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        }
        
        system.run();
    }
    
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
            .help("Sets path to configuration file with PostagreSQL, MQTT servers credentials and energy contract data")
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

fn get_contract_data(params: &Ini) -> Result<(u32, u32, u32, f64), &str> {
    let start_year = if let Ok(Some(value)) = params.getint("Contract", "YearStart") {
        if let Ok(value) = u32::try_from(value) {
            value
        } else {
            return Err("Contract start year improper value!");
        }
    } else {
        return Err("Contract start year configuration invalid!");
    };

    let start_month = if let Ok(Some(value)) = params.getint("Contract", "MonthStart") {
        if let Ok(value) = u32::try_from(value) {
            value
        } else {
            return Err("Contract start year improper value!");
        }
    } else {
        return Err("Contract start year configuration invalid!");
    };

    let billing_period_months = if let Ok(Some(value)) = params.getint("Contract", "BillingPeriod") {
        if let Ok(value) = u32::try_from(value) {
            value
        } else {
            return Err("Billing data improper value!");
        }
    } else {
        return Err("Billing data configuration invalid!");
    };

    let recovery_ratio = if let Ok(Some(value)) = params.getfloat("Contract", "RecoveryRatio") {
        if 0.0 <= value && value <= 1.0 {
            value
        } else {
            return Err("Recovery ratio improper value!");
        }
    } else {
        return Err("Recovery ratio configuration invalid!");
    };
    
    return Ok((start_year, start_month, billing_period_months, recovery_ratio));
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

fn get_mqtt_config(params: &Ini) -> Result<MqttConfig, &str> {
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

    return Ok(MqttConfig {
        host,
        port,
        user,
        password,
    });
} 


fn load_yaml_config(path: &str) -> 
Result<
    (
        Switchboard,
        HashMap<String, Guard>,
        HashMap<String, Miner>,
        HashMap<String, Plug>
    ),
    &str
> {
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

    if let Some(devices) = parse_yaml(doc) {
        Ok(devices)
    } else {
        Err("Not valid yaml configuration!")
    }
}

fn parse_yaml(conf: &Yaml) 
-> Option<(
    Switchboard,
    HashMap<String, Guard>,
    HashMap<String, Miner>,
    HashMap<String, Plug>
)> {
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
    let guards_array = if let Some(array) = conf["guards"].as_vec() {
        array
    } else {
        return None
    };

    let switchboard = Switchboard {
        id: String::from(switchboard_id),
        state: DeviceState::Inaccessible,
        last_seen: MIN_DATETIME,
    };

    let mut guards = HashMap::new();
    let mut miners = HashMap::new();
    let mut plugs = HashMap::new();

    for guard in guards_array {
        let guard_id = if let Some(guard_id) = guard["id"].as_str() {
            /* Guards id are uniqe */
            if guards.contains_key(guard_id) {
                return None;
            }
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

        let miners_array = if let Some(array) = guard["miners"].as_vec() {
            array
        } else {
            return None;
        };

        let mut local_guard = Guard {
            id: String::from(guard_id),
            miners: Vec::new(),
            board_type: guard_type,
            state: DeviceState::Inaccessible,
            last_seen: MIN_DATETIME,
        };
        /* Check all miners under this guard */
        for miner in miners_array {
            let mut pinsets = HashSet::new();
            
            let miner_id = if let Some(miner_id) = miner["id"].as_str() {
                /* Miner ids are uniqe */
                if miners.contains_key(miner_id) {
                    return None;
                }
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
                if plugs.contains_key(plug_id) {
                    return None;
                }
                plug_id
            } else {
                return None;
            };

            let phase = if let Some(phase) = miner["phase"].as_i64() {
                if phase < 0 || 2 < phase {
                    return None;
                }
                phase as u8
            } else {
                return None;
            };

            let consumption = if let Some(consumption) = miner["consumption"].as_i64() {
                if consumption < 0 {
                    return None;
                }
                consumption as u32
            } else {
                return None;
            };

            local_guard.miners.push(String::from(miner_id));
            miners.insert(
                String::from(miner_id),
                Miner {
                    id: String::from(miner_id),
                    guard: String::from(guard_id),
                    plug_id: String::from(plug_id),
                    pinset: miner_pinset as u32,
                    phase: phase,
                    estimated_consumption: consumption as f32,
                    power_consumption: None,
                    state: MinerState::Undefined,
                    target_state: None,
                    command_ts: None,
                    included: true,
                }    
            );
            plugs.insert(
                String::from(plug_id),
                Plug {
                    id: String::from(plug_id),
                    state: DeviceState::Inaccessible,
                    miner_id: String::from(miner_id),
                    is_enabled: true,
                    last_seen: MIN_DATETIME,
                }
            );
        }

        guards.insert(String::from(guard_id), local_guard);
    }

    Some((switchboard, guards, miners, plugs))
}