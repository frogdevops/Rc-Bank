use std::fmt;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::accounts::AccountID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionID(i64);

impl TransactionID {
    pub fn from_db(value: i64) -> Self {
        TransactionID(value)
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    TransferIn,
    TransferOut,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::Deposit => "DEPOSIT",
            TransactionType::Withdrawal => "WITHDRAWAL",
            TransactionType::TransferIn => "TRANSFER_IN",
            TransactionType::TransferOut => "TRANSFER_OUT",
        }
    }
}

impl FromStr for TransactionType {
    type Err = TransactionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "DEPOSIT" => Ok(TransactionType::Deposit),
            "WITHDRAWAL" => Ok(TransactionType::Withdrawal),
            "TRANSFER_IN" => Ok(TransactionType::TransferIn),
            "TRANSFER_OUT" => Ok(TransactionType::TransferOut),
            other => Err(TransactionError::InvalidAmount(format!(
                "unknown transaction type: {}",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amount(i64);

impl Amount {
    pub const MAX_AMOUNT_CENTS: i64 = 1_000_000_000_00; // $1 Billion in cents

    pub fn new(cents: i64) -> Result<Self, TransactionError> {
        if cents <= 0 {
            return Err(TransactionError::InvalidAmount(
                "amount must be greater than zero".into(),
            ));
        }
        if cents > Self::MAX_AMOUNT_CENTS {
            return Err(TransactionError::InvalidAmount(
                "amount exceeds maximum transaction limit".into(),
            ));
        }
        Ok(Amount(cents))
    }

    pub fn cents(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transactions {
    pub transaction_id: TransactionID,
    pub account_id: AccountID,
    pub amount_cents: i64,
    pub transaction_type: TransactionType,
    pub previous_hash: Option<String>,
    pub current_hash: String,
    pub created_at: DateTime<Utc>,
}

impl Transactions {
    pub fn calculate_hash(
        previous_hash: Option<&str>,
        account_id: AccountID,
        amount_cents: i64,
        transaction_type: TransactionType,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        let payload = format!(
            "{}|{}|{}|{}",
            previous_hash.unwrap_or("GENESIS"),
            account_id.value(),
            amount_cents,
            transaction_type.as_str()
        );
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTransaction {
    pub account_id: AccountID,
    pub amount_cents: i64,
    pub transaction_type: TransactionType,
}

impl NewTransaction {
    pub fn new(account_id: AccountID, amount_cents: i64, transaction_type: TransactionType) -> Self {
        NewTransaction {
            account_id,
            amount_cents,
            transaction_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    InsufficientFunds,
    InvalidAmount(String),
    AccountNotFound,
    AccountNotActive,
    SelfTransferNotAllowed,
    UnauthorizedAccountAccess,
    DatabaseError(String),
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionError::InsufficientFunds => write!(f, "insufficient funds for transaction"),
            TransactionError::InvalidAmount(msg) => write!(f, "invalid amount: {}", msg),
            TransactionError::AccountNotFound => write!(f, "account not found"),
            TransactionError::AccountNotActive => write!(f, "account is not active"),
            TransactionError::SelfTransferNotAllowed => {
                write!(f, "cannot transfer funds to the same account")
            }
            TransactionError::UnauthorizedAccountAccess => {
                write!(f, "you do not own or have access to this account")
            }
            TransactionError::DatabaseError(msg) => write!(f, "database error: {}", msg),
        }
    }
}

impl std::error::Error for TransactionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_positive_valid() {
        assert!(Amount::new(100).is_ok());      // $1.00
        assert!(Amount::new(50000).is_ok());    // $500.00
        assert_eq!(Amount::new(50000).unwrap().cents(), 50000);
    }

    #[test]
    fn test_amount_zero_or_negative_rejected() {
        let err_zero = Amount::new(0).unwrap_err();
        assert_eq!(err_zero, TransactionError::InvalidAmount("amount must be greater than zero".into()));

        let err_neg = Amount::new(-100).unwrap_err();
        assert_eq!(err_neg, TransactionError::InvalidAmount("amount must be greater than zero".into()));
    }

    #[test]
    fn test_amount_exceeding_max_limit_rejected() {
        let too_much = Amount::MAX_AMOUNT_CENTS + 1;
        let err = Amount::new(too_much).unwrap_err();
        assert_eq!(err, TransactionError::InvalidAmount("amount exceeds maximum transaction limit".into()));
    }


    #[test]
    fn test_transaction_type_parsing() {
        assert_eq!(TransactionType::from_str("DEPOSIT").unwrap(), TransactionType::Deposit);
        assert_eq!(TransactionType::from_str("deposit").unwrap(), TransactionType::Deposit);
        assert_eq!(TransactionType::from_str("WITHDRAWAL").unwrap(), TransactionType::Withdrawal);
        assert_eq!(TransactionType::from_str("TRANSFER_IN").unwrap(), TransactionType::TransferIn);
        assert_eq!(TransactionType::from_str("TRANSFER_OUT").unwrap(), TransactionType::TransferOut);
    }

    #[test]
    fn test_transaction_type_invalid_rejected() {
        assert!(TransactionType::from_str("UNKNOWN").is_err());
        assert!(TransactionType::from_str("").is_err());
    }

    #[test]
    fn test_transaction_type_as_str() {
        assert_eq!(TransactionType::Deposit.as_str(), "DEPOSIT");
        assert_eq!(TransactionType::Withdrawal.as_str(), "WITHDRAWAL");
        assert_eq!(TransactionType::TransferIn.as_str(), "TRANSFER_IN");
        assert_eq!(TransactionType::TransferOut.as_str(), "TRANSFER_OUT");
    }

    // ==========================================
    // HASH CHAINING TESTS
    // ==========================================
    #[test]
    fn test_transaction_hash_chain_calculation() {
        let acc_id = AccountID::from_db(10);
        let genesis_hash = Transactions::calculate_hash(None, acc_id, 10000, TransactionType::Deposit);
        assert_eq!(genesis_hash.len(), 64, "SHA-256 hash must be 64 hex characters");

        // Next transaction chained to previous hash
        let second_hash = Transactions::calculate_hash(
            Some(&genesis_hash),
            acc_id,
            -2500,
            TransactionType::Withdrawal,
        );
        assert_eq!(second_hash.len(), 64);
        assert_ne!(genesis_hash, second_hash);
    }
}
