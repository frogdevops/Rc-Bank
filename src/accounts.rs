use chrono::{DateTime, Utc};
use serde::Deserialize;

pub(crate) struct AccountID(i32);
pub(crate) struct AccountNumber(i32);
pub(crate) struct Balance(i32);
pub(crate) struct UserID(i32);

pub(crate) struct UserName(String);

pub(crate) enum Status {
	Active,
	Frozen,
	Closed,
	Suspended,
}

pub(crate) enum AccountType {
	Savings,
	Checking,
}
pub(crate) struct Accounts {
	account_id: AccountID,
	account_number: AccountNumber,
	user_id: UserID,
	user_name: UserName,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	balance: Balance,
	status: Status,
}
#[derive(Deserialize)]
pub(crate) struct CreateAccountSystem {

}


