use anyhow::{anyhow, Result};
use clap::ArgMatches;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Platform {
    Ios,
    Android,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub platform: Platform,
    pub app_id: String,
    pub key_id: Option<String>,
    pub issuer_id: Option<String>,
    pub private_key_path: Option<PathBuf>,
    pub service_account_path: Option<PathBuf>,
    pub openai_api_key: Option<String>,
}

impl Config {
    pub fn from_args_and_env(matches: &ArgMatches) -> Result<Self> {
        let platform = if matches.get_flag("android") {
            Platform::Android
        } else {
            Platform::Ios
        };

        let app_id = matches
            .get_one::<String>("app-id")
            .cloned()
            .or_else(|| match platform {
                Platform::Ios => env::var("APP_STORE_APP_ID").ok(),
                Platform::Android => env::var("GOOGLE_PLAY_PACKAGE_NAME").ok(),
            })
            .ok_or_else(|| match platform {
                Platform::Ios => anyhow!("App ID is required. Use --app-id or set APP_STORE_APP_ID environment variable"),
                Platform::Android => anyhow!("Package name is required. Use --app-id or set GOOGLE_PLAY_PACKAGE_NAME environment variable"),
            })?;

        match platform {
            Platform::Ios => {
                let key_id = matches
                    .get_one::<String>("key-id")
                    .cloned()
                    .or_else(|| env::var("APP_STORE_CONNECT_KEY_ID").ok())
                    .ok_or_else(|| anyhow!("Key ID is required for iOS. Use --key-id or set APP_STORE_CONNECT_KEY_ID environment variable"))?;

                let issuer_id = matches
                    .get_one::<String>("issuer-id")
                    .cloned()
                    .or_else(|| env::var("APP_STORE_CONNECT_ISSUER_ID").ok())
                    .ok_or_else(|| anyhow!("Issuer ID is required for iOS. Use --issuer-id or set APP_STORE_CONNECT_ISSUER_ID environment variable"))?;

                let private_key_path = matches
                    .get_one::<String>("private-key")
                    .map(PathBuf::from)
                    .or_else(|| env::var("APP_STORE_CONNECT_PRIVATE_KEY_PATH").ok().map(PathBuf::from))
                    .ok_or_else(|| anyhow!("Private key path is required for iOS. Use --private-key or set APP_STORE_CONNECT_PRIVATE_KEY_PATH environment variable"))?;

                let openai_api_key = env::var("OPENAI_API_KEY").ok();

                Ok(Config {
                    platform,
                    app_id,
                    key_id: Some(key_id),
                    issuer_id: Some(issuer_id),
                    private_key_path: Some(private_key_path),
                    service_account_path: None,
                    openai_api_key,
                })
            }
            Platform::Android => {
                let service_account_path = matches
                    .get_one::<String>("service-account")
                    .map(PathBuf::from)
                    .or_else(|| env::var("GOOGLE_PLAY_SERVICE_ACCOUNT_PATH").ok().map(PathBuf::from))
                    .ok_or_else(|| anyhow!("Service account path is required for Android. Use --service-account or set GOOGLE_PLAY_SERVICE_ACCOUNT_PATH environment variable"))?;

                let openai_api_key = env::var("OPENAI_API_KEY").ok();

                Ok(Config {
                    platform,
                    app_id,
                    key_id: None,
                    issuer_id: None,
                    private_key_path: None,
                    service_account_path: Some(service_account_path),
                    openai_api_key,
                })
            }
        }
    }
}