use chrono::{NaiveDateTime, Utc};
use rumqttc::{Client, Connection, Event, Packet, Outgoing};
use sscanf::scanf;
use std::collections::{HashMap, HashSet};
use std::fmt::Result;
use std::str::FromStr;
use std::sync::mpsc::{self, Sender, Receiver};
use std::time::{Duration, Instant};

use crate::system::structs::{GuardType, MinerState};

use super::structs;

pub fn switchboard_loop(mut connection: Connection, tx_db: Sender<structs::EnergyData>, tx_main: Sender<structs::Message>) {
    fn is_collected<T>(array: &[Option<T>; 3]) -> bool {
        array.iter().all(|x: &Option<T>| x.is_some())
    }

    fn erase_array<T>(array: &mut [Option<T>; 3]) {
        for elem in array.iter_mut() {*elem = None}
    }

    let mut energy_collected = false;
    let mut total_collected = false;

    let mut energy_consumed_wmin = [None; 3];
    let mut energy_returned_wmin = [None; 3];
    let mut total_consumed_kwh = [None; 3];
    let mut total_returned_kwh = [None; 3];

    for msg in connection.iter() {
        match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                let (_, i, data_type) = if let Some(parsed) = scanf!(
                    data.topic,
                    "shellies/{/[^/]+/}/emeter/{/[^/]+/}/{/[^/]+/}",
                    String, usize, String
                ) {
                    parsed
                } else {
                    eprintln!("[Switchboard loop] Received wrong topic: {}", data.topic);
                    continue;
                };

                if i > 2 {
                    eprintln!("[Switchboard loop] Got message of phase: {}", i);
                    continue;
                }

                let payload = std::str::from_utf8(&data.payload).unwrap();

                match data_type.as_str() {
                    "energy" => {
                        let value = payload.parse::<u64>().unwrap();
                        energy_consumed_wmin[i] = Some(value);
                    },
                    "returned_energy" => {
                        let value = payload.parse::<u64>().unwrap();
                        energy_returned_wmin[i] = Some(value);
                    },
                    "total" => {
                        if !energy_collected { continue; }

                        let value = payload.parse::<f64>().unwrap();
                        total_consumed_kwh[i] = Some(value);
                    }
                    "total_returned" => {
                        if !energy_collected { continue; }

                        let value = payload.parse::<f64>().unwrap();
                        total_returned_kwh[i] = Some(value);
                    }
                    _ => {}
                }
            },
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                /* Mithra is terminating */
                drop(tx_db);
                drop(tx_main);
                break;
            }
            Ok(_) => (), 
            Err(_) => (),
        }

        if !energy_collected {
            energy_collected = is_collected(&energy_consumed_wmin) && is_collected(&energy_returned_wmin);
        } else if !total_collected {
            total_collected = is_collected(&total_consumed_kwh) && is_collected(&total_returned_kwh);
        } else {
            /* Send data to database and to main thread by channel */
            
            let msg = structs::EnergyData::Switchboard{
                ts: Utc::now().naive_utc(),
                ec: [energy_consumed_wmin[0].unwrap(), energy_consumed_wmin[1].unwrap(), energy_consumed_wmin[2].unwrap()],
                er: [energy_returned_wmin[0].unwrap(), energy_returned_wmin[1].unwrap(), energy_returned_wmin[2].unwrap()],
                tc: [total_consumed_kwh[0].unwrap(), total_consumed_kwh[1].unwrap(), total_consumed_kwh[2].unwrap()],
                tr: [total_returned_kwh[0].unwrap(), total_returned_kwh[1].unwrap(), total_returned_kwh[2].unwrap()],
            };

            if let Err(_) = tx_db.send(msg.clone()) {
                eprintln!("[Switchboard loop] Database channel is closed!");
                break;
            }

            if let Err(_) = tx_main.send(structs::Message::Energy(msg)) {
                println!("[Switchboard loop] Main thread channel is closed!");
                drop(tx_db);
                break;
            }

            erase_array(&mut energy_consumed_wmin);
            erase_array(&mut energy_returned_wmin);
            erase_array(&mut total_consumed_kwh);
            erase_array(&mut total_returned_kwh);

            energy_collected = false;
            total_collected = false;
        }
    }

    println!("Switchboard MQTT messages receiver exits.");
}

pub fn plugs_loop(mut connection: Connection, mut miners: HashMap<String, structs::MinerData>, tx_db: Sender<structs::EnergyData>, tx_main: Sender<structs::Message>) {
    let interval = Duration::from_secs(90);

    for msg in connection.iter() {
        //println!("Miner loop msg got.");
        match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                let (plug_id, data_type) = if let Some(parsed) = scanf!(data.topic ,"shellies/{/[^/]+/}/relay/{}", String, String) {
                    parsed
                } else {
                    eprintln!("[Plugs loop] Arrived message from undefined topic: {}", data.topic);
                    continue;
                };

                let payload = std::str::from_utf8(&data.payload).unwrap();
                let mut miner = if let Some(miner) = miners.get_mut(&plug_id) {
                    miner
                } else {
                    eprintln!("[Plugs loop] Arrived message from undefined plug: {}", plug_id);
                    continue;
                };

                let now = Instant::now();

                match data_type.as_str() {
                    "0/power" => {
                        let power_now = payload.parse::<f32>().unwrap();
                        if let Some(last) = miner.last_received {
                            if last + interval > now {
                                miner.power = miner.power.max(power_now);
                            } else {
                                miner.power = power_now
                            }
                        } else {
                            miner.power = power_now
                        }
                    },
                    "0/energy" => {
                        let consumed_now = payload.parse::<u64>().unwrap();
                        
                        if let Some(last) = miner.last_received {
                            if last + interval > now && miner.energy_consumed < consumed_now {
                                let msg = structs::EnergyData::Miner{
                                    ts: Utc::now().naive_utc(),
                                    name: miner.name.clone(),
                                    ec: consumed_now - miner.energy_consumed,
                                    phase: miner.phase,
                                    power: miner.power, 
                                };

                                if let Err(_) = tx_db.send(msg.clone()) {
                                    eprintln!("[Plugs loop] Database channel is closed!");
                                    break;
                                }
                                if let Err(_) = tx_main.send(structs::Message::Energy(msg)) {
                                    println!("[Plugs loop] Main thread channel is closed!");
                                    drop(tx_db);
                                    break;
                                }
                            }
                        }

                        miner.energy_consumed = consumed_now;
                        miner.last_received = Some(now);
                    },
                    "0" => { 
                        let is_on = match payload {
                            "on" => true,
                            "off" => false,
                            _ => continue
                        };
                        if let Err(_) = tx_main.send(structs::Message::Plug{
                            plug_id,
                            ts: Utc::now().naive_utc(),
                            is_on
                        }) {
                            eprintln!("[Plugs loop] Main thread channel is closed!");
                            drop(tx_db);
                            break;
                        }
                    }
                    _ => {},
                }
            },
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                /* Mithra is terminating */
                drop(tx_main);
                drop(tx_db);
                break;
            }
            Ok(_) => (), 
            Err(_) => (),
        }
    }
    println!("Plugs MQTT messages receiver exits.");
}

pub fn guards_loop(mut connection: Connection, tx: Sender<structs::Message>) {
    use super::structs::{CommandStatus, GuardData, Message, MinerAlert};

    for msg in connection.iter() { match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                let payload = std::str::from_utf8(&data.payload).unwrap();

                let topic1 = scanf!(
                    data.topic,
                    "guards/{/[^/]+/}",
                    String
                );
                let topic2 = scanf!(
                    data.topic,
                    "guards/{/[^/]+/}/{/[^/]+/}",
                    String, String
                );
                let topic3 = scanf!(
                    data.topic,
                    "guards/{/[^/]+/}/miners/{/[^/]+/}/{/[^/]+/}",
                    String, String, String
                );

                let mut msg = None;
                let ts = Utc::now().naive_utc();

                if let Some(subtopic) = topic1 {
                    match subtopic.as_str() {
                        "started" => {
                            msg = Some( Message::Guard{
                                guard_id: String::from(payload),
                                ts,
                                data: GuardData::Started
                            });
                        }
                        _ => {}
                    }
                }
                if let Some((guard_id, subtopic)) = topic2 {
                    match subtopic.as_str() {
                        "configured" => {
                            msg = Some( Message::Guard{
                                guard_id,
                                ts,
                                data: GuardData::Configured
                            });
                        },
                        "ping" => {
                            msg = Some( Message::Guard{
                                guard_id,
                                ts,
                                data: GuardData::Ping
                            });
                        }
                        _ => {}
                    }
                }
                if let Some((guard_id, miner_id, subtopic)) = topic3 {
                    match subtopic.as_str() {
                        "alert" => {
                            let alert = MinerAlert::from_str(payload).unwrap();
                            msg = Some( Message::Guard{
                                guard_id,
                                ts,
                                data: GuardData::Alert{
                                    miner_id,
                                    alert,
                                }
                            });
                        },
                        "command" => {
                            let (command_status, miner_state) = scanf!(
                                payload,
                                "command={}, state={}",
                                String, String
                            ).unwrap();
                            msg = Some( Message::Guard{
                                guard_id,
                                ts,
                                data: GuardData::Command{
                                    miner_id,
                                    command_status: CommandStatus::from_str(command_status.as_str()).unwrap(),
                                    miner_state: MinerState::from_str(miner_state.as_str()).unwrap(),
                                }
                            });
                        },
                        "status" => {
                            msg = Some( Message::Guard {
                                guard_id,
                                ts,
                                data: GuardData::State{
                                    miner_id,
                                    state: MinerState::from_str(payload).unwrap()
                                },
                            });
                        },
                        _ => {}
                    }
                }

                if let Some(msg) = msg {
                    if let Err(_) = tx.send(msg) {
                        println!("[Guard loop] Main thead channel is closed!");
                        break;
                    }
                } else {
                    /* No message, topic is wrong */
                    eprintln!("[Guard loop] Received message from unspecified topic: {} {}", data.topic, payload);
                }
            },
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                /* Mithra is terminating */
                drop(tx);
                break;
            }
            Ok(_) => (), 
            Err(_) => (),
        }
    }

    println!("Guards MQTT messages receiver exits.");
}

pub fn user_loop(mut connection: Connection, tx: Sender<structs::Message>) {
    use super::structs::{UserCommands, Message};

    for msg in connection.iter() { match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                let payload = std::str::from_utf8(&data.payload).unwrap();

                let miner_id = if let Some(miner_id) = scanf!(
                    data.topic,
                    "user/{/[^/]+/}",
                    String
                ) {
                    miner_id
                } else {
                    eprintln!("[User loop] Wrong topic: {}", data.topic);
                    continue;
                };

                let command = if let Ok(command) = UserCommands::from_str(payload) {
                    command
                } else {
                    eprintln!("[User loop] Undefined user command: {}", miner_id);
                    continue;
                };

                if let Err(_) = tx.send(Message::User{
                    miner_id,
                    command,
                }) {
                    eprintln!("[User loop] Main thread channel is closed!");
                    break;
                }
            }
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                /* Mithra is terminating */
                drop(tx);
                break;
            }
            Ok(_) => (), 
            Err(_) => (),
        }
        
    }

    println!("User MQTT messages receiver exits. Connection is disconnected by client.");
}
