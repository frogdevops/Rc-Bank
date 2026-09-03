use std::sync::Arc;
use bank_domain::{Email, Name, NewUser, Password, Users, UsersError, UsersID};
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

           let mut result =  conn.execute_named(
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

           let rows = result.returned_row()
	           .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let id: i64 = rows
                .get("user_id")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let created_at_raw: OracleTimestamp = rows
                .get("created_at")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let updated_at_raw: OracleTimestamp = rows
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

    pub async fn find_by_user_name(&self, user_name: String) -> Result<Users, UsersError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;

            let row = conn
                .query_row_named(
                    "SELECT user_id, first_name, middle_name, last_name, email, password_hash, created_at, updated_at \
                     FROM users WHERE user_name = :user_name",
                    &[("user_name", &user_name)],
                )
                .map_err(|_| UsersError::NotFound)?;

            let id: i64 = row
                .get("user_id")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let first_name: String = row
                .get("first_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let middle_name: Option<String> = row
                .get("middle_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let last_name: String = row
                .get("last_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let email_raw: Option<String> = row
                .get("email")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let password_hash: String = row
                .get("password_hash")
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

            let name = Name::new(first_name, middle_name, last_name)?;
            let email = match email_raw {
                Some(em) => Some(Email::new(em)?),
                None => None,
            };

            Ok(Users {
                user_id: UsersID::from_db(id),
                name,
                user_name,
                email,
                password: Password::from_hash(password_hash),
                created_at,
                updated_at,
            })
        })
        .await
        .map_err(|e| UsersError::DatabaseError(e.to_string()))?
    }

    pub async fn find_by_user_id(&self, user_id: UsersID) -> Result<Users, UsersError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;

            let user_id_val = user_id.value();
            let row = conn
                .query_row_named(
                    "SELECT user_id, user_name, first_name, middle_name, last_name, email, password_hash, created_at, updated_at \
                     FROM users WHERE user_id = :user_id",
                    &[("user_id", &user_id_val)],
                )
                .map_err(|_| UsersError::NotFound)?;

            let id: i64 = row
                .get("user_id")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let user_name: String = row
                .get("user_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let first_name: String = row
                .get("first_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let middle_name: Option<String> = row
                .get("middle_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let last_name: String = row
                .get("last_name")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let email_raw: Option<String> = row
                .get("email")
                .map_err(|e| UsersError::DatabaseError(e.to_string()))?;
            let password_hash: String = row
                .get("password_hash")
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

            let name = Name::new(first_name, middle_name, last_name)?;
            let email = match email_raw {
                Some(em) => Some(Email::new(em)?),
                None => None,
            };

            Ok(Users {
                user_id: UsersID::from_db(id),
                name,
                user_name,
                email,
                password: Password::from_hash(password_hash),
                created_at,
                updated_at,
            })
        })
        .await
        .map_err(|e| UsersError::DatabaseError(e.to_string()))?
    }
}
