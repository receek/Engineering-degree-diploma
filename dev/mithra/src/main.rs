use clap::{Arg, ArgMatches, App, Error};
use configparser::ini::Ini;
use hostname_validator;
use postgres::Config;
use rumqttc::MqttOptions;
use std::path::Path;

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
