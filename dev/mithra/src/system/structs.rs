use chrono::NaiveDateTime;
use std::{
    str::FromStr,
    time::Instant,
};

/* There is status enum for switchboard, guards and plugs */
#[derive(Debug, PartialEq)]
pub enum DeviceState {
    Available,
    ConfigExpired,
    Inaccessible,
    StartingUp,
}

#[allow(non_camel_case_types)]
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
    pub estimated_consumption: f32,
    pub power_consumption: Option<f32>, // Watts
    pub state: MinerState,
    pub target_state: Option<MinerState>,
    pub command_ts: Option<NaiveDateTime>,
    pub included: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MinerState {
    Undefined,
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
        match s.to_lowercase().as_str() {
            "undefined" => Ok(Self::Undefined),
            "poweredoff" => Ok(Self::PoweredOff),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "hardstopping" => Ok(Self::HardStopping), 
            "restarting" => Ok(Self::Restarting),
            "hardrestarting" => Ok(Self::HardRestarting),
            "aborted" => Ok(Self::Aborted),
            "unreachable" => Ok(Self::Unreachable),
            _ => Err(String::from("Unimplemented miner state")) 
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CommandStatus {
    Busy,
    Disallowed,
    Done,
    Failed,
    Undefined,
}

impl FromStr for CommandStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "busy" => Ok(Self::Busy),
            "disallowed" => Ok(Self::Disallowed),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "undefined" => Ok(Self::Undefined),
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
    PoweredOff,
}

impl FromStr for MinerAlert {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "poweredon" => Ok(Self::PoweredOn),
            "poweredoff" => Ok(Self::PoweredOff),
            _ => Err(String::from("Unimplemented miner alert")) 
        }
    }
}

/* Energy data representation for channels */

#[derive(Debug, Clone)]
pub enum EnergyData {
    Switchboard {ts: NaiveDateTime, ec: [u64; 3], er: [u64; 3], tc: [f64; 3], tr: [f64; 3]},
    Miner {ts: NaiveDateTime, name: String, ec: u64, phase: u8, power: f32},
    MinersGrid {ts: NaiveDateTime, ec: u64, phase: u8}
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

#[derive(Debug)]
pub enum Message {
    Energy(EnergyData),
    Guard {guard_id: String, ts: NaiveDateTime, data: GuardData},
    Plug {plug_id: String, ts: NaiveDateTime, is_on: bool},
    User {miner_id: String, command: UserCommands},
}