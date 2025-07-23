use anyhow::Result;
use chrono::Utc;
use clap::{Arg, Command};
use dotenv::dotenv;

use crate::ai::{AIConfig, AIResponseGenerator};
use crate::api::ApiClient;
use crate::review::Review;

mod ai;
mod api;
mod config;
mod review;
mod ui;

use ui::ReviewUI;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists (ignore errors if file doesn't exist)
    dotenv().ok();

    // Test Google Play API access with --test-android flag
    if std::env::args().any(|arg| arg == "--test-android") {
        let matches = Command::new("rustpond")
            .arg(
                Arg::new("android")
                    .long("android")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(Arg::new("app-id").long("app-id").value_name("APP_ID"))
            .arg(
                Arg::new("service-account")
                    .long("service-account")
                    .value_name("SERVICE_ACCOUNT_PATH"),
            )
            .get_matches();

        let config = config::Config::from_args_and_env(&matches)?;
        let mut client = ApiClient::new(config);

        match client.refresh_all_reviews().await {
            Ok(reviews) => println!("Successfully accessed reviews: {} found", reviews.len()),
            Err(e) => println!("Error accessing reviews: {}", e),
        }

        return Ok(());
    }

    // Test AI functionality with --test-ai flag
    if std::env::args().any(|arg| arg == "--test-ai") {
        let config = AIConfig::default();
        let generator = AIResponseGenerator::new(config)?;

        let test_review = Review {
            id: "test".to_string(),
            rating: 5,
            title: Some("Great app!".to_string()),
            body: Some("I love this app, it works perfectly!".to_string()),
            reviewer_nickname: "TestUser".to_string(),
            created_date: Utc::now(),
            territory: "US".to_string(),
            version: Some("1.0".to_string()),
            response: None,
        };

        println!("Testing AI response generation...");
        match generator.generate_response(&test_review).await {
            Ok(response) => println!("AI Response: {}", response),
            Err(e) => println!("AI Error: {}", e),
        }
        return Ok(());
    }

    let matches = Command::new("rustpond")
        .version("0.1.0")
        .about("CLI tool for responding to app store reviews (iOS and Android)")
        .arg(
            Arg::new("ios")
                .long("ios")
                .help("Use Apple App Store (default)")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("android"),
        )
        .arg(
            Arg::new("android")
                .long("android")
                .help("Use Google Play Store")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("ios"),
        )
        .arg(
            Arg::new("app-id")
                .long("app-id")
                .value_name("APP_ID")
                .help("Your app's App Store ID (iOS) or package name (Android)")
                .required(false),
        )
        .arg(
            Arg::new("key-id")
                .long("key-id")
                .value_name("KEY_ID")
                .help("App Store Connect API Key ID (iOS only)")
                .required(false),
        )
        .arg(
            Arg::new("issuer-id")
                .long("issuer-id")
                .value_name("ISSUER_ID")
                .help("App Store Connect API Issuer ID (iOS only)")
                .required(false),
        )
        .arg(
            Arg::new("private-key")
                .long("private-key")
                .value_name("PRIVATE_KEY_PATH")
                .help("Path to your App Store Connect API private key file (iOS only)")
                .required(false),
        )
        .arg(
            Arg::new("service-account")
                .long("service-account")
                .value_name("SERVICE_ACCOUNT_PATH")
                .help("Path to Google Play Console service account JSON file (Android only)")
                .required(false),
        )
        .get_matches();

    let config = config::Config::from_args_and_env(&matches)?;

    let mut ui = ReviewUI::new(config).await?;
    ui.run().await?;

    Ok(())
}
