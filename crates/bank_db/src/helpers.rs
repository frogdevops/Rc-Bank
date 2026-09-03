use chrono::{DateTime, NaiveDateTime, Utc};
use oracledb::OracleTimestamp;

pub fn oracle_ts_to_chrono(ts: &OracleTimestamp) -> Result<DateTime<Utc>, String> {
	let naive: NaiveDateTime = (*ts)
		.try_into()
		.map_err(|e: oracledb::Error| e.to_string())?;

	Ok(naive.and_utc())
}