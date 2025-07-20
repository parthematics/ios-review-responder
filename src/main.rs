use anyhow::Result;
use clap::{Arg, Command};
use dotenv::dotenv;

mod api;
mod ui;
mod config;
mod review;

use ui::ReviewUI;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists (ignore errors if file doesn't exist)
    dotenv().ok();
    let matches = Command::new("apple-review-responder")
        .version("0.1.0")
        .about("CLI tool for responding to Apple App Store reviews")
        .arg(
            Arg::new("app-id")
                .long("app-id")
                .value_name("APP_ID")
                .help("Your app's App Store ID")
                .required(false),
        )
        .arg(
            Arg::new("key-id")
                .long("key-id")
                .value_name("KEY_ID")
                .help("App Store Connect API Key ID")
                .required(false),
        )
        .arg(
            Arg::new("issuer-id")
                .long("issuer-id")
                .value_name("ISSUER_ID")
                .help("App Store Connect API Issuer ID")
                .required(false),
        )
        .arg(
            Arg::new("private-key")
                .long("private-key")
                .value_name("PRIVATE_KEY_PATH")
                .help("Path to your App Store Connect API private key file")
                .required(false),
        )
        .get_matches();

    let config = config::Config::from_args_and_env(&matches)?;
    
    let mut ui = ReviewUI::new(config).await?;
    ui.run().await?;

    Ok(())
}
