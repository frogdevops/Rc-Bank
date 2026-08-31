use bank_db::{AccountsRepository, UsersRepository};
use bank_domain::{AccountError, AccountType, Accounts, NewAccount, UsersID};

pub struct AccountsService {
    users_repo: UsersRepository,
    accounts_repo: AccountsRepository,
}

impl AccountsService {
    pub fn new(users_repo: UsersRepository, accounts_repo: AccountsRepository) -> Self {
        AccountsService {
            users_repo,
            accounts_repo,
        }
    }

    pub async fn create_account(
        &self,
        user_id: UsersID,
        account_type: AccountType,
    ) -> Result<Accounts, AccountError> {
        // 1. Verify user exists in the database
        self.users_repo
            .find_by_user_id(user_id)
            .await
            .map_err(|_| AccountError::NotFound)?;

        // 2. Safe to insert new account
        let new_account = NewAccount::new(user_id, account_type);
        self.accounts_repo.insert(new_account).await
    }
}
