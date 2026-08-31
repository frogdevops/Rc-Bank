use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsersError {
    NotFound,
    InvalidInput(String),
    DatabaseError(String),
}

impl fmt::Display for UsersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsersError::NotFound => write!(f, "user not found"),
            UsersError::InvalidInput(msg) => write!(f, "invalid input: {}", msg),
            UsersError::DatabaseError(msg) => write!(f, "database error: {}", msg),
        }
    }
}

impl std::error::Error for UsersError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    NotFound,
    InvalidInput(String),
    DatabaseError(String),
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountError::NotFound => write!(f, "account not found"),
            AccountError::InvalidInput(msg) => write!(f, "invalid input: {}", msg),
            AccountError::DatabaseError(msg) => write!(f, "database error: {}", msg),
        }
    }
}

impl std::error::Error for AccountError {}
