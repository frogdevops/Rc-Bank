use std::str::FromStr;
use std::sync::Arc;
use bank_domain::{AccountError, AccountID, AccountNumber, AccountType, Accounts, Balance, NewAccount, Status , UsersID};
use oracledb::{OracleTimestamp, Pool};
use crate::helpers::oracle_ts_to_chrono;

#[derive(Clone)]
pub struct AccountsRepository {
    pool: Arc<Pool>,
}

impl AccountsRepository {
    pub fn new(pool: Arc<Pool>) -> Self {
        AccountsRepository { pool }
    }

    pub async fn insert(&self, new_account: NewAccount) -> Result<Accounts, AccountError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;

            let user_id_val = new_account.user_id.value();
            let account_type_str = new_account.account_type.as_str();

            let mut results = conn.execute_named(
                "INSERT INTO accounts (account_number, account_type, user_id, status) \
                 VALUES (generate_account_number(), :account_type, :user_id, 'ACTIVE')",
                &[
                    ("account_type", &account_type_str),
                    ("user_id", &user_id_val),
                ],
            )
            .map_err(|e| AccountError::DatabaseError(e.to_string()))?;

            conn.commit()
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
	        let rows = results.returned_row()
		        .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
            let account_id: i64 = rows
                .get("account_id")
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
            let account_number_raw: String = rows
                .get("account_number")
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
            let account_type_raw: String = rows
                .get("account_type")
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
            let status_raw: String = rows
                .get("status")
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
            let created_at_raw: OracleTimestamp = rows
                .get("created_at")
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;
            let updated_at_raw: OracleTimestamp = rows
                .get("updated_at")
                .map_err(|e| AccountError::DatabaseError(e.to_string()))?;

            let created_at = oracle_ts_to_chrono(&created_at_raw)
                .map_err(AccountError::DatabaseError)?;
            let updated_at = oracle_ts_to_chrono(&updated_at_raw)
                .map_err(AccountError::DatabaseError)?;

            let account_type = AccountType::from_str(&account_type_raw)?;
            let status = Status::from_str(&status_raw)?;

            Ok(Accounts {
                account_id: AccountID::from_db(account_id),
                account_number: AccountNumber::from_db(account_number_raw),
                account_type,
                user_id: UsersID::from_db(user_id_val),
                balance: Balance::zero(),
                status,
                created_at,
                updated_at,
            })
        })
        .await
        .map_err(|e| AccountError::DatabaseError(e.to_string()))?
    }
}
