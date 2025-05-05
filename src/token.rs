use std::env;
use thiserror::Error;

const TOKEN_KEY: &str = "TELEGRAM_BOT_TOKEN";

#[derive(Debug, Error)]
pub enum LoadTokenError {
    #[error("Failed to parse the .env file")]
    DotEnv(dotenvy::Error),
    #[error("Failed to find the bot token in environment variables or the .env file")]
    NotFound,
}

impl From<dotenvy::Error> for LoadTokenError {
    fn from(value: dotenvy::Error) -> Self {
        if value.not_found() {
            Self::NotFound
        } else {
            Self::DotEnv(value)
        }
    }
}

pub fn load_token() -> Result<String, LoadTokenError> {
    let maybe_token = env::vars().find_map(|(key, value)| (key == TOKEN_KEY).then_some(value));
    if let Some(token) = maybe_token {
        return Ok(token);
    }

    let mut dotenv_file = dotenvy::dotenv_iter()?;
    let maybe_token = dotenv_file.find_map(|kv_pair| match kv_pair {
        Err(e) => Some(Err(e.into())),
        Ok((key, value)) => (key == TOKEN_KEY).then_some(Ok(value)),
    });

    maybe_token.unwrap_or(Err(LoadTokenError::NotFound))
}
