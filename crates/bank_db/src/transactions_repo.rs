use std::str::FromStr;
use std::sync::Arc;
use bank_domain::{
    AccountID, AccountNumber, AccountType, Accounts, Balance, Status, TransactionError,
    TransactionID, TransactionType, Transactions, UsersID,
};
use oracledb::{OracleTimestamp, Pool};
use crate::helpers::oracle_ts_to_chrono;

#[derive(Clone)]
pub struct TransactionsRepository {
    pool: Arc<Pool>,
}

impl TransactionsRepository {
    pub fn new(pool: Arc<Pool>) -> Self {
        TransactionsRepository { pool }
    }

    pub async fn get_balance(&self, account_id: AccountID) -> Result<Balance, TransactionError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let balance_cents = query_balance_cents(&conn, account_id.value())?;
            Ok(Balance::from_cents(balance_cents))
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }

    pub async fn find_account_by_id(&self, account_id: AccountID) -> Result<Accounts, TransactionError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let account_id_val = account_id.value();
            let row = conn
                .query_row_named(
                    "SELECT a.account_id, a.account_number, a.account_type, a.user_id, a.status, \
                            a.created_at, a.updated_at, NVL(SUM(t.amount_cents), 0) AS balance_cents \
                     FROM accounts a \
                     LEFT JOIN transactions t ON a.account_id = t.account_id \
                     WHERE a.account_id = :account_id \
                     GROUP BY a.account_id, a.account_number, a.account_type, a.user_id, a.status, a.created_at, a.updated_at",
                    &[("account_id", &account_id_val)],
                )
                .map_err(|_| TransactionError::AccountNotFound)?;

            parse_account_row(row)
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }

    pub async fn find_account_by_number(
        &self,
        account_number: &AccountNumber,
    ) -> Result<Accounts, TransactionError> {
        let pool = self.pool.clone();
        let acc_num_str = account_number.as_str().to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let row = conn
                .query_row_named(
                    "SELECT a.account_id, a.account_number, a.account_type, a.user_id, a.status, \
                            a.created_at, a.updated_at, NVL(SUM(t.amount_cents), 0) AS balance_cents \
                     FROM accounts a \
                     LEFT JOIN transactions t ON a.account_id = t.account_id \
                     WHERE a.account_number = :account_number \
                     GROUP BY a.account_id, a.account_number, a.account_type, a.user_id, a.status, a.created_at, a.updated_at",
                    &[("account_number", &acc_num_str)],
                )
                .map_err(|_| TransactionError::AccountNotFound)?;

            parse_account_row(row)
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }

    pub async fn deposit(
        &self,
        account_id: AccountID,
        amount_cents: i64,
    ) -> Result<Transactions, TransactionError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let tx = insert_transaction_record(
                &conn,
                account_id.value(),
                amount_cents,
                TransactionType::Deposit,
            )
            .map_err(|e| {
                let _ = conn.rollback();
                e
            })?;

            conn.commit().map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
            Ok(tx)
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }

    pub async fn withdraw(
        &self,
        account_id: AccountID,
        amount_cents: i64,
    ) -> Result<Transactions, TransactionError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let account_id_val = account_id.value();
            let current_balance = query_balance_cents(&conn, account_id_val)?;

            if current_balance < amount_cents {
                return Err(TransactionError::InsufficientFunds);
            }

            let tx = insert_transaction_record(
                &conn,
                account_id_val,
                -amount_cents,
                TransactionType::Withdrawal,
            )
            .map_err(|e| {
                let _ = conn.rollback();
                e
            })?;

            conn.commit().map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
            Ok(tx)
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }

    pub async fn transfer(
        &self,
        from_account_id: AccountID,
        to_account_id: AccountID,
        amount_cents: i64,
    ) -> Result<(Transactions, Transactions), TransactionError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let from_id = from_account_id.value();
            let to_id = to_account_id.value();

            let sender_balance = query_balance_cents(&conn, from_id)?;
            if sender_balance < amount_cents {
                return Err(TransactionError::InsufficientFunds);
            }

            let tx_out = insert_transaction_record(
                &conn,
                from_id,
                -amount_cents,
                TransactionType::TransferOut,
            )
            .map_err(|e| {
                let _ = conn.rollback();
                TransactionError::DatabaseError(format!("Debit failed: {}", e))
            })?;

            let tx_in = insert_transaction_record(
                &conn,
                to_id,
                amount_cents,
                TransactionType::TransferIn,
            )
            .map_err(|e| {
                let _ = conn.rollback();
                TransactionError::DatabaseError(format!("Credit failed: {}", e))
            })?;

            if let Err(e) = conn.commit() {
                let _ = conn.rollback();
                return Err(TransactionError::DatabaseError(format!("Commit failed: {}", e)));
            }

            Ok((tx_out, tx_in))
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }

    pub async fn get_statement(
        &self,
        account_id: AccountID,
        limit: i64,
    ) -> Result<Vec<Transactions>, TransactionError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let account_id_val = account_id.value();
            let cursor = conn
                .query_named(
                    "SELECT transaction_id, account_id, amount_cents, transaction_type, previous_hash, current_hash, created_at \
                     FROM transactions WHERE account_id = :account_id \
                     ORDER BY transaction_id DESC FETCH FIRST :limit ROWS ONLY",
                    &[
                        ("account_id", &account_id_val),
                        ("limit", &limit),
                    ],
                )
                .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

            let mut transactions = Vec::new();
            for row_result in cursor {
                let row = row_result.map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
                transactions.push(parse_transaction_row(row)?);
            }

            Ok(transactions)
        })
        .await
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?
    }
}

fn query_balance_cents(
    conn: &oracledb::Connection,
    account_id: i64,
) -> Result<i64, TransactionError> {
    let row = conn
        .query_row_named(
            "SELECT NVL(SUM(amount_cents), 0) AS balance_cents \
             FROM transactions WHERE account_id = :account_id",
            &[("account_id", &account_id)],
        )
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

    row.get("balance_cents")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))
}

fn insert_transaction_record(
    conn: &oracledb::Connection,
    account_id: i64,
    amount_cents: i64,
    tx_type: TransactionType,
) -> Result<Transactions, TransactionError> {
    let tx_type_str = tx_type.as_str().to_string();

    let out_tx_id: i64 = 0;
    let out_acc_id: i64 = 0;
    let out_amount: i64 = 0;
    let out_type = " ".repeat(20);
    let out_prev_hash: Option<String> = Some(" ".repeat(64));
    let out_curr_hash = " ".repeat(64);
    let out_created_at = OracleTimestamp::default();

    let mut result = conn
        .execute_named(
            "INSERT INTO transactions (account_id, amount_cents, transaction_type) \
             VALUES (:account_id, :amount_cents, :tx_type) \
             RETURNING transaction_id, account_id, amount_cents, transaction_type, previous_hash, current_hash, created_at \
             INTO :out_tx_id, :out_acc_id, :out_amount, :out_type, :out_prev_hash, :out_curr_hash, :out_created_at",
            &[
                ("account_id", &account_id),
                ("amount_cents", &amount_cents),
                ("tx_type", &tx_type_str),
                ("out_tx_id", &out_tx_id),
                ("out_acc_id", &out_acc_id),
                ("out_amount", &out_amount),
                ("out_type", &out_type),
                ("out_prev_hash", &out_prev_hash),
                ("out_curr_hash", &out_curr_hash),
                ("out_created_at", &out_created_at),
            ],
        )
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("ORA-20001") || err_str.contains("ACCOUNT_NOT_ACTIVE") {
                TransactionError::AccountNotActive
            } else {
                TransactionError::DatabaseError(err_str)
            }
        })?;

    let row = result
        .returned_row()
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

    parse_transaction_row(row)
}

fn parse_account_row(row: oracledb::Row) -> Result<Accounts, TransactionError> {
    let id: i64 = row
        .get("account_id")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let account_number_raw: String = row
        .get("account_number")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let account_type_raw: String = row
        .get("account_type")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let user_id_raw: i64 = row
        .get("user_id")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let status_raw: String = row
        .get("status")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let balance_cents: i64 = row
        .get("balance_cents")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let created_at_raw: OracleTimestamp = row
        .get("created_at")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let updated_at_raw: OracleTimestamp = row
        .get("updated_at")
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

    let created_at = oracle_ts_to_chrono(&created_at_raw)
        .map_err(TransactionError::DatabaseError)?;
    let updated_at = oracle_ts_to_chrono(&updated_at_raw)
        .map_err(TransactionError::DatabaseError)?;

    let account_type = AccountType::from_str(&account_type_raw)
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let status = Status::from_str(&status_raw)
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

    Ok(Accounts {
        account_id: AccountID::from_db(id),
        account_number: AccountNumber::from_db(account_number_raw),
        account_type,
        user_id: UsersID::from_db(user_id_raw),
        balance: Balance::from_cents(balance_cents),
        status,
        created_at,
        updated_at,
    })
}

fn parse_transaction_row(row: oracledb::Row) -> Result<Transactions, TransactionError> {
    let tx_id: i64 = row
        .get("transaction_id")
        .or_else(|_| row.get("out_tx_id"))
        .or_else(|_| row.get(0))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let acc_id: i64 = row
        .get("account_id")
        .or_else(|_| row.get("out_acc_id"))
        .or_else(|_| row.get(1))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let amount_cents: i64 = row
        .get("amount_cents")
        .or_else(|_| row.get("out_amount"))
        .or_else(|_| row.get(2))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let tx_type_raw: String = row
        .get("transaction_type")
        .or_else(|_| row.get("out_type"))
        .or_else(|_| row.get(3))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let prev_hash: Option<String> = row
        .get("previous_hash")
        .or_else(|_| row.get("out_prev_hash"))
        .or_else(|_| row.get(4))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let curr_hash: String = row
        .get("current_hash")
        .or_else(|_| row.get("out_curr_hash"))
        .or_else(|_| row.get(5))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;
    let created_at_raw: OracleTimestamp = row
        .get("created_at")
        .or_else(|_| row.get("out_created_at"))
        .or_else(|_| row.get(6))
        .map_err(|e| TransactionError::DatabaseError(e.to_string()))?;

    let created_at = oracle_ts_to_chrono(&created_at_raw)
        .map_err(TransactionError::DatabaseError)?;
    let transaction_type = TransactionType::from_str(&tx_type_raw)?;

    Ok(Transactions {
        transaction_id: TransactionID::from_db(tx_id),
        account_id: AccountID::from_db(acc_id),
        amount_cents,
        transaction_type,
        previous_hash: prev_hash,
        current_hash: curr_hash,
        created_at,
    })
}
