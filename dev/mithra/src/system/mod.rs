
use json::JsonValue;

use r2d2::State;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS, Publish};

use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub enum DeviceState {
    Unknown,
    Available,
    Inaccessible,
    Broken,
}

#[derive(Debug, PartialEq)]
pub enum ShellyType {
    /* List can be extended in future */
    SHEM_3,
    SHPLG_S
}

impl FromStr for ShellyType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SHEM_3" => Ok(Self::SHEM_3),
            "SHPLG_S" => Ok(Self::SHPLG_S),
            _ => Err(String::from("Unimplemented shelly device")) 
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum GuardType {
    /* List can be extended in future */
    ESP32
}

impl GuardType {
    pub fn get_pinout_limit(&self) -> u32 {
        match self {
            GuardType::ESP32 => 4
        }
    }
}

impl FromStr for GuardType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ESP32" => Ok(Self::ESP32),
            _ => Err(String::from("Unimplemented guard type")) 
        }
    }
}

#[derive(Debug)]
pub struct Plug {
    pub id: String,
    pub state: DeviceState,
    // last seen
}

#[derive(Debug)]
pub struct Miner {
    pub id: String,
    pub plug: String,
    pub guard: String,
    pub pinout: u32,
    pub power_consumption: Option<u32>, // Watts
}


#[derive(Debug)]
pub struct Guard {
    pub id: String,
    pub miners: Vec<String>,
    pub board_type: GuardType,
    pub state: DeviceState,
}

#[derive(Debug)]
pub struct Switchboard {
    pub id: String,
    pub state: DeviceState,
}

#[derive(Debug)]
pub struct System {
    pub switchboard: Switchboard,
    pub guards: HashMap<String, Guard>,
    pub miners: HashMap<String, Miner>,
    pub plugs: HashMap<String, Plug>, // maps plug to miner 
}

impl System {
    pub fn init(&mut self, mqtt_config: &MqttOptions) {
        let (mut client, mut connection) = Client::new(mqtt_config.clone(), 1);
        client.subscribe("shellies/announce", QoS::AtMostOnce);
        //client.subscribe("guards/announce", QoS::AtMostOnce);

        if let Err(error_msg) = client.publish(
            "shellies/command",
            QoS::AtMostOnce,
            false,
            "announce".as_bytes()
        ) {
            eprintln!("Shellies annouce command error: {}", error_msg);
            std::process::exit(1);
        }

        let timer = Instant::now();
        let time = Duration::from_secs(30);

        for msg in connection.iter() {
            if timer.elapsed() > time {
                client.disconnect();
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

        }
        
    }

    fn parse_announce(&mut self, data: &Publish) {
        let device = json::parse(
            std::str::from_utf8(&data.payload).unwrap()
        ).unwrap();

        if data.topic == "shellies/announce" {
            let dev_type =  ShellyType::from_str(device["model"].as_str().unwrap());
            let id = device["id"].as_str().unwrap();
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
            let dev_type =  GuardType::from_str(device["model"].as_str().unwrap());
            let id = device["id"].as_str().unwrap();
            match dev_type {
                Ok(GuardType::ESP32) => {
                    /* It is plug */
                    if let Some(guard) = self.guards.get_mut(id) {
                        guard.state = DeviceState::Available;
                    }
                    else {
                        return;
                    }
                },
                Err(_) => {}
            }

            if let JsonValue::Array(miners) = &device["miners"] {

            } else {
                /* TODO: check how libn parse jsons */
            }
        } else {
            /* Got msg from unimplemented device */
        }
    }
}