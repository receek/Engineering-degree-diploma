
use chrono::{NaiveDate, NaiveDateTime, Utc, Datelike};
use postgres::{Client, Config, Error, NoTls};
use std::collections::{HashSet};

mod queries;

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

pub fn get_switchboard_params(client: &mut Client, period_start: NaiveDateTime) -> Option<(u64, u64)> {
    let mut month = period_start;
    let now = Utc::now().naive_utc();

    while month <= now {
        let query = queries::get_first_row(month.year(), month.month());
        let start_energy_state = client.query(&query, &[]).unwrap_or_else(|error_msg| {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        });

        if let Some(row) = start_energy_state.iter().next() {
            let total_consumed: f64 = row.get("total_consumed_wh");
            let total_returned: f64 = row.get("total_returned_wh");

            return Some((total_consumed as u64, total_returned as u64));
        }

        println!("{}, {:?}", month, start_energy_state);
        month = next_month(month);
    }
    
    None
}

pub fn get_miners_consumption(client: &mut Client, period_start: NaiveDateTime) -> u64 {
    let mut month = period_start;
    let now = Utc::now().naive_utc();
    let mut consumption = 0;


    while month <= now {
        let query = queries::get_month_miner_consumption(month.year(), month.month());
        let result = client.query(&query, &[]).unwrap_or_else(|error_msg| {
            eprintln!("{}", error_msg);
            std::process::exit(1);
        });

        if let Some(row) = result.first() {
            let month_sum: i64 = row.get("sum");
            consumption += month_sum as u64;
        }

        month = next_month(month);
    }
    return consumption;
    
}