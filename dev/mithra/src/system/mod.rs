use chrono::{NaiveDate, NaiveDateTime, Utc, Datelike};

use json::JsonValue;

use postgres::{Config, NoTls};

use r2d2::State;

use rumqttc::{Client, Event, MqttOptions, Packet, QoS, Publish};

use sscanf::scanf;

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;

mod database;

mod handlers;

pub mod structs;
use structs::*;

#[derive(Debug)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Debug)]
pub struct System {
    /* Contract data */
    pub start_year: u32,
    pub start_month: u32,
    pub billing_period: u32,
    pub recovery_ratio: f64,

    /* Postgres configuration */
    pub db_config: Config,
    
    /* MQTT configuration */
    pub mqtt_config: MqttConfig,

    /* Devices in system */
    pub switchboard: Switchboard,
    pub guards: HashMap<String, Guard>,
    pub miners: HashMap<String, Miner>,
    pub plugs: HashMap<String, Plug>,
}

impl System {

pub fn check_servers_connection(&self) -> Result<(), &'static str> {
    let mut db_client = match self.db_config.connect(NoTls) {
        Ok(conn) => conn,
        Err(error) => {
            eprintln!("Database test connection: {}", error.to_string());
            std::process::exit(1)
        },
    };

    /* Try to connect with MQTT server */
    let mut mqtt_options = self.get_mqtt_options("Test_connection");
    mqtt_options.set_keep_alive(60);

    let (mut client, mut connection) = Client::new(mqtt_options, 1);
    let msg = connection.iter().next();
    if let Some(Err(error)) = msg {
        eprintln!("MQTT server test connection: {}", error.to_string());
        std::process::exit(1);
    }
    
    if let Err(error ) = client.disconnect() {
        eprintln!("MQTT server closing test connection: {}", error.to_string());
        std::process::exit(1);
    }

    return Ok(());
}

pub fn init(&mut self) {
    let mut mqtt_options = self.get_mqtt_options("Announce_loop");
    mqtt_options.set_keep_alive(5);

    let (mut client, mut connection) = Client::new(mqtt_options, 1024);
    client.subscribe("shellies/announce", QoS::AtMostOnce).unwrap();
    client.subscribe("guards/announce", QoS::AtMostOnce).unwrap();

    if let Err(error_msg) = client.publish(
        "shellies/command",
        QoS::AtMostOnce,
        false,
        "announce".as_bytes()
    ) {
        eprintln!("Shellies annouce command error: {}", error_msg);
        std::process::exit(1);
    }

    if let Err(error_msg) = client.publish(
        "guards/command",
        QoS::AtMostOnce,
        false,
        "announce".as_bytes()
    ) {
        eprintln!("Guards annouce command error: {}", error_msg);
        std::process::exit(1);
    }

    let timer = Instant::now();
    let time = Duration::from_secs(5); // TODO: CHANGE to not less than 30s.
    println!("Announce message sent");
    for msg in connection.iter() {
        if timer.elapsed() > time {
            client.disconnect().unwrap();
            break;
        }

        match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                self.parse_announce(&data);
            },
            Ok(_) => (), 
            Err(_) => (),
        }
    }

    if self.switchboard.state != DeviceState::Available {
        eprintln!("Switchboard is not available!");
        std::process::exit(1);
    }

    let mut db_client = match self.db_config.connect(NoTls) {
        Ok(conn) => conn,
        Err(error) => {
            eprintln!("Database test connection: {}", error.to_string());
            std::process::exit(1)
        },
    };

    /* Check database schema and create needed tables eventually */
    let period = current_biling_period(self.start_year as i32, self.start_month, self.billing_period);
    println!("Checking period from {:?} to {:?}", period.0, period.1);
    database::check_db_schema(&mut db_client, period);

    /* Obtaining power state of switchboard */
    let switchboard_params = if let Some(params) = database::get_switchboard_params(&mut db_client, period.0) {
        println!("Switchboard data obtained from database.");
        params
    } else {
        println!("Switchboard data obtained from MQTT server.");
        self.get_switchboard_data()
    };

    /* Obtaining energy consumed by miners */
    let miners_consumption = database::get_miners_consumption(&mut db_client, period.0);
    println!("Miners have consumed {:?} watt-minute until now.", miners_consumption);
}

pub fn run(&mut self) {
    // let (main_tx, main_rx) = mpsc::channel();
    let (db_tx, db_rx) = mpsc::channel();

    /* Run database thread */
    let db_thread = {
        let db_config = self.db_config.clone();
        thread::spawn(|| database::insert_energy_data(db_config, db_rx))
    };

    /* Run switchboard MQTT messages receiver */
    let mut mqtt_options = self.get_mqtt_options("Switchboard_loop");
    mqtt_options.set_keep_alive(60);

    let (mut switchboard_mqtt, switchboard_connection) = Client::new(mqtt_options, 1024);

    let switchboard_id = &self.switchboard.id;
    let topics = vec![
        format!("shellies/{}/emeter/+/energy", switchboard_id),
        format!("shellies/{}/emeter/+/returned_energy", switchboard_id),
        format!("shellies/{}/emeter/+/total", switchboard_id),
        format!("shellies/{}/emeter/+/total_returned", switchboard_id),
    ];

    for topic in topics {
        switchboard_mqtt.subscribe(topic, QoS::AtMostOnce).unwrap();
    }

    let switchboard_thread = {
        let db_tx = db_tx.clone();
        thread::spawn(|| handlers::switchboard_loop(switchboard_connection, db_tx))
    };
    println!("Switchboard loop spawned");

    /* Run miners MQTT messages receiver */
    let mut mqtt_options = self.get_mqtt_options("Miners_loop");
    mqtt_options.set_keep_alive(60);
    let (mut miners_mqtt, miners_connection) = Client::new(mqtt_options, 1024);

    let mut miners = HashMap::new();
    for (id, miner) in self.miners.iter() {
        miners.insert(miner.plug_id.clone(), structs::MinerData{
            last_received: None,
            name: id.clone(),
            energy_consumed: 0,
            phase: 0, //TODO: miner.phase,
            power: 0.0,
        });

        //TODO: check miner is active
        miners_mqtt.subscribe(
            format!("shellies/{}/relay/0/power", miner.plug_id),
            QoS::AtMostOnce
        ).unwrap();
        miners_mqtt.subscribe(
            format!("shellies/{}/relay/0/energy", miner.plug_id),
            QoS::AtMostOnce
        ).unwrap();
    }

    let miners_thread = {
        let db_tx = db_tx.clone();
        thread::spawn(|| handlers::miners_loop(miners_connection, miners, db_tx))
    };
    println!("Miners loop spawned");

    //thread::sleep(Duration::from_secs(70));
    //switchboard_mqtt.disconnect().unwrap();
    
    db_thread.join().unwrap();
    //switchboard_thread.join().unwrap();
    miners_thread.join().unwrap();

    println!("Threads terminated");
}

fn parse_announce(&mut self, data: &Publish) {
    let device = json::parse(
        std::str::from_utf8(&data.payload).unwrap()
    ).unwrap();

    if data.topic == "shellies/announce" {
        let id = device["id"].as_str().unwrap();
        let dev_type =  ShellyType::from_str(device["model"].as_str().unwrap());
        println!("dev type = {:?}", device["model"].as_str().unwrap());
        match dev_type {
            Ok(ShellyType::SHEM_3) => {
                /* It is switchboard */
                if self.switchboard.id == id {
                    self.switchboard.state = DeviceState::Available;
                } else {
                    return;
                }
            },
            Ok(ShellyType::SHPLG_S) => {
                /* It is plug */
                if let Some(plug) = self.plugs.get_mut(id) {
                    plug.state = DeviceState::Available;
                }
                else {
                    return;
                }
            },
            Err(_) => {}
        }
    } else if data.topic == "guards/announce" {
        let guard_id = device["id"].as_str().unwrap();
        let dev_type = match GuardType::from_str(device["type"].as_str().unwrap()) {
            Ok(t) => t,
            Err(error_msg ) => {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            }
        };

        let guard = if let Some(guard) = self.guards.get_mut(guard_id) {
            if guard.board_type != dev_type {
                eprintln!("Guard {} has different board type than in config file", guard_id);
                std::process::exit(1);
            } 
            guard.state = DeviceState::ConfigExpired;
            guard
        }
        else {
            /* Guard is not defined in config file */
            return;
        };

        let mut guard_miners = guard.miners.clone();

        if let JsonValue::Array(miners) = &device["miners"] {
            for miner in miners {
                let miner_id = miner["id"].as_str().unwrap();
                let pinset = miner["pinset"].as_u32().unwrap();
                if let Some(miner) = self.miners.get_mut(miner_id) {
                    /* Check miner_id from announce cover system data from config file */
                    if miner.id != miner_id || miner.pinset != pinset {
                        /* Miner has different configuration */
                        continue;
                    }

                    if let Some(position) = guard_miners.iter().position(|x| x == miner_id) {
                        guard_miners.swap_remove(position);
                    }
                } else {
                    /* Miner is not defined in config file */
                    eprintln!("Miner {} is not configured in config file", miner_id);
                    std::process::exit(1);
                }
            }

            if guard_miners.is_empty() {
                guard.state = DeviceState::Available;
            }
        } else {
            eprintln!("Guard {} sent improper json format", guard_id);
            std::process::exit(1);
        }
    } else {
        /* Got msg from unimplemented device */
    }
}

fn get_mqtt_options(&self, client_id: &str) -> MqttOptions {
    let mut mqtt_options = MqttOptions::new(
        client_id,
        self.mqtt_config.host.clone(), 
        self.mqtt_config.port
    );
    mqtt_options.set_credentials(
        self.mqtt_config.user.clone(),
        self.mqtt_config.password.clone()
    );
    return mqtt_options;
} 

fn get_switchboard_data(&self) -> ([f64; 3], [f64; 3]) {
    let mut mqtt_options = self.get_mqtt_options("Initial_loop");
    mqtt_options.set_keep_alive(5);
    let (mut client, mut connection) = Client::new(mqtt_options, 128);

    let topic_consumed = format!("shellies/{}/emeter/+/total", self.switchboard.id);
    let topic_returned = format!("shellies/{}/emeter/+/total_returned", self.switchboard.id);

    //let topic_pattern = format!("shellies/{}/emeter/{{}}/{{}}", self.switchboard.id);

    client.subscribe(topic_consumed, QoS::AtMostOnce).unwrap();
    client.subscribe(topic_returned, QoS::AtMostOnce).unwrap();
    

    let mut consumed = [None; 3];
    let mut returned = [None; 3];

    for msg in connection.iter() {
        if let ([Some(_), Some(_), Some(_)], [Some(_), Some(_), Some(_)]) = (consumed, returned) {
            client.disconnect().unwrap();
            break;
        }

        match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                let (_, i, d) = scanf!(data.topic, "shellies/{}/emeter/{}/{}", String, usize, String).unwrap();
                let value = f64::from_str(std::str::from_utf8(&data.payload).unwrap()).unwrap().ceil();
                match d.as_str() {
                    "total" => consumed[i] = Some(value),
                    "total_returned" => returned[i] = Some(value),
                    _ => {}
                }
            },
            Ok(_) => (), 
            Err(_) => (),
        }
    }

    let consumed = [consumed[0].unwrap(), consumed[1].unwrap(), consumed[2].unwrap()];
    let returned = [returned[0].unwrap(), returned[1].unwrap(), returned[2].unwrap()];

    return (consumed, returned);
}

}

pub fn current_biling_period(start_year: i32, start_month: u32, billing_period: u32) -> (NaiveDateTime, NaiveDateTime) {
    
    fn get_period(mut year: i32, mut month: u32, mut period: u32) -> (NaiveDateTime, NaiveDateTime) {
        let start = NaiveDate::from_ymd(year, month, 1).and_hms( 0, 0, 0);
        
        while period >= 12 {
            year += 1;
            period -= 12;
        }

        month += period;
        if month > 12 {
            year += 1;
            month -= 12 ;
        }

        let end = NaiveDate::from_ymd(year, month, 1).and_hms( 0, 0, 0);

        return (start, end);
    }

    let (mut period_start, mut period_end) = get_period(start_year, start_month, billing_period);
    let now = Utc::now().naive_utc();
    while period_end < now {
        let period = get_period(period_end.year(), period_end.month(), billing_period);
        period_start = period.0;
        period_end = period.1;
    }

    return (period_start, period_end);
}