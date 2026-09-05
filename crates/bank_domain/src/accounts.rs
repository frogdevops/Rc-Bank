use std::fmt;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::errors::AccountError;
use crate::users::UsersID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountID(i64);

impl AccountID {
    pub fn from_db(value: i64) -> Self {
        AccountID(value)
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountNumber(String);

impl AccountNumber {
    pub fn from_db(raw: String) -> Self {
        AccountNumber(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for AccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountType {
    Savings,
    Checking,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Savings => "SAVINGS",
            AccountType::Checking => "CHECKING",
        }
    }
}

impl FromStr for AccountType {
    type Err = AccountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "SAVINGS" => Ok(AccountType::Savings),
            "CHECKING" => Ok(AccountType::Checking),
            other => Err(AccountError::InvalidInput(format!("unknown account type: {}", other))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Active,
    Frozen,
    Closed,
    Suspended,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Active => "ACTIVE",
            Status::Frozen => "FROZEN",
            Status::Closed => "CLOSED",
            Status::Suspended => "SUSPENDED",
        }
    }
}

impl FromStr for Status {
    type Err = AccountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "ACTIVE" => Ok(Status::Active),
            "FROZEN" => Ok(Status::Frozen),
            "CLOSED" => Ok(Status::Closed),
            "SUSPENDED" => Ok(Status::Suspended),
            other => Err(AccountError::InvalidInput(format!("unknown status: {}", other))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Balance(i64);

impl Balance {
    pub fn zero() -> Self {
        Balance(0)
    }

    pub fn from_cents(cents: i64) -> Self {
        Balance(cents)
    }

    pub fn cents(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Accounts {
    pub account_id: AccountID,
    pub account_number: AccountNumber,
    pub account_type: AccountType,
    pub user_id: UsersID,
    pub balance: Balance,
    pub status: Status,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAccount {
    pub user_id: UsersID,
    pub account_type: AccountType,
}

impl NewAccount {
    pub fn new(user_id: UsersID, account_type: AccountType) -> Self {
        NewAccount {
            user_id,
            account_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
	
    #[test]
    fn test_account_type_parsing_case_insensitive() {
        assert_eq!(AccountType::from_str("SAVINGS").unwrap(), AccountType::Savings);
        assert_eq!(AccountType::from_str("savings").unwrap(), AccountType::Savings);
        assert_eq!(AccountType::from_str("  Savings  ").unwrap(), AccountType::Savings);

        assert_eq!(AccountType::from_str("CHECKING").unwrap(), AccountType::Checking);
        assert_eq!(AccountType::from_str("checking").unwrap(), AccountType::Checking);
        assert_eq!(AccountType::from_str("  Checking  ").unwrap(), AccountType::Checking);
    }

    #[test]
    fn test_account_type_invalid_rejected() {
        let err = AccountType::from_str("INVESTMENT").unwrap_err();
        assert_eq!(err, AccountError::InvalidInput("unknown account type: INVESTMENT".into()));

        let err_empty = AccountType::from_str("").unwrap_err();
        assert_eq!(err_empty, AccountError::InvalidInput("unknown account type: ".into()));
    }

    #[test]
    fn test_account_type_as_str_representation() {
        assert_eq!(AccountType::Savings.as_str(), "SAVINGS");
        assert_eq!(AccountType::Checking.as_str(), "CHECKING");
    }
	
    #[test]
    fn test_status_parsing_all_variants() {
        assert_eq!(Status::from_str("ACTIVE").unwrap(), Status::Active);
        assert_eq!(Status::from_str("active").unwrap(), Status::Active);
        assert_eq!(Status::from_str("FROZEN").unwrap(), Status::Frozen);
        assert_eq!(Status::from_str("CLOSED").unwrap(), Status::Closed);
        assert_eq!(Status::from_str("SUSPENDED").unwrap(), Status::Suspended);
    }

    #[test]
    fn test_status_invalid_rejected() {
        let err = Status::from_str("DELETED").unwrap_err();
        assert_eq!(err, AccountError::InvalidInput("unknown status: DELETED".into()));
    }
	
    #[test]
    fn test_balance_zero_and_cents_preservation() {
        let zero = Balance::zero();
        assert_eq!(zero.cents(), 0);

        let hundred_dollars = Balance::from_cents(10000);
        assert_eq!(hundred_dollars.cents(), 10000);

        let negative_balance = Balance::from_cents(-500);
        assert_eq!(negative_balance.cents(), -500);
    }
	
    #[test]
    fn test_account_number_wrapping_and_display() {
        let raw = "00680306222204791821".to_string();
        let acc_num = AccountNumber::from_db(raw.clone());
        assert_eq!(acc_num.as_str(), "00680306222204791821");
        assert_eq!(format!("{}", acc_num), "00680306222204791821");
        assert_eq!(acc_num.into_inner(), raw);
    }
}
