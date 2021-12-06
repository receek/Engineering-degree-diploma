use clap::{Arg, ArgMatches, App};
use configparser::ini::Ini;
use std::path::Path;

fn main() {
    let params = get_cli_parameters();
    let servers_file = params.value_of("servers").unwrap();
    let config_file = params.value_of("config").unwrap();
}

fn get_cli_parameters() -> ArgMatches<'static> {
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
    .unwrap_or_else(|e| e.exit())
}

