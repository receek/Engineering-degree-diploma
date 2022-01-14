use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use sscanf::const_format::__str_methods::StrIndexArgs;

use std::collections::HashMap;

use std::time::{Instant};

#[derive(Debug, PartialEq)]
pub enum DeviceState {
    Unknown,
    Available,
    ConfigExpired,
    Inaccessible,
    Broken,
}

#[derive(Debug, PartialEq)]
pub enum ShellyType {
    /* List can be extended in future */
    SHEM_3,
    SHPLG_S,
}

impl FromStr for ShellyType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SHEM-3" => Ok(Self::SHEM_3),
            "SHPLG-S" => Ok(Self::SHPLG_S),
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
    pub fn get_pinset_limit(&self) -> u32 {
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
    pub miner_id: String,
    // last seen
}

#[derive(Debug)]
pub struct Miner {
    pub id: String,
    pub plug_id: String,
    pub guard: String,
    pub pinset: u32,
    pub power_consumption: Option<u32>, // Watts
}

#[derive(Debug)]
pub enum MinerState {
    NotDefined,
    PoweredOff,
    Starting,
    Running,
    Stopping,
    HardStopping, 
    Restarting,
    HardRestarting,
    Aborted,
    Unreachable
}

impl FromStr for MinerState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NotDefined" => Ok(Self::NotDefined),
            "PoweredOff" => Ok(Self::PoweredOff),
            "Starting" => Ok(Self::Starting),
            "Running" => Ok(Self::Running),
            "Stopping" => Ok(Self::Stopping),
            "HardStopping" => Ok(Self::HardStopping), 
            "Restarting" => Ok(Self::Restarting),
            "HardRestarting" => Ok(Self::HardRestarting),
            "Aborted" => Ok(Self::Aborted),
            "Unreachable" => Ok(Self::Unreachable),
            _ => Err(String::from("Unimplemented miner state")) 
        }
    }
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

/* Miner data for miners MQTT messages receiver loop */

#[derive(Debug)]
pub struct MinerData {
    pub last_received: Option<Instant>,
    pub name: String,
    pub energy_consumed: u64,
    pub phase: u16,
    pub power: f32,
}

/* Energy data representation for channel */

#[derive(Debug, Clone)]
pub enum EnergyData {
    Switchboard {ts: NaiveDateTime, ec: [u64; 3], er: [u64; 3], tc: [f64; 3], tr: [f64; 3]},
    Miner {ts: NaiveDateTime, name: String, ec: u64, phase: u16, power: f32},
}

/* Data for main thread channel */
#[derive(Debug)]
pub enum GuardData {
    Alert {miner_id: String, event: String},
    Command {miner_id: String, status: String},
    Configured,
    Ping,
    Started,
    State {miner_id: String, state: MinerState},

}

#[derive(Debug)]
pub enum Message {
    Energy(EnergyData),
    Guard {guard_id: String, ts: NaiveDateTime, data: GuardData},
}