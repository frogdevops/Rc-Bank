use chrono::{DateTime, Utc};
use serde::Deserialize;

pub(crate) struct AccountID(i32);
pub(crate) struct AccountNumber(i32);
pub(crate) struct Balance(i32);

impl Balance {
	//TODO: Validate the balance
	// if balance is < 0 reject
	// if balance is greater than account limit reject

}
pub(crate) struct UserID(i32);

impl UserID {
	//TODO: Get UserId from database
	// Simple handling
}

pub(crate) struct UserName(String);
impl UserName {
	//TODO: Get the username and stuff it into the system as type
}
pub(crate) enum Status {
	Active,
	Frozen,
	Closed,
	Suspended,
}

impl Status {
	// TODO: Convert to String based on the status
}

pub(crate) enum AccountType {
	Savings,
	Checking,
}

impl AccountType {
	// TODO: From request gets deserialized
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

impl Accounts {
	// TODO: wire up
}

pub(crate) struct CreateAccountSystem {

}


