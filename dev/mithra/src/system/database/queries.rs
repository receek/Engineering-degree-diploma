pub static GET_ALL_TABLES: &str = "
SELECT table_name FROM information_schema.tables 
    WHERE 
        table_schema = 'public' AND
        table_type = 'BASE TABLE'
;";

pub fn create_switchboard_table(year: i32, month: u32) -> String {
    format!(
        "CREATE TABLE switchboard_{}_{:02} (
            ts timestamp PRIMARY KEY,
            energy_consumed_Wmin bigint,
            energy_returned_Wmin bigint,
            total_consumed_Wh double precision,
            total_returned_Wh double precision
        );",
        year, month
    )
}

pub fn create_miner_table(year: i32, month: u32) -> String {
    format!(
        "CREATE TABLE miners_{}_{:02} (
            ts timestamp PRIMARY KEY,
            name text,
            energy_consumed_Wmin bigint,
            power_W real
        );",
        year, month
    )
}

pub fn get_first_row(year: i32, month: u32) -> String {
    let table_name = format!("switchboard_{}_{:02}", year, month);
    format!(
        "SELECT * FROM {table}
        WHERE ts = (
            SELECT MIN(ts) FROM {table}
        );",
        table = table_name
    )
}

pub fn get_month_miner_consumption(year: i32, month: u32) -> String {
    format!(
        "SELECT CAST(COALESCE(SUM(energy_consumed_Wmin), 0) AS bigint) AS sum FROM miners_{}_{:02};",
        year, month
    )
}