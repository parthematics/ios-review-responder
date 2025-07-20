use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::config::Config;
use crate::review::{Review, ReviewsResponse};

const APP_STORE_CONNECT_API_BASE: &str = "https://api.appstoreconnect.apple.com/v1";

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    exp: i64,
    aud: String,
}

pub struct AppStoreConnectClient {
    client: Client,
    config: Config,
    jwt_token: Option<String>,
    token_expires_at: Option<chrono::DateTime<Utc>>,
}

impl AppStoreConnectClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
            jwt_token: None,
            token_expires_at: None,
        }
    }

    fn generate_jwt(&self) -> Result<String> {
        // Read the private key file
        let private_key_content = fs::read_to_string(&self.config.private_key_path)
            .map_err(|e| anyhow!("Failed to read private key file: {}", e))?;

        // Create JWT header and claims
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.config.key_id.clone());
        
        let now = Utc::now();
        let exp = now + Duration::minutes(20); // Apple recommends max 20 minutes

        let claims = Claims {
            iss: self.config.issuer_id.clone(),
            exp: exp.timestamp(),
            aud: "appstoreconnect-v1".to_string(),
        };

        // Create encoding key directly from the PEM content
        // App Store Connect uses ES256 (P-256 elliptic curve) keys
        let encoding_key = EncodingKey::from_ec_pem(private_key_content.as_bytes())
            .map_err(|e| anyhow!("Failed to create encoding key from EC private key: {}", e))?;

        // Generate JWT
        let token = encode(&header, &claims, &encoding_key)
            .map_err(|e| anyhow!("Failed to encode JWT: {}", e))?;

        Ok(token)
    }

    async fn ensure_valid_token(&mut self) -> Result<()> {
        let now = Utc::now();
        
        // Check if we need a new token
        let needs_new_token = match &self.token_expires_at {
            Some(expires_at) => now >= *expires_at - Duration::minutes(5), // Refresh 5 minutes early
            None => true,
        };

        if needs_new_token {
            let token = self.generate_jwt()?;
            let expires_at = now + Duration::minutes(15); // Conservative expiry
            
            self.jwt_token = Some(token);
            self.token_expires_at = Some(expires_at);
        }

        Ok(())
    }

    pub async fn get_reviews(&mut self) -> Result<Vec<Review>> {
        self.ensure_valid_token().await?;

        let token = self.jwt_token.as_ref().unwrap();
        let url = format!(
            "{}/apps/{}/customerReviews",
            APP_STORE_CONNECT_API_BASE, self.config.app_id
        );

        use std::io::Write;
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug.log")
            .unwrap_or_else(|_| std::fs::File::create("debug.log").unwrap());
        writeln!(log_file, "DEBUG: About to fetch reviews from URL: {}", url).ok();
        writeln!(log_file, "DEBUG: Using token (first 20 chars): {}...", &token[..20.min(token.len())]).ok();
        
        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .query(&[("limit", "200"), ("sort", "-createdDate")])
            .send()
            .await
            .map_err(|e| {
                writeln!(log_file, "DEBUG: Request failed with error: {}", e).ok();
                anyhow!("Failed to fetch reviews: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "API request failed with status {}: {}",
                status,
                error_text
            ));
        }

        // Get the raw response text first for debugging
        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response text: {}", e))?;

        // Try to parse the JSON response
        let reviews_response: ReviewsResponse = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse reviews response: {}. Response was: {}", e, response_text))?;

        let reviews = reviews_response
            .data
            .into_iter()
            .map(|data| data.into())
            .collect();

        Ok(reviews)
    }

    pub async fn submit_response(&mut self, review_id: &str, response_body: &str) -> Result<()> {
        self.ensure_valid_token().await?;

        let token = self.jwt_token.as_ref().unwrap();
        let url = format!("{}/customerReviewResponses", APP_STORE_CONNECT_API_BASE);

        let request_body = serde_json::json!({
            "data": {
                "type": "customerReviewResponses",
                "attributes": {
                    "responseBody": response_body
                },
                "relationships": {
                    "review": {
                        "data": {
                            "type": "customerReviews",
                            "id": review_id
                        }
                    }
                }
            }
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(token)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to submit response: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to submit response with status {}: {}",
                status,
                error_text
            ));
        }

        Ok(())
    }

    pub async fn get_review_response(&mut self, review_id: &str) -> Result<Option<crate::review::ReviewResponse>> {
        self.ensure_valid_token().await?;

        let token = self.jwt_token.as_ref().unwrap();
        let url = format!(
            "{}/customerReviews/{}/relationships/response",
            APP_STORE_CONNECT_API_BASE, review_id
        );

        use std::io::Write;
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug.log")
            .unwrap_or_else(|_| std::fs::File::create("debug.log").unwrap());
        writeln!(log_file, "DEBUG: Fetching response for review ID: {}", review_id).ok();
        writeln!(log_file, "DEBUG: URL: {}", url).ok();

        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch response: {}", e))?;

        writeln!(log_file, "DEBUG: Response status: {}", response.status()).ok();

        if response.status() == 404 {
            writeln!(log_file, "DEBUG: No response exists (404)").ok();
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            writeln!(log_file, "DEBUG: Error response: {}", error_text).ok();
            return Err(anyhow!(
                "Failed to fetch response with status {}: {}",
                status,
                error_text
            ));
        }

        // If we get a successful response, there is a response - now get the actual response data
        let response_text = response.text().await.map_err(|e| anyhow!("Failed to read response: {}", e))?;
        writeln!(log_file, "DEBUG: Relationship response: {}", response_text).ok();
        
        // Parse the relationship response to get the response ID
        let relationship_data: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse relationship response: {}", e))?;
        
        if let Some(data) = relationship_data.get("data") {
            if !data.is_null() {
                if let Some(response_id) = data.get("id").and_then(|id| id.as_str()) {
                    writeln!(log_file, "DEBUG: Found response ID: {}", response_id).ok();
                    // Now fetch the actual response data
                    return self.get_response_details(response_id).await.map(Some);
                }
            } else {
                writeln!(log_file, "DEBUG: Relationship data is null - no response").ok();
            }
        } else {
            writeln!(log_file, "DEBUG: No data field in relationship response").ok();
        }
        
        Ok(None)
    }

    async fn get_response_details(&mut self, response_id: &str) -> Result<crate::review::ReviewResponse> {
        let token = self.jwt_token.as_ref().unwrap();
        let url = format!(
            "{}/customerReviewResponses/{}",
            APP_STORE_CONNECT_API_BASE, response_id
        );

        use std::io::Write;
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug.log")
            .unwrap_or_else(|_| std::fs::File::create("debug.log").unwrap());
        writeln!(log_file, "DEBUG: Fetching response details for ID: {}", response_id).ok();
        writeln!(log_file, "DEBUG: Response details URL: {}", url).ok();

        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch response details: {}", e))?;

        writeln!(log_file, "DEBUG: Response details status: {}", response.status()).ok();

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            writeln!(log_file, "DEBUG: Response details error: {}", error_text).ok();
            return Err(anyhow!(
                "Failed to fetch response details with status {}: {}",
                status,
                error_text
            ));
        }

        let response_text = response.text().await.map_err(|e| anyhow!("Failed to read response: {}", e))?;
        writeln!(log_file, "DEBUG: Response details JSON: {}", response_text).ok();
        let response_data: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse response details: {}", e))?;

        if let Some(data) = response_data.get("data") {
            if let Some(attrs) = data.get("attributes") {
                let response_body = attrs.get("responseBody")
                    .and_then(|b| b.as_str())
                    .unwrap_or("")
                    .to_string();
                
                let last_modified_str = attrs.get("lastModifiedDate")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                    
                writeln!(log_file, "DEBUG: Parsing date string: {}", last_modified_str).ok();
                let last_modified_date = chrono::DateTime::parse_from_rfc3339(last_modified_str)
                    .map_err(|e| {
                        writeln!(log_file, "DEBUG: Date parse error: {}", e).ok();
                        anyhow!("Failed to parse date: {}", e)
                    })?
                    .with_timezone(&chrono::Utc);
                writeln!(log_file, "DEBUG: Parsed date: {}", last_modified_date).ok();

                let state_str = attrs.get("state")
                    .and_then(|s| s.as_str())
                    .unwrap_or("PENDING");

                let state = match state_str {
                    "PUBLISHED" => crate::review::ResponseState::Published,
                    _ => crate::review::ResponseState::Pending,
                };

                writeln!(log_file, "DEBUG: Creating ReviewResponse with body: {}", response_body).ok();
                let review_response = crate::review::ReviewResponse {
                    id: response_id.to_string(),
                    response_body,
                    last_modified_date,
                    state,
                };
                writeln!(log_file, "DEBUG: Successfully created ReviewResponse").ok();
                return Ok(review_response);
            }
        }

        Err(anyhow!("Invalid response data format"))
    }
}