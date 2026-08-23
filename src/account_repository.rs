use crate::account::{Account, AccountError, AccountID, Name, NewAccount, Password};
use oracledb::{Connection, OracleTimestamp};
use std::sync::{Arc, Mutex};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};

pub struct AccountRepository {
	conn: Arc<Mutex<Connection>>,
}

impl AccountRepository {
	pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
		AccountRepository { conn }
	}

	pub async fn insert(&self, new_account: NewAccount) -> Result<Account, AccountError> {
		let conn = self.conn.clone();

		tokio::task::spawn_blocking(move || {
			let conn = conn.lock().map_err(|e| AccountError::DatabaseError(e.to_string()))?;
			conn
				.execute(
					"INSERT INTO accounts (name, password_hash) \
                     VALUES (:1, :2)",
					&[
						&new_account.name.as_str(),
						&new_account.password.hash_str(),
					],
				)
				.map_err(|e| AccountError::DatabaseError(e.to_string()))?;

			conn.commit().map_err(|e| AccountError::DatabaseError(e.to_string()))?; 

			let row = conn
				.query_row(
					"SELECT account_id, created_at, updated_at FROM accounts \
                 WHERE name = :1 AND password_hash = :2 \
                 ORDER BY account_id DESC FETCH FIRST 1 ROW ONLY",
					&[&new_account.name.as_str(), &new_account.password.hash_str()],
				)
				.map_err(|e| AccountError::DatabaseError(e.to_string()))?;


			let id = row.get(0).map_err(|e| AccountError::DatabaseError(e.to_string()))?;
				let created_at_raw:OracleTimestamp = row.get(1).map_err(|e| AccountError::DatabaseError(e.to_string()))?;
				let updated_at_raw:OracleTimestamp = row.get(2).map_err(|e| AccountError::DatabaseError(e.to_string()))?;

				let created_at = oracle_ts_to_chrono(&created_at_raw)?;
				let updated_at = oracle_ts_to_chrono(&updated_at_raw)?;


				Ok(Account {
					account_id: AccountID::from_db(id),
					name: new_account.name,
					password: new_account.password,
					created_at,
					updated_at
				})
		})
			.await
			.map_err(|e| AccountError::DatabaseError(e.to_string()))?
	}
}

fn oracle_ts_to_chrono(ts: &OracleTimestamp) -> Result<DateTime<Utc>, AccountError> {
	let naive_date = NaiveDate::from_ymd_opt(ts.year() as i32, ts.month() as u32, ts.day() as u32)
		.ok_or_else(|| AccountError::DatabaseError("invalid date from Oracle".into()))?;

	let naive_datetime = naive_date
		.and_hms_nano_opt(
			ts.hour() as u32,
			ts.minute() as u32,
			ts.second() as u32,
			ts.nanoseconds(),
		)
		.ok_or_else(|| AccountError::DatabaseError("invalid time from Oracle".into()))?;

	Ok(Utc.from_utc_datetime(&naive_datetime))
}