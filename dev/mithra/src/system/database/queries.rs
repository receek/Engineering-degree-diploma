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
            energy_consumed_Wmin_0 bigint,
            energy_consumed_Wmin_1 bigint,
            energy_consumed_Wmin_2 bigint,
            energy_returned_Wmin_0 bigint,
            energy_returned_Wmin_1 bigint,
            energy_returned_Wmin_2 bigint,
            total_consumed_Wh_0 double precision,
            total_consumed_Wh_1 double precision,
            total_consumed_Wh_2 double precision,
            total_returned_Wh_0 double precision,
            total_returned_Wh_1 double precision,
            total_returned_Wh_2 double precision
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
            phase smallint,
            power_W real
        );",
        year, month
    )
}

pub fn create_miner_grid_table(year: i32, month: u32) -> String {
    format!(
        "CREATE TABLE miners_grid_{}_{:02} (
            ts timestamp PRIMARY KEY,
            energy_consumed_Wmin bigint,
            phase smallint
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

pub fn get_month_miner_consumption(year: i32, month: u32, phase: u32) -> String {
    format!(
        "SELECT CAST(COALESCE(SUM(energy_consumed_Wmin), 0) AS bigint) AS sum FROM miners_{}_{:02}
         WHERE phase = {};",
        year, month, phase
    )
}

pub fn get_month_miner_grid_consumption(year: i32, month: u32, phase: u32) -> String {
    format!(
        "SELECT CAST(COALESCE(SUM(energy_consumed_Wmin), 0) AS bigint) AS sum FROM miners_grid_{}_{:02}
         WHERE phase = {};",
        year, month, phase
    )
}

pub fn insert_switchboard_row(year: i32, month: u32) -> String {
    format!(
        "INSERT INTO switchboard_{}_{:02} VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);",
        year, month
    )
}

pub fn insert_miners_row(year: i32, month: u32) -> String {
    format!(
        "INSERT INTO miners_{}_{:02} VALUES ($1, $2, $3, $4, $5);",
        year, month
    )
}

pub fn insert_miners_grid_row(year: i32, month: u32) -> String {
    format!(
        "INSERT INTO miners_grid_{}_{:02} VALUES ($1, $2, $3);",
        year, month
    )
}