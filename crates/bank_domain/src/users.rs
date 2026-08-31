use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use chrono::{DateTime, Utc};
use crate::errors::UsersError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    pub fn new(raw: String) -> Result<Self, UsersError> {
        let raw = raw.trim().to_string();

        if raw.is_empty() {
            return Err(UsersError::InvalidInput("email cannot be empty".into()));
        }
        if !raw.contains('@') {
            return Err(UsersError::InvalidInput("invalid email format".into()));
        }
        if raw.len() > 200 {
            return Err(UsersError::InvalidInput("email too long".into()));
        }

        Ok(Email(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
}

impl Name {
    pub fn new(first: String, middle: Option<String>, last: String) -> Result<Self, UsersError> {
        let first_name = Self::validate(first)?;
        let last_name = Self::validate(last)?;
        let middle_name = match middle {
            Some(m) => Some(Self::validate(m)?),
            None => None,
        };

        Ok(Name {
            first_name,
            middle_name,
            last_name,
        })
    }

    fn validate(raw: String) -> Result<String, UsersError> {
        if raw.is_empty() {
            return Err(UsersError::InvalidInput(String::from("name cannot be empty")));
        }
        if raw.chars().count() > 100 {
            return Err(UsersError::InvalidInput(String::from("name is too long")));
        }

        if !raw.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '\'') {
            return Err(UsersError::InvalidInput("name contains invalid characters".into()));
        }
        Ok(raw)
    }

    pub fn full_name(&self) -> String {
        match &self.middle_name {
            Some(m) => format!("{} {} {}", self.first_name, m, self.last_name),
            None => format!("{} {}", self.first_name, self.last_name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsersID(i64);

impl UsersID {
    pub fn from_db(value: i64) -> Self {
        UsersID(value)
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Password(String);

impl Password {
    pub fn new(raw: String) -> Result<Password, UsersError> {
        Self::validate_strength(&raw)?;

        let pepper = std::env::var("PASSWORD_PEPPER")
            .unwrap_or_else(|_| "default_test_pepper_for_unit_tests_32_bytes!".to_string());
        let peppered = format!("{}/{}", pepper, raw);

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(peppered.as_bytes(), &salt)
            .map_err(|_| UsersError::InvalidInput(String::from("invalid password")))?
            .to_string();

        Ok(Password(password_hash))
    }

    pub fn from_hash(hash: String) -> Self {
        Password(hash)
    }

    fn validate_strength(raw: &str) -> Result<(), UsersError> {
        if raw.chars().count() < 12 {
            return Err(UsersError::InvalidInput("password must be at least 12 characters".into()));
        }
        if raw.chars().count() > 128 {
            return Err(UsersError::InvalidInput("password too long".into()));
        }
        if !raw.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(UsersError::InvalidInput("password must contain an uppercase letter".into()));
        }
        if !raw.chars().any(|c| c.is_ascii_lowercase()) {
            return Err(UsersError::InvalidInput("password must contain a lowercase letter".into()));
        }
        if !raw.chars().any(|c| c.is_ascii_digit()) {
            return Err(UsersError::InvalidInput("password must contain a digit".into()));
        }
        if !raw.chars().any(|c| "!@#$%^&*()-_=+[]{}|;:,.<>?".contains(c)) {
            return Err(UsersError::InvalidInput("password must contain a special character".into()));
        }
        if !raw.is_ascii() {
            return Err(UsersError::InvalidInput("password must contain only standard ASCII characters".into()));
        }
        Ok(())
    }

    pub fn verify(&self, attempt: &str) -> bool {
        let pepper = std::env::var("PASSWORD_PEPPER")
            .unwrap_or_else(|_| "default_test_pepper_for_unit_tests_32_bytes!".to_string());
        let peppered = format!("{pepper}/{attempt}");

        let Ok(parsed_hash) = PasswordHash::new(&self.0) else { return false };
        Argon2::default()
            .verify_password(peppered.as_bytes(), &parsed_hash)
            .is_ok()
    }

    pub fn hash_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct Users {
    pub name: Name,
    pub user_id: UsersID,
    pub user_name: String,
    pub email: Option<Email>,
    pub password: Password,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub name: Name,
    pub password: Password,
    pub user_name: String,
    pub email: Option<Email>,
}

impl NewUser {
    pub fn new(name: Name, password: Password, user_name: String, email: Option<Email>) -> Self {
        NewUser {
            name,
            password,
            user_name,
            email,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // EMAIL VALIDATION EDGE CASES
    // ==========================================
    #[test]
    fn test_email_valid_formats() {
        assert!(Email::new("user@example.com".to_string()).is_ok());
        assert!(Email::new("user.name+tag@sub.domain.org".to_string()).is_ok());
        assert!(Email::new("  spaced@example.com  ".to_string()).is_ok());
    }

    #[test]
    fn test_email_empty_rejected() {
        let err = Email::new("".to_string()).unwrap_err();
        assert_eq!(err, UsersError::InvalidInput("email cannot be empty".into()));

        let err_spaces = Email::new("   ".to_string()).unwrap_err();
        assert_eq!(err_spaces, UsersError::InvalidInput("email cannot be empty".into()));
    }

    #[test]
    fn test_email_missing_at_symbol_rejected() {
        let err = Email::new("plainaddress".to_string()).unwrap_err();
        assert_eq!(err, UsersError::InvalidInput("invalid email format".into()));
    }

    #[test]
    fn test_email_too_long_rejected() {
        let long_email = format!("{}@example.com", "a".repeat(201));
        let err = Email::new(long_email).unwrap_err();
        assert_eq!(err, UsersError::InvalidInput("email too long".into()));
    }

    #[test]
    fn test_name_valid_and_international() {
        assert!(Name::new("John".to_string(), None, "Doe".to_string()).is_ok());
        assert!(Name::new("John".to_string(), Some("Robert".to_string()), "Doe".to_string()).is_ok());
        assert!(Name::new("Mary-Jane".to_string(), None, "O'Connor".to_string()).is_ok());
        assert!(Name::new("王小明".to_string(), None, "王".to_string()).is_ok());
        assert!(Name::new("José".to_string(), None, "García".to_string()).is_ok());
        assert!(Name::new("François".to_string(), None, "Dupont".to_string()).is_ok());
        assert!(Name::new("田中".to_string(), None, "太郎".to_string()).is_ok());
    }

    #[test]
    fn test_name_empty_rejected() {
        assert!(Name::new("".to_string(), None, "Doe".to_string()).is_err());
        assert!(Name::new("John".to_string(), None, "".to_string()).is_err());
        assert!(Name::new("John".to_string(), Some("".to_string()), "Doe".to_string()).is_err());
    }

    #[test]
    fn test_name_too_long_rejected() {
        let long_name = "a".repeat(101);
        assert!(Name::new(long_name, None, "Doe".to_string()).is_err());
    }

    #[test]
    fn test_name_invalid_characters_rejected() {
        assert!(Name::new("🍕Pizza".to_string(), None, "Doe".to_string()).is_err());
        assert!(Name::new("John".to_string(), None, "😀".to_string()).is_err());
        assert!(Name::new("User<Script>".to_string(), None, "Doe".to_string()).is_err());
        assert!(Name::new("Admin;DROP".to_string(), None, "Table".to_string()).is_err());
    }

    #[test]
    fn test_name_full_name_formatting() {
        let name_with_middle = Name::new("Jan".to_string(), Some("Barona".to_string()), "Angulo".to_string()).unwrap();
        assert_eq!(name_with_middle.full_name(), "Jan Barona Angulo");

        let name_no_middle = Name::new("Jan".to_string(), None, "Angulo".to_string()).unwrap();
        assert_eq!(name_no_middle.full_name(), "Jan Angulo");
    }

    #[test]
    fn test_password_valid_strength_and_verification() {
        let raw = "StrongP@ssw0rd123";
        let pwd = Password::new(raw.to_string()).expect("valid password should succeed");
        assert!(pwd.verify(raw), "password verification must succeed with correct attempt");
        assert!(!pwd.verify("WrongP@ssw0rd123"), "password verification must fail with wrong attempt");
    }

    #[test]
    fn test_password_length_edge_cases() {
        // Less than 12 chars
        let short_err = Password::new("Str0ng!1".to_string()).unwrap_err();
        assert_eq!(short_err, UsersError::InvalidInput("password must be at least 12 characters".into()));

        // Exactly 12 chars (boundary minimum)
        assert!(Password::new("Abc!12345678".to_string()).is_ok());

        // More than 128 chars
        let long_raw = format!("{}Aa1!", "a".repeat(130));
        let long_err = Password::new(long_raw).unwrap_err();
        assert_eq!(long_err, UsersError::InvalidInput("password too long".into()));
    }

    #[test]
    fn test_password_missing_complexity_requirements() {
        // Missing uppercase
        let err_no_upper = Password::new("lowercase123!@#".to_string()).unwrap_err();
        assert_eq!(err_no_upper, UsersError::InvalidInput("password must contain an uppercase letter".into()));

        // Missing lowercase
        let err_no_lower = Password::new("UPPERCASE123!@#".to_string()).unwrap_err();
        assert_eq!(err_no_lower, UsersError::InvalidInput("password must contain a lowercase letter".into()));

        // Missing digit
        let err_no_digit = Password::new("NoDigitsHere!@#".to_string()).unwrap_err();
        assert_eq!(err_no_digit, UsersError::InvalidInput("password must contain a digit".into()));

        // Missing special character
        let err_no_special = Password::new("NoSpecialChar123".to_string()).unwrap_err();
        assert_eq!(err_no_special, UsersError::InvalidInput("password must contain a special character".into()));

        // Non-ASCII character
        let err_non_ascii = Password::new("Password123!ñ".to_string()).unwrap_err();
        assert_eq!(err_non_ascii, UsersError::InvalidInput("password must contain only standard ASCII characters".into()));
    }
}
