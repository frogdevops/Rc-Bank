use std::sync::Arc;
use bank_domain::{NewUser, Users, UsersError, UsersID};
use oracledb::{OracleTimestamp, Pool};
use crate::helpers::oracle_ts_to_chrono;

#[derive(Clone)]
pub struct UsersRepository {
    pool: Arc<Pool>,
}

impl UsersRepository {
    pub fn new(pool: Arc<Pool>) -> Self {
        UsersRepository { pool }
    }

    pub async fn insert(&self, new_user: NewUser) -> Result<Users, UsersError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;

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

            conn.commit()
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;

            let row = conn
                .query_row_named(
                    "SELECT user_id, created_at, updated_at FROM users \
                     WHERE user_name = :user_name AND password_hash = :password_hash \
                     ORDER BY user_id DESC FETCH FIRST 1 ROW ONLY",
                    &[
                        ("user_name", &new_user.user_name),
                        ("password_hash", &new_user.password.hash_str()),
                    ],
                )
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;

            let id: i64 = row
                .get("user_id")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let created_at_raw: OracleTimestamp = row
                .get("created_at")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let updated_at_raw: OracleTimestamp = row
                .get("updated_at")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;

            let created_at = oracle_ts_to_chrono(&created_at_raw)
                .map_err(UsersError::DatabaseError)?;
            let updated_at = oracle_ts_to_chrono(&updated_at_raw)
                .map_err(UsersError::DatabaseError)?;

            Ok(Users {
                user_id: UsersID::from_db(id),
                name: new_user.name,
                user_name: new_user.user_name,
                email: new_user.email,
                password: new_user.password,
                created_at,
                updated_at,
            })
        })
        .await
        .map_err(|e| UsersError::DatabaseError(e.to_string()))?
    }
}
