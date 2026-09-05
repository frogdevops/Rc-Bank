use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::{AccountID, AccountNumber, UsersID, Transactions};

/// The command payload published to NATS by the API layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferCommand {
    /// Unique correlation ID for end-to-end tracing
    pub correlation_id: String,
    /// Optional reply subject for request-reply flows
    pub reply_to: Option<String>,
    pub user_id: UsersID,
    pub from_account_id: AccountID,
    pub to_account_number: AccountNumber,
    pub amount_cents: i64,
    pub created_at: DateTime<Utc>,
}

/// The result payload replied by the NATS worker back to the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub correlation_id: String,
    pub success: bool,
    pub debit_transaction: Option<Transactions>,
    pub credit_transaction: Option<Transactions>,
    pub error_message: Option<String>,
}

impl TransferResult {
    pub fn ok(
        correlation_id: String,
        debit: Transactions,
        credit: Transactions,
    ) -> Self {
        Self {
            correlation_id,
            success: true,
            debit_transaction: Some(debit),
            credit_transaction: Some(credit),
            error_message: None,
        }
    }

    pub fn err(correlation_id: String, error_message: impl Into<String>) -> Self {
        Self {
            correlation_id,
            success: false,
            debit_transaction: None,
            credit_transaction: None,
            error_message: Some(error_message.into()),
        }
    }
}

/// The command payload published to NATS for deposit operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositCommand {
    pub correlation_id: String,
    pub reply_to: Option<String>,
    pub user_id: UsersID,
    pub account_id: AccountID,
    pub amount_cents: i64,
    pub created_at: DateTime<Utc>,
}

/// The command payload published to NATS for withdraw operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawCommand {
    pub correlation_id: String,
    pub reply_to: Option<String>,
    pub user_id: UsersID,
    pub account_id: AccountID,
    pub amount_cents: i64,
    pub created_at: DateTime<Utc>,
}

/// Generic result for single-transaction operations (deposit / withdraw).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyResult {
    pub correlation_id: String,
    pub success: bool,
    pub transaction: Option<Transactions>,
    pub error_message: Option<String>,
}

impl MoneyResult {
    pub fn ok(correlation_id: String, transaction: Transactions) -> Self {
        Self {
            correlation_id,
            success: true,
            transaction: Some(transaction),
            error_message: None,
        }
    }

    pub fn err(correlation_id: String, error_message: impl Into<String>) -> Self {
        Self {
            correlation_id,
            success: false,
            transaction: None,
            error_message: Some(error_message.into()),
        }
    }
}
