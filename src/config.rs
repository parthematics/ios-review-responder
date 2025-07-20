use anyhow::{anyhow, Result};
use clap::ArgMatches;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub app_id: String,
    pub key_id: String,
    pub issuer_id: String,
    pub private_key_path: PathBuf,
    pub openai_api_key: Option<String>,
}

impl Config {
    pub fn from_args_and_env(matches: &ArgMatches) -> Result<Self> {
        let app_id = matches
            .get_one::<String>("app-id")
            .cloned()
            .or_else(|| env::var("APP_STORE_APP_ID").ok())
            .ok_or_else(|| anyhow!("App ID is required. Use --app-id or set APP_STORE_APP_ID environment variable"))?;

        let key_id = matches
            .get_one::<String>("key-id")
            .cloned()
            .or_else(|| env::var("APP_STORE_CONNECT_KEY_ID").ok())
            .ok_or_else(|| anyhow!("Key ID is required. Use --key-id or set APP_STORE_CONNECT_KEY_ID environment variable"))?;

        let issuer_id = matches
            .get_one::<String>("issuer-id")
            .cloned()
            .or_else(|| env::var("APP_STORE_CONNECT_ISSUER_ID").ok())
            .ok_or_else(|| anyhow!("Issuer ID is required. Use --issuer-id or set APP_STORE_CONNECT_ISSUER_ID environment variable"))?;

        let private_key_path = matches
            .get_one::<String>("private-key")
            .map(PathBuf::from)
            .or_else(|| env::var("APP_STORE_CONNECT_PRIVATE_KEY_PATH").ok().map(PathBuf::from))
            .ok_or_else(|| anyhow!("Private key path is required. Use --private-key or set APP_STORE_CONNECT_PRIVATE_KEY_PATH environment variable"))?;

        let openai_api_key = env::var("OPENAI_API_KEY").ok();

        Ok(Config {
            app_id,
            key_id,
            issuer_id,
            private_key_path,
            openai_api_key,
        })
    }
}