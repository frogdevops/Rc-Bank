use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use oracledb::OracleTimestamp;

pub fn oracle_ts_to_chrono(ts: &OracleTimestamp) -> Result<DateTime<Utc>, String> {
    let naive_date = NaiveDate::from_ymd_opt(ts.year() as i32, ts.month() as u32, ts.day() as u32)
        .ok_or_else(|| "invalid date from Oracle".to_string())?;

    let naive_datetime = naive_date
        .and_hms_nano_opt(
            ts.hour() as u32,
            ts.minute() as u32,
            ts.second() as u32,
            ts.nanoseconds(),
        )
        .ok_or_else(|| "invalid time from Oracle".to_string())?;

    Ok(Utc.from_utc_datetime(&naive_datetime))
}
