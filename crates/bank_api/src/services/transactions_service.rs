use bank_db::TransactionsRepository;
use bank_domain::{
    AccountID, AccountNumber, Amount, Balance, Status, TransactionError, Transactions, UsersID,
};

pub struct TransactionsService {
    transactions_repo: TransactionsRepository,
}

impl TransactionsService {
    pub fn new(transactions_repo: TransactionsRepository) -> Self {
        TransactionsService { transactions_repo }
    }

    pub async fn deposit(
        &self,
        user_id: UsersID,
        account_id: AccountID,
        amount: Amount,
    ) -> Result<Transactions, TransactionError> {
        let account = self.transactions_repo.find_account_by_id(account_id).await?;
        if account.user_id != user_id {
            return Err(TransactionError::UnauthorizedAccountAccess);
        }
        if account.status != Status::Active {
            return Err(TransactionError::AccountNotActive);
        }

        self.transactions_repo.deposit(account_id, amount.cents()).await
    }

    pub async fn withdraw(
        &self,
        user_id: UsersID,
        account_id: AccountID,
        amount: Amount,
    ) -> Result<Transactions, TransactionError> {
        let account = self.transactions_repo.find_account_by_id(account_id).await?;
        if account.user_id != user_id {
            return Err(TransactionError::UnauthorizedAccountAccess);
        }
        if account.status != Status::Active {
            return Err(TransactionError::AccountNotActive);
        }

        self.transactions_repo.withdraw(account_id, amount.cents()).await
    }

    pub async fn transfer(
        &self,
        user_id: UsersID,
        from_account_id: AccountID,
        to_account_number: AccountNumber,
        amount: Amount,
    ) -> Result<(Transactions, Transactions), TransactionError> {
        // 1. Verify source account
        let source_account = self.transactions_repo.find_account_by_id(from_account_id).await?;
        if source_account.user_id != user_id {
            return Err(TransactionError::UnauthorizedAccountAccess);
        }
        if source_account.status != Status::Active {
            return Err(TransactionError::AccountNotActive);
        }

        // 2. Resolve and verify destination account
        let dest_account = self
            .transactions_repo
            .find_account_by_number(&to_account_number)
            .await?;
        if dest_account.status != Status::Active {
            return Err(TransactionError::AccountNotActive);
        }

        // 3. Prevent self-transfers to the same account ID
        if source_account.account_id == dest_account.account_id {
            return Err(TransactionError::SelfTransferNotAllowed);
        }

        // 4. Execute atomic transfer in Oracle
        self.transactions_repo
            .transfer(from_account_id, dest_account.account_id, amount.cents())
            .await
    }

    pub async fn get_balance(
        &self,
        user_id: UsersID,
        account_id: AccountID,
    ) -> Result<Balance, TransactionError> {
        let account = self.transactions_repo.find_account_by_id(account_id).await?;
        if account.user_id != user_id {
            return Err(TransactionError::UnauthorizedAccountAccess);
        }

        self.transactions_repo.get_balance(account_id).await
    }

    pub async fn get_statement(
        &self,
        user_id: UsersID,
        account_id: AccountID,
        limit: i64,
    ) -> Result<Vec<Transactions>, TransactionError> {
        let account = self.transactions_repo.find_account_by_id(account_id).await?;
        if account.user_id != user_id {
            return Err(TransactionError::UnauthorizedAccountAccess);
        }

        let limit_sanitized = limit.clamp(1, 100);
        self.transactions_repo.get_statement(account_id, limit_sanitized).await
    }
}
