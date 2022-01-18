
use chrono::{NaiveDate, NaiveDateTime, Utc, Datelike};
use postgres::{Client, Config, Error, NoTls, GenericClient};
use std::collections::{HashSet};
use std::sync::mpsc::{self, Sender, Receiver};
mod queries;
use super::structs;


fn next_month(date: NaiveDateTime) -> NaiveDateTime {
    let (month, year) = if date.month() == 12 {
        (1, date.year() + 1)
    } else {
        (date.month() + 1, date.year())
    };
    
    return NaiveDate::from_ymd(year, month, 1).and_hms( 0, 0, 0); 
}

pub fn check_db_schema(client: &mut Client, (period_start, period_end): (NaiveDateTime, NaiveDateTime)) {
    println!("Get db schema, and check all tables for contract billing period");
    let mut tables = HashSet::new();

    let rows = client.query(queries::GET_ALL_TABLES, &[]).unwrap_or_else(|error_msg| {
        eprintln!("{}", error_msg);
        std::process::exit(1);
    });
    
    for row in rows {
        let table: String = row.get("table_name");
        tables.insert(table);
    }

    println!("Got db schema, start checking tables for every month");

    let mut month = period_start;

    while month < period_end {
        let table = format!("switchboard_{}_{:02}", month.year(), month.month());
        if let None = tables.get(&table) {
            let query = queries::create_switchboard_table(month.year(), month.month());
            client.execute(&query, &[]).unwrap_or_else(|error_msg| {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            });
            println!("{} created", table);
        }

        let table = format!("miners_{}_{:02}", month.year(), month.month());
        if let None = tables.get(&table) {
            let query = queries::create_miner_table(month.year(), month.month());
            client.execute(&query, &[]).unwrap_or_else(|error_msg| {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            });
            println!("{} created", table);
        }

        println!("{} checked at all", month);
        month = next_month(month);
    }
}

pub fn get_switchboard_params(client: &mut Client, period_start: NaiveDateTime) -> Option<([f64; 3], [f64; 3])> {
    let mut month = period_start;
    let now = Utc::now().naive_utc();

    while month <= now {
        let query = queries::get_first_row(month.year(), month.month());
        let start_energy_state = client.query(&query, &[]).unwrap_or_else(|error_msg| {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        });

        if let Some(row) = start_energy_state.iter().next() {
            let total_consumed: [f64; 3] = [
                row.get("total_consumed_wh_0"),
                row.get("total_consumed_wh_1"),
                row.get("total_consumed_wh_2"),
            ];
            let total_returned: [f64; 3] = [
                row.get("total_returned_wh_0"),
                row.get("total_returned_wh_1"),
                row.get("total_returned_wh_2"),
            ];

            return Some((total_consumed, total_returned));
        }

        println!("{}, {:?}", month, start_energy_state);
        month = next_month(month);
    }
    
    None
}

pub fn get_miners_consumption(client: &mut Client, period_start: NaiveDateTime) -> [u64; 3] {
    let mut month = period_start;
    let now = Utc::now().naive_utc();
    let mut consumption = [0; 3];


    while month <= now {
        for phase in 0..3 {
            let query = queries::get_month_miner_consumption(month.year(), month.month(), phase);
            let result = client.query(&query, &[]).unwrap_or_else(|error_msg| {
                eprintln!("{}", error_msg);
                std::process::exit(1);
            });

            if let Some(row) = result.first() {
                let month_sum: i64 = row.get("sum");
                consumption[phase as usize] += month_sum as u64;
            }
        }
        

        month = next_month(month);
    }
    return consumption;
    
}

pub fn insert_energy_data(db_config: Config, rx: Receiver<structs::EnergyData>) {
    use super::structs::EnergyData;

    let mut client = match db_config.connect(NoTls) {
        Ok(conn) => conn,
        Err(error) => {
            eprintln!("Database test connection: {}", error.to_string());
            std::process::exit(1)
        },
    };

    while let Ok(item) = rx.recv() {
        match item {
            EnergyData::Switchboard{ts, ec, er, tc, tr} => {
                println!("DB energy thread: received switchboard data from channel.");
                let query = queries::insert_switchboard_row(ts.year(), ts.month());

                if let Err(error_msg)  = client.execute(
                    &query,
             &[
                        &ts,
                        &(ec[0] as i64), &(ec[1] as i64), &(ec[2] as i64),
                        &(er[0] as i64), &(er[1] as i64), &(er[2] as i64),
                        &tc[0], &tc[1], &tc[2],
                        &tr[0], &tr[1], &tr[2],
                    ]
                ) {
                    eprintln!("Inserting switchboard row error: {}", error_msg);
                }
            },
            EnergyData::Miner{ts, name,  ec, phase, power} => {
                println!("DB energy thread: received miner data from channel.");

                let query = queries::insert_miners_row(ts.year(), ts.month());

                if let Err(error_msg)  = client.execute(&query,
                 &[&ts, &name, &(ec as i64), &(phase as i16), &power]
                ) {
                    eprintln!("Inserting miner row error: {}", error_msg);
                }
            },
        }
    }

    println!("Inserting energy data into database thread is exiting, channel is closed");
}