use bank_db::UsersRepository;
use bank_domain::{Email, Name, NewUser, Password, Users, UsersError};

pub struct UsersService {
    repo: UsersRepository,
}

impl UsersService {
    pub fn new(repo: UsersRepository) -> Self {
        UsersService { repo }
    }

    pub async fn create_user(
        &self,
        first_name: String,
        middle_name: Option<String>,
        last_name: String,
        user_name: String,
        password_raw: String,
        email_raw: Option<String>,
    ) -> Result<Users, UsersError> {
        let name = Name::new(first_name, middle_name, last_name)?;
        let password = Password::new(password_raw)?;
        let email = match email_raw {
            Some(email) => Some(Email::new(email)?),
            None => None,
        };

        let new_user = NewUser::new(name, password, user_name, email);
        self.repo.insert(new_user).await
    }
}
