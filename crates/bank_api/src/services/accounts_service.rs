use bank_db::AccountsRepository;
use bank_domain::{AccountError, AccountType, Accounts, NewAccount, UsersID};

pub struct AccountsService {
    repo: AccountsRepository,
}

impl AccountsService {
    pub fn new(repo: AccountsRepository) -> Self {
        AccountsService { repo }
    }

    pub async fn create_account(
        &self,
        user_id_raw: i64,
        account_type: AccountType,
    ) -> Result<Accounts, AccountError> {
        let user_id = UsersID::from_db(user_id_raw);
        let new_account = NewAccount::new(user_id, account_type);
        self.repo.insert(new_account).await
    }
}
