use crate::users::{Users, UsersError, UsersID, Name, NewUser, Password};
use oracledb::{Connection, OracleTimestamp};
use std::sync::{Arc, Mutex};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};

pub struct UsersRepository {
	conn: Arc<Mutex<Connection>>,
}

impl UsersRepository {
	pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
		UsersRepository { conn }
	}

	pub async fn insert(&self, new_user: NewUser) -> Result<Users, UsersError> {
		let conn = self.conn.clone();

		tokio::task::spawn_blocking(move || {
			let conn = conn.lock().map_err(|e| UsersError::DatabaseError(e.to_string()))?;
			conn.execute_named(
				"INSERT INTO users (first_name, middle_name, last_name, user_name, password_hash, email) \
                     VALUES (:first_name, :middle_name, :last_name, :user_name, :password_hash, :email)",
				&[
					("first_name", &new_user.name.first_name),
					("middle_name", &new_user.name.middle_name),
					("last_name", &new_user.name.last_name),
					("user_name", &new_user.user_name),
					("password_hash", &new_user.password.hash_str()),
					("email", &new_user.email.as_ref().map(|e| e.as_str())),
				],
			)
				.map_err(|e| UsersError::DatabaseError(e.to_string()))?;

			conn.commit().map_err(|e| UsersError::DatabaseError(e.to_string()))?;

			let row = conn
				.query_row(
					"SELECT user_id, created_at, updated_at, user_name FROM users \
                 WHERE user_name = :1 AND password_hash = :2 \
                 ORDER BY user_id DESC FETCH FIRST 1 ROW ONLY",
					&[&new_user.user_name, &new_user.password.hash_str()],
				)
				.map_err(|e| UsersError::DatabaseError(e.to_string()))?;


			let id = row.get(0).map_err(|e| UsersError::DatabaseError(e.to_string()))?;
				let created_at_raw:OracleTimestamp = row.get(1).map_err(|e| UsersError::DatabaseError(e.to_string()))?;
				let updated_at_raw:OracleTimestamp = row.get(2).map_err(|e| UsersError::DatabaseError(e.to_string()))?;

				let created_at = oracle_ts_to_chrono(&created_at_raw)?;
				let updated_at = oracle_ts_to_chrono(&updated_at_raw)?;


				Ok(Users {
					user_id: UsersID::from_db(id),
					name: new_user.name,
					user_name: new_user.user_name,
					email: new_user.email,
					password: new_user.password,
					created_at,
					updated_at
				})
		})
			.await
			.map_err(|e| UsersError::DatabaseError(e.to_string()))?
	}
}

fn oracle_ts_to_chrono(ts: &OracleTimestamp) -> Result<DateTime<Utc>, UsersError> {
	let naive_date = NaiveDate::from_ymd_opt(ts.year() as i32, ts.month() as u32, ts.day() as u32)
		.ok_or_else(|| UsersError::DatabaseError("invalid date from Oracle".into()))?;

	let naive_datetime = naive_date
		.and_hms_nano_opt(
			ts.hour() as u32,
			ts.minute() as u32,
			ts.second() as u32,
			ts.nanoseconds(),
		)
		.ok_or_else(|| UsersError::DatabaseError("invalid time from Oracle".into()))?;

	Ok(Utc.from_utc_datetime(&naive_datetime))
}