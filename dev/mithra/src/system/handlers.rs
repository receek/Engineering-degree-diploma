use chrono::{NaiveDateTime, Utc};
use rumqttc::{Connection, Event, Packet, Outgoing, };
use sscanf::scanf;
use std::collections::{HashMap};
use std::sync::mpsc::{self, Sender, Receiver};
use std::time::{Duration, Instant};

use super::structs;

pub fn switchboard_loop(mut connection: Connection, tx: Sender<structs::EnergyData>) {
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
                let (_, i, data_type) = scanf!(data.topic ,"shellies/{}/emeter/{}/{}", String, usize, String).unwrap();
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
                drop(tx);
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
            // TODO:
            if let Err(error_msg) = tx.send(structs::EnergyData::Switchboard{
                ts: Utc::now().naive_utc(),
                ec: [energy_consumed_wmin[0].unwrap(), energy_consumed_wmin[1].unwrap(), energy_consumed_wmin[2].unwrap()],
                er: [energy_returned_wmin[0].unwrap(), energy_returned_wmin[1].unwrap(), energy_returned_wmin[2].unwrap()],
                tc: [total_consumed_kwh[0].unwrap(), total_consumed_kwh[1].unwrap(), total_consumed_kwh[2].unwrap()],
                tr: [total_returned_kwh[0].unwrap(), total_returned_kwh[1].unwrap(), total_returned_kwh[2].unwrap()],
            }) {
                eprintln!("Error during sending switchboard data to db_client thread: {}", error_msg);
            }

            erase_array(&mut energy_consumed_wmin);
            erase_array(&mut energy_returned_wmin);
            erase_array(&mut total_consumed_kwh);
            erase_array(&mut total_returned_kwh);

            energy_collected = false;
            total_collected = false;
        }
    }

    println!("Switchboard MQTT messages receiver exits. Connection is disconnected by client.");
}

pub fn miners_loop(mut connection: Connection, mut miners: HashMap<String, structs::MinerData>, tx: Sender<structs::EnergyData>) {
    
    let interval = Duration::from_secs(90);

    for msg in connection.iter() {
        //println!("Miner loop msg got.");
        match msg {
            Ok(Event::Incoming(Packet::Publish(data))) => {
                let (miner_id, data_type) = scanf!(data.topic ,"shellies/{}/relay/0/{}", String, String).unwrap();
                let payload = std::str::from_utf8(&data.payload).unwrap();

                let mut miner = miners.get_mut(&miner_id).unwrap();
                let now = Instant::now();

                match data_type.as_str() {
                    "power" => {
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
                    "energy" => {
                        let consumed_now = payload.parse::<u64>().unwrap();
                        
                        if let Some(last) = miner.last_received {
                            if last + interval > now && miner.energy_consumed < consumed_now {
                                if let Err(error_msg) = tx.send(structs::EnergyData::Miner{
                                    ts: Utc::now().naive_utc(),
                                    name: miner.name.clone(),
                                    ec: consumed_now - miner.energy_consumed,
                                    phase: miner.phase,
                                    power: miner.power, 
                                }) {
                                    eprintln!("Error during sending miner data to db_client thread: {}", error_msg);
                                }
                                //TODO: Send data to main thread 
                            }
                        }

                        miner.energy_consumed = consumed_now;
                        miner.last_received = Some(now);
                    },
                    _ => {},
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
}