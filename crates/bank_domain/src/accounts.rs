use std::fmt;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::errors::AccountError;
use crate::users::UsersID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        match s.to_uppercase().as_str() {
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
        match s.to_uppercase().as_str() {
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
