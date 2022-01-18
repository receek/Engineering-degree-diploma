use std::process::Command;
use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use sscanf::const_format::__str_methods::StrIndexArgs;

use std::collections::HashMap;

use std::time::{Instant};


/* There is status enum for switchboard, guards and plugs */
#[derive(Debug, PartialEq)]
pub enum DeviceState {
    NotDefined,
    Available,
    ConfigExpired,
    StartingUp,
    Inaccessible,
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
    pub is_enabled: bool,
    pub last_seen: NaiveDateTime,
}

#[derive(Debug)]
pub struct Miner {
    pub id: String,
    pub plug_id: String,
    pub guard: String,
    pub pinset: u32,
    pub phase: u8,
    pub estimated_consumption: u32,
    pub power_consumption: Option<u32>, // Watts
    pub state: MinerState,
    pub included: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
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

impl ToString for MinerState {
    fn to_string(&self) -> String {
        String::from( match self {
            Self::NotDefined =>  "NotDefined",
            Self::PoweredOff =>  "PoweredOff",
            Self::Starting =>  "Starting",
            Self::Running =>  "Running",
            Self::Stopping =>  "Stopping",
            Self::HardStopping => "HardStopping",
            Self::Restarting =>  "Restarting",
            Self::HardRestarting =>  "HardRestarting",
            Self::Aborted =>  "Aborted",
            Self::Unreachable =>  "Unreachable",
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum CommandStatus {
    Disallowed,
    Done,
    Failed,
    Busy,
}

impl FromStr for CommandStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "disallowed" => Ok(Self::Disallowed),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "busy" => Ok(Self::Busy),
            _ => Err(String::from("Unimplemented command status")) 
        }
    }
}

#[derive(Debug)]
pub struct Guard {
    pub id: String,
    pub miners: Vec<String>,
    pub board_type: GuardType,
    pub state: DeviceState,
    pub last_seen: NaiveDateTime,
}

#[derive(Debug)]
pub struct Switchboard {
    pub id: String,
    pub state: DeviceState,
    pub last_seen: NaiveDateTime,
}

/* Miner data for miners MQTT messages receiver loop */

#[derive(Debug)]
pub struct MinerData {
    pub last_received: Option<Instant>,
    pub name: String,
    pub energy_consumed: u64,
    pub phase: u8,
    pub power: f32,
}

#[derive(Debug)]
pub enum MinerAlert {
    PoweredOn,
    PoweredDown,
}

impl FromStr for MinerAlert {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PoweredOn" => Ok(Self::PoweredOn),
            "PoweredDown" => Ok(Self::PoweredDown),
            _ => Err(String::from("Unimplemented miner alert")) 
        }
    }
}

/* Energy data representation for channel */

#[derive(Debug, Clone)]
pub enum EnergyData {
    Switchboard {ts: NaiveDateTime, ec: [u64; 3], er: [u64; 3], tc: [f64; 3], tr: [f64; 3]},
    Miner {ts: NaiveDateTime, name: String, ec: u64, phase: u8, power: f32},
}

/* Data for main thread channel */
#[derive(Debug)]
pub enum GuardData {
    Alert {miner_id: String, alert: MinerAlert},
    Command {miner_id: String, command_status: CommandStatus, miner_state: MinerState},
    Configured,
    Ping,
    Started,
    State {miner_id: String, state: MinerState},

}

#[derive(Debug)]
pub enum UserCommands {
    Exclude,
    Include,
}

impl FromStr for UserCommands {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "exclude" => Ok(Self::Exclude),
            "include" => Ok(Self::Include),
            _ => Err(String::from("Unimplemented UserCommand")) 
        }
    }
}

impl ToString for UserCommands {
    fn to_string(&self) -> String {
        String::from( match self {
            Self::Exclude => "Exclude",
            Self::Include => "Include",
        })
    }
}

#[derive(Debug)]
pub enum Message {
    //ShellyAnnounce,
    Energy(EnergyData),
    Guard {guard_id: String, ts: NaiveDateTime, data: GuardData},
    Plug {plug_id: String, ts: NaiveDateTime, is_on: bool},
    User {miner_id: String, command: UserCommands},
}