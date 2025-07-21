use anyhow::Result;
use clap::{Arg, Command};
use dotenv::dotenv;

mod ai;
mod api;
mod ui;
mod config;
mod review;

use ui::ReviewUI;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists (ignore errors if file doesn't exist)
    dotenv().ok();
    
    // Test AI functionality with --test-ai flag
    if std::env::args().any(|arg| arg == "--test-ai") {
        use crate::ai::{AIConfig, AIResponseGenerator};
        use crate::review::Review;
        use chrono::Utc;
        
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
