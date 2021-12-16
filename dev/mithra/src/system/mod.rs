
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug)]
pub struct Miner {
    pub name: String,
    pub plug: String,
    pub guard: String,
    pub pinout: u32,
    pub power_consumption: Option<u32>, // Watts
}

#[derive(Debug)]
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
            _ => Err(String::from("Unimplemented board type")) 
        }
    }
}

#[derive(Debug)]
pub struct Guard {
    pub name: String,
    pub miners: Vec<String>,
    pub board_type: GuardType,
}

#[derive(Debug)]
pub struct System {
    pub guards: HashMap<String, Guard>,
    pub miners: HashMap<String, Miner>,
}