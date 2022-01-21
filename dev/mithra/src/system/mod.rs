use chrono::{NaiveDate, NaiveDateTime, Utc, Datelike};

use json::JsonValue;

use postgres::{Config, NoTls};

use rumqttc::{Client, Event, MqttOptions, Outgoing, Packet, Publish, QoS, Connection};

use sscanf::scanf;

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self, JoinHandle};

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
    let _db_client = match self.db_config.connect(NoTls) {
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

fn init(&mut self) -> 
(
    (NaiveDateTime, NaiveDateTime),
    ([f64; 3], [f64; 3]),
    [u64; 3]
) {
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
    let time = Duration::from_secs(5);
    println!("Announce message sent");
    for msg in connection.iter() {
        if timer.elapsed() > time {
            client.disconnect().unwrap();
        }

        match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                self.parse_announce(&data);
            },
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                break;
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

    return (period, switchboard_params, miners_consumption)
}

pub fn run(&mut self) {
    /* Initialize system */
    let (
        period,
        (mut start_consumed_kWh, mut start_returned_kWh),
        mut miners_consumed_Wmin
    ) = self.init();

    /* Creating all essentials channels */
    let (db_tx, db_rx) = mpsc::channel();
    let (main_tx, main_rx) = mpsc::channel();

    /* Creating MQTT connection for every worker loop */
    let mut mqtt_options = self.get_mqtt_options("Switchboard_loop");
    mqtt_options.set_keep_alive(60);
    let (mut switchboard_mqtt, switchboard_connection) = Client::new(mqtt_options, 1024);

    let mut mqtt_options = self.get_mqtt_options("Miners_loop");
    mqtt_options.set_keep_alive(60);
    let (mut plugs_mqtt, plugs_connection) = Client::new(mqtt_options, 1024);

    let mut mqtt_options = self.get_mqtt_options("Guards_loop");
    mqtt_options.set_keep_alive(60);
    let (mut guards_mqtt, guards_connection) = Client::new(mqtt_options, 1024);

    let mut mqtt_options = self.get_mqtt_options("User_loop");
    mqtt_options.set_keep_alive(60);
    let (mut user_mqtt, user_connection) = Client::new(mqtt_options, 1024);

    /* Subscribing all essentials topics */

    /* Switchboard topics */
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

    /* Plugs topics */
    for (_, miner) in self.miners.iter() {
        plug_subscribe(&mut plugs_mqtt, &miner.plug_id);
        plugs_mqtt.subscribe(
            format!("shellies/{}/relay/0", miner.plug_id),
            QoS::AtMostOnce
        ).unwrap();
    }

    /* Guards topics */
    for (guard_id, guard) in self.guards.iter() {
        guards_mqtt.subscribe(format!("guards/started"),QoS::AtMostOnce).unwrap();
        guards_mqtt.subscribe(format!("guards/{}/configured", guard_id),QoS::AtMostOnce).unwrap();
        guards_mqtt.subscribe(format!("guards/{}/ping", guard_id),QoS::AtMostOnce).unwrap();
        for miner_id in guard.miners.iter() {
            miner_subscribe(&mut guards_mqtt, guard_id, miner_id);
        }

        /* Reset guard if it has expired config */
        if guard.state == DeviceState::ConfigExpired {
            guard_reset(&mut guards_mqtt, guard_id);
        } else if guard.state == DeviceState::Available {
            for miner_id in guard.miners.iter() {
                let miner = self.miners.get_mut(miner_id).unwrap();
                guard_send_command(&mut guards_mqtt, guard_id, miner_id, "StateReport");
                miner.command_ts = Some(Utc::now().naive_utc());
            }
        }
    }

    /* User topics */
    for (miner_id, _) in self.miners.iter() {
        user_mqtt.subscribe(format!("user/{}", miner_id), QoS::AtMostOnce).unwrap();
    }

    /* Spawn all workers */
    let db_thread = {
        let db_config = self.db_config.clone();
        thread::spawn(|| database::insert_energy_data(db_config, db_rx))
    };
    println!("Database worker loop spawned");

    let switchboard_thread = {
        let db_tx = db_tx.clone();
        let main_tx = main_tx.clone();
        thread::spawn(|| handlers::switchboard_loop(switchboard_connection, db_tx, main_tx))
    };
    println!("Switchboard worker loop spawned");

    let plugs_thread = {
        let mut miners = HashMap::new();
        for (id, miner) in self.miners.iter() {
            miners.insert(miner.plug_id.clone(), structs::MinerData{
                last_received: None,
                name: id.clone(),
                energy_consumed: 0,
                phase: miner.phase,
                power: 0.0,
            });
        }
        let db_tx = db_tx.clone();
        let main_tx = main_tx.clone();
        thread::spawn(|| handlers::plugs_loop(plugs_connection, miners, db_tx, main_tx))
    };
    println!("Plugs worker loop spawned");
    
    let guards_thread = {
        let main_tx = main_tx.clone();
        thread::spawn(|| handlers::guards_loop(guards_connection, main_tx))
    };
    println!("Guards worker loop spawned");
    
    let user_thread = {
        let main_tx = main_tx.clone();
        thread::spawn(|| handlers::user_loop(user_connection, main_tx))
    };
    println!("User worker loop spawned");

    let mut last_miners_consumed_Wmin = [0; 3];
    let mut last_switchboard_consumed_Wmin = [0; 3];
    let mut last_switchboard_returned_Wmin = [0; 3];
    let mut actual_total_consumed_kWh = [0.0; 3];
    let mut actual_total_returned_kWh = [0.0; 3];

    let mut switchboard_received_msgs = 0;

    let mut deadline = Instant::now() + Duration::from_secs(60);

    loop {
        match main_rx.recv_deadline(deadline) {
            Ok(msg) => match msg {
                Message::Energy(EnergyData::Miner{ts: _, name, ec, phase, power}) => {
                    /* Update local energy data */
                    let miner = self.miners.get_mut(&name).unwrap();
                    miner.power_consumption = Some(power);
    
                    let i = phase as usize;
                    miners_consumed_Wmin[i] += ec;
                    last_miners_consumed_Wmin[i] += ec;
    
                    switchboard_received_msgs += 1;
                },
                Message::Energy(EnergyData::Switchboard{ts, ec, er, tc, tr}) => {
                    /* Update local energy data */
                    self.switchboard.last_seen = ts;
                    for i in 0..3 {
                        last_switchboard_consumed_Wmin[i] += ec[i];
                        last_switchboard_returned_Wmin[i] += er[i];
                    }
                    actual_total_consumed_kWh = tc;
                    actual_total_returned_kWh = tr;
                },
                Message::Guard {guard_id, ts, data} => {
                    self.handle_guard_msg(&guard_id, ts, data, &mut guards_mqtt, &mut plugs_mqtt);
                },
                Message::Plug{plug_id, ts, is_on} => {
                    let plug = self.plugs.get_mut(&plug_id).unwrap();
    
                    plug.last_seen = ts;
                    plug.is_enabled = is_on;
                },
                Message::User{miner_id, command} => {
                    let miner = self.miners.get_mut(&miner_id).unwrap();
    
                    match command {
                        UserCommands::Exclude => {
                            if miner.included {
                                miner.included = false;
                                miner.state = MinerState::NotDefined;
                                plug_unsubscribe(&mut plugs_mqtt, &miner.guard);
                                miner_unsubscribe(&mut guards_mqtt, &miner.guard, &miner_id);
                            } else {
                                eprintln!("User tried to exclude excluded miner = {}", miner_id);
                            }
                        },
                        UserCommands::Include => {
                            if miner.included {
                                plug_subscribe(&mut plugs_mqtt, &miner.guard);
                                miner_subscribe(&mut guards_mqtt, &miner.guard, &miner_id);
                                guard_send_command(&mut guards_mqtt, &miner.guard, &miner_id, "StateReport");
                                miner.included = true;
                                miner.state = MinerState::NotDefined;
                                miner.target_state = None;
                                miner.command_ts = Some(Utc::now().naive_utc());
                            } else {
                                eprintln!("User tried to include included miner = {}", miner_id);
                            }
                        },
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                /* Renew deadline */
                deadline += Duration::from_secs(30);

                /* Check threads status */
                let are_threads_running = [
                    &db_thread,
                    &switchboard_thread,
                    &plugs_thread,
                    &guards_thread,
                    &user_thread
                ].iter().all(|&t| t.is_running());

                if !are_threads_running {
                    switchboard_mqtt.disconnect().unwrap();
                    plugs_mqtt.disconnect().unwrap();
                    guards_mqtt.disconnect().unwrap();
                    user_mqtt.disconnect().unwrap();

                    break;
                }

                /* Validate devices status */
                self.validate_devices(&mut guards_mqtt, &mut plugs_mqtt);

                /* Check is it scheduling time */
                if self.switchboard.state != DeviceState::Available {
                    /* Disable all running miners - just set target state to powered off */

                } else if switchboard_received_msgs >= 5 {
                    /* Obtain all running and runnable miners */

                    /* Shedule resources */

                    switchboard_received_msgs = 0;
                    /* Set target state to miners after scheduling */
                }

                /* If billing period is ending then close disconnect mqtt clients and sleep thread until new period start */
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    //thread::sleep(Duration::from_secs(70));
    //switchboard_mqtt.disconnect().unwrap();

    db_thread.join().unwrap();
    switchboard_thread.join().unwrap();
    plugs_thread.join().unwrap();
    guards_thread.join().unwrap();
    user_thread.join().unwrap();

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
        println!("dev type = {:?}", dev_type);

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

fn handle_guard_msg(&mut self, guard_id: &String, ts: NaiveDateTime, data: GuardData, guards_mqtt: &mut Client, plugs_mqtt: &mut Client) {
    let guard = if let Some(guard) = self.guards.get_mut(guard_id) {
        guard
    } else {
        eprintln!("Main loop - undefined guard: {}", guard_id);
        return;
    };

    guard.last_seen = ts;

    match data {
        GuardData::Alert{miner_id, alert} => {
            /* Try to react */
            let miner = if let Some(miner) = self.miners.get_mut(&miner_id) {
                miner
            } else {
                eprintln!("Configuration is not consistent");
                std::process::exit(1);
            };

            match alert {
                MinerAlert::PoweredOff => {
                    /* Miner was running but is powered off now */
                    miner.state = MinerState::Aborted;
                },
                MinerAlert::PoweredOn => {
                    /* Miner runs unexpectedly, cut power off by plug */
                    miner.state = MinerState::Unreachable;
                    plug_cut_off(plugs_mqtt, &miner.plug_id);
                },
            }
        },
        GuardData::Command{miner_id, command_status, miner_state} => {
            let miner = if let Some(miner) = self.miners.get_mut(&miner_id) {
                miner
            } else {
                eprintln!("Configuration is not consistent");
                std::process::exit(1);
            };

            if command_status == CommandStatus::Busy  {
                eprintln!("Mithra made illegal operations while guard was running command!");
                miner.state = miner_state;
                return;
            }
            if command_status == CommandStatus::Disallowed {
                eprintln!("Mithra made illegal operations to miner state");
                miner.state = miner_state;
                return;
            }
            

            if command_status == CommandStatus::Done { match (miner.state, miner_state) {
                /* Guard retuns that command execution succeed */
                (MinerState::Starting, MinerState::Running) |
                (MinerState::Stopping, MinerState::PoweredOff) |
                (MinerState::HardStopping, MinerState::PoweredOff) |
                (MinerState::Restarting, MinerState::Running) |
                (MinerState::HardRestarting, MinerState::Running) => {
                    miner.state = miner_state;
                    miner.command_ts = None;
                }
                (_, _) => {
                    eprintln!(
                        "Mithra has wrong miner state, miner = {}, state returned from guard = {}, mithra state = {}",
                        miner_id, miner_state.to_string(), miner.state.to_string()
                    );
                    guard_send_command(guards_mqtt, guard_id, &miner_id, "StateReport");
                    miner.state = MinerState::NotDefined;
                    miner.command_ts = Some(Utc::now().naive_utc());
                }
            }} else if command_status == CommandStatus::Failed { match (miner.state, miner_state) {
                /* Guard retuns that command execution failed */
                (MinerState::Stopping, MinerState::Unreachable) => {
                    /* Try hardstop */
                    guard_send_command(guards_mqtt, &guard_id, &miner_id, "HardStop");
                    miner.state = MinerState::HardStopping;
                    miner.command_ts = Some(Utc::now().naive_utc());
                },
                (MinerState::HardStopping, MinerState::Unreachable) => {
                    miner.state = MinerState::Unreachable;
                    plug_cut_off(plugs_mqtt, &miner.plug_id);
                },
                (MinerState::Starting, MinerState::Aborted) |
                (MinerState::Restarting, MinerState::Aborted) |
                (MinerState::HardRestarting, MinerState::Aborted) => {
                    miner.state = miner_state;
                    miner.command_ts = None;
                }
                (_, _) => {
                    eprintln!(
                        "Mithra has wrong miner state, miner = {}, state returned from guard = {}, mithra state = {}",
                        miner_id, miner_state.to_string(), miner.state.to_string()
                    );
                    guard_send_command(guards_mqtt, guard_id, &miner_id, "StateReport");
                    miner.state = MinerState::NotDefined;
                    miner.command_ts = Some(Utc::now().naive_utc());
                }
            }}
        },
        GuardData::Configured => {
            /* Change guard state, publish state message to obtain miners status */
            if guard.state != DeviceState::StartingUp {
                eprintln!("Mithra got guard configured messeage but there was not guard started message!");
                guard_reset(guards_mqtt, &guard_id);
                return;
            }

            guard.state = DeviceState::Available;

            for miner_id in guard.miners.iter() {
                let miner = self.miners.get_mut(miner_id).unwrap();
                if miner.included {
                    guard_send_command(guards_mqtt, &guard_id, miner_id, "StateReport");
                    miner.command_ts = Some(Utc::now().naive_utc());
                }
            }   
        },
        GuardData::Ping => {
            /* Timestamp has been updated just before match */
        },
        GuardData::Started => {
            let config = get_guard_config(guard, &self.miners);
            guard.state = DeviceState::StartingUp;
            
            for miner_id in guard.miners.iter() {
                let miner = if let Some(miner) = self.miners.get_mut(miner_id) {
                    miner
                } else {
                    eprintln!("Configuration is not consistent");
                    std::process::exit(1);
                };
                
                miner.state = MinerState::NotDefined;
            }
            guards_mqtt.publish(
                format!("guards/{}/config", guard_id),
                QoS::AtMostOnce,
                false,
                config
            ).unwrap();
        },
        GuardData::State{miner_id, state} => {
            /* Update miner state */
            if state == MinerState::NotDefined {
                panic!("[Main loop] Guard sent NotDefined state of miner!");
            }

            let miner = if let Some(miner) = self.miners.get_mut(&miner_id) {
                miner
            } else {
                eprintln!("Configuration is not consistent");
                std::process::exit(1);
            };


            /* Mithra should ask miner state only if state is not defined */
            if miner.state != MinerState::NotDefined  {
                eprintln!("Miner state report on guard {} and miner {} while state is known", guard_id, miner_id);
            } else {
                match state {
                    MinerState::Starting  |
                    MinerState::Stopping  |
                    MinerState::HardStopping |  
                    MinerState::Restarting | 
                    MinerState::HardRestarting => {
                        miner.command_ts = Some(Utc::now().naive_utc());
                    },
                    _ => {}
                }
                miner.state = state;
            }
        },
    }
}

fn validate_devices(&mut self, guards_mqtt: &mut Client, plugs_mqtt: &mut Client) {
    use chrono::Duration;

    let now = Utc::now().naive_utc();

    if now - self.switchboard.last_seen > Duration::seconds(150) {
        if self.switchboard.state == DeviceState::Available {
            self.switchboard.state = DeviceState::Inaccessible;
            for (_, miner) in self.miners.iter_mut() {
                miner.target_state = Some(MinerState::PoweredOff);
            }
        }
    } else if self.switchboard.state == DeviceState::Inaccessible {
        self.switchboard.state = DeviceState::Available;
    }

    for (guard_id, guard) in self.guards.iter_mut() {
        if now - guard.last_seen > Duration::seconds(45) {
            if guard.state == DeviceState::Available {
                guard.state = DeviceState::Inaccessible;
            }
            if now - guard.last_seen > Duration::minutes(5) {
                for miner_id in guard.miners.iter() {
                    let miner = self.miners.get_mut(miner_id).unwrap();
                    let plug = self.plugs.get_mut(&miner.plug_id).unwrap();

                    if miner.included && plug.is_enabled {
                        plug_cut_off(plugs_mqtt, &miner.plug_id);
                    }
                }
            }
        } else if guard.state ==  DeviceState::Inaccessible {
            guard.state = DeviceState::Available;
            if now - guard.last_seen <= Duration::minutes(5) {
                for miner_id in guard.miners.iter() {
                    let miner = self.miners.get_mut(miner_id).unwrap();
                    let plug = self.plugs.get_mut(&miner.plug_id).unwrap();

                    if miner.included && !plug.is_enabled {
                        plug_enable(plugs_mqtt, &miner.plug_id);
                        guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                        miner.state = MinerState::NotDefined;
                        miner.command_ts = Some(Utc::now().naive_utc());
                    }
                }
            }
        } else {
            for miner_id in guard.miners.iter() {
                let miner = self.miners.get_mut(miner_id).unwrap();

                if !miner.included { continue; }

                let plug = self.plugs.get_mut(&miner.plug_id).unwrap();
                
                if now - plug.last_seen > Duration::seconds(60) {
                    plug.state = DeviceState::Inaccessible;
                } else {
                    if plug.state == DeviceState::Inaccessible {
                        plug.state = DeviceState::Available;
                    }
                }
    
                if !plug.is_enabled { 
                    if miner.target_state == Some(MinerState::Running) {
                        plug_enable(plugs_mqtt, &miner.plug_id);
                    } 
                    match miner.state {
                        MinerState::Aborted | MinerState::PoweredOff => {}
                        _ => {
                            eprintln!("[Main loop] Miner '{}' has disabled plug but it's not powered off!", miner_id);
                        }
                    }
                }

                match miner.target_state {
                    Some(MinerState::Running) | Some(MinerState::PoweredOff) => {},
                    None => {} ,
                    Some(target) => {
                        eprintln!("[Main loop] Miner '{}' improper target state '{:?}'!", miner_id, target);
                        std::process::exit(1);
                    },
                }

                match (miner.state, miner.target_state, miner.command_ts) {
                    (MinerState::PoweredOff, Some(MinerState::PoweredOff), _) |
                    (MinerState::Running, Some(MinerState::Running), _) => {
                        /* Achieved target state */
                    },
                    (MinerState::PoweredOff, Some(MinerState::Running), _) => {
                        guard_send_command(guards_mqtt, guard_id, miner_id, "PowerOn");
                        miner.state = MinerState::Starting;
                        miner.command_ts = Some(Utc::now().naive_utc());
                    },
                    (MinerState::Running, Some(MinerState::PoweredOff), _) => {
                        guard_send_command(guards_mqtt, guard_id, miner_id, "PowerOff");
                        miner.state = MinerState::Stopping;
                        miner.command_ts = Some(Utc::now().naive_utc());
                    },
                    (MinerState::Stopping, _, Some(ts)) => {
                        if now - ts > Duration::seconds(130) {
                            eprintln!("[Main thread] Miner '{}' should be powered off, resetting local miner state.", miner_id);
                            guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                            miner.state = MinerState::NotDefined;
                            miner.command_ts = Some(Utc::now().naive_utc());
                        }
                    },
                    (MinerState::HardStopping, _, Some(ts)) => {
                        if now - ts > Duration::seconds(12) {
                            eprintln!("[Main thread] Miner '{}' should be powered off, resetting local miner state.", miner_id);
                            guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                            miner.state = MinerState::NotDefined;
                            miner.command_ts = Some(Utc::now().naive_utc());
                        }
                    },
                    (MinerState::Starting, _, Some(ts)) => {
                        if now - ts > Duration::seconds(7) {
                            eprintln!("[Main thread] Miner '{}' should be running, resetting local miner state.", miner_id);
                            guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                            miner.state = MinerState::NotDefined;
                            miner.command_ts = Some(Utc::now().naive_utc());
                        }
                    },
                    (MinerState::Restarting, _, Some(ts))  => {
                        if now - ts > Duration::seconds(12) {
                            eprintln!("[Main thread] Miner '{}' restarting failed, resetting local miner state.", miner_id);
                            guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                            miner.state = MinerState::NotDefined;
                            miner.command_ts = Some(Utc::now().naive_utc());
                        }
                    },
                    (MinerState::HardRestarting, _, Some(ts)) => {
                        if now - ts > Duration::seconds(20) {
                            eprintln!("[Main thread] Miner '{}' hard restarting failed, resetting local miner state.", miner_id);
                            guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                            miner.state = MinerState::NotDefined;
                            miner.command_ts = Some(Utc::now().naive_utc());
                        }
                    },
                    (MinerState::NotDefined, _, Some(ts)) => {
                        if now - ts > Duration::seconds(10) {
                            eprintln!("[Main thread] Miner '{}' has undefined state.", miner_id);
                            guard_send_command(guards_mqtt, guard_id, miner_id, "StateReport");
                            miner.command_ts = Some(Utc::now().naive_utc());
                        }
                    },
                    (MinerState::Aborted, _, _) => {
    
                    }
                    (MinerState::Unreachable, _, _) => {
                        if plug.is_enabled {
                            plug_cut_off(plugs_mqtt, &miner.plug_id);
                        } 
                    }
                    
                    (state, target, _) => {
                        eprintln!(
                            "[Main thread] System data for miner '{}' has undesirable state = {:?}, target state = {:?}",
                            miner_id, state, target
                        );
                    }
                }
            }
        }
    }
}

}


fn get_guard_config(guard: &Guard, miners: &HashMap<String, Miner>) -> String {
    let mut config = guard.miners.len().to_string();

    for miner_id in guard.miners.iter() {
        let pinset = if let Some(miner) = miners.get(miner_id) {
            miner.pinset
        } else {
            eprintln!("Configuration is not consistent");
            std::process::exit(1);
        };

        config += format!(" {} {}", miner_id, pinset).as_str();
    }

    return config;
}

fn guard_send_command(guards_mqtt: &mut Client, guard_id: &String, miner_id: &String, command: &str) {
    guards_mqtt.publish(
        format!("guards/{}/miners/{}", guard_id, miner_id),
        QoS::AtMostOnce,
        false,
        command,
    ).unwrap();
}

fn guard_reset(guards_mqtt: &mut Client, guard_id: &String) {
    guards_mqtt.publish(
        format!("guards/{}/command", guard_id),
        QoS::AtMostOnce,
        false,
        "reset".as_bytes(),
    ).unwrap();
}

fn miner_subscribe(guards_mqtt: &mut Client, guard_id: &String, miner_id: &String) {
    guards_mqtt.subscribe(format!("guards/{}/miners/{}/alert", guard_id, miner_id), QoS::AtMostOnce).unwrap();
    guards_mqtt.subscribe(format!("guards/{}/miners/{}/command", guard_id, miner_id), QoS::AtMostOnce).unwrap();
    guards_mqtt.subscribe(format!("guards/{}/miners/{}/status", guard_id, miner_id), QoS::AtMostOnce).unwrap();
}


fn miner_unsubscribe(guards_mqtt: &mut Client, guard_id: &String, miner_id: &String) {
    guards_mqtt.unsubscribe(format!("guards/{}/miners/{}/alert", guard_id, miner_id)).unwrap();
    guards_mqtt.unsubscribe(format!("guards/{}/miners/{}/command", guard_id, miner_id)).unwrap();
    guards_mqtt.unsubscribe(format!("guards/{}/miners/{}/status", guard_id, miner_id)).unwrap();
}

fn plug_subscribe(plugs_mqtt: &mut Client, plug_id: &String) {
    plugs_mqtt.subscribe(format!("shellies/{}/relay/0/power", plug_id), QoS::AtMostOnce).unwrap();
    plugs_mqtt.subscribe(format!("shellies/{}/relay/0/energy", plug_id), QoS::AtMostOnce).unwrap();
}


fn plug_unsubscribe(plugs_mqtt: &mut Client, plug_id: &String) {
    plugs_mqtt.unsubscribe(format!("shellies/{}/relay/0/power", plug_id)).unwrap();
    plugs_mqtt.unsubscribe(format!("shellies/{}/relay/0/energy", plug_id)).unwrap();
}

fn plug_cut_off(plugs_mqtt: &mut Client, plug_id: &String) {
    plugs_mqtt.publish(
        format!("shellies/{}/relay/0/command", plug_id), 
        QoS::AtMostOnce,
        false,
        "off".as_bytes()
    ).unwrap();
}

fn plug_enable(plugs_mqtt: &mut Client, plug_id: &String) {
    plugs_mqtt.publish(
        format!("shellies/{}/relay/0/command", plug_id), 
        QoS::AtMostOnce,
        false,
        "on".as_bytes()
    ).unwrap();
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