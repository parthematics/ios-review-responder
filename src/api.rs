use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::config::{Config, Platform};
use crate::review::{Review, ReviewsResponse};

const APP_STORE_CONNECT_API_BASE: &str = "https://api.appstoreconnect.apple.com/v1";
const GOOGLE_PLAY_API_BASE: &str = "https://www.googleapis.com/androidpublisher/v3";

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    exp: i64,
    aud: String,
}

pub enum ApiClient {
    AppStore(AppStoreConnectClient),
    GooglePlay(GooglePlayClient),
}

pub struct AppStoreConnectClient {
    client: Client,
    config: Config,
    jwt_token: Option<String>,
    token_expires_at: Option<chrono::DateTime<Utc>>,
}

pub struct GooglePlayClient {
    client: Client,
    config: Config,
    access_token: Option<String>,
    token_expires_at: Option<chrono::DateTime<Utc>>,
    next_page_token: Option<String>,
    has_more_pages: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceAccountKey {
    #[serde(rename = "type")]
    key_type: String,
    project_id: String,
    private_key_id: String,
    private_key: String,
    client_email: String,
    client_id: String,
    auth_uri: String,
    token_uri: String,
}

impl ApiClient {
    pub fn new(config: Config) -> Self {
        match config.platform {
            Platform::Ios => Self::AppStore(AppStoreConnectClient::new(config)),
            Platform::Android => Self::GooglePlay(GooglePlayClient::new(config)),
        }
    }

    pub async fn get_reviews(&mut self) -> Result<Vec<Review>> {
        match self {
            Self::AppStore(client) => client.get_reviews().await,
            Self::GooglePlay(client) => client.get_reviews().await,
        }
    }

    pub async fn submit_response(&mut self, review_id: &str, response_body: &str) -> Result<()> {
        match self {
            Self::AppStore(client) => client.submit_response(review_id, response_body).await,
            Self::GooglePlay(client) => client.submit_response(review_id, response_body).await,
        }
    }

    pub async fn get_review_response(
        &mut self,
        review_id: &str,
    ) -> Result<Option<crate::review::ReviewResponse>> {
        match self {
            Self::AppStore(client) => client.get_review_response(review_id).await,
            Self::GooglePlay(client) => client.get_review_response(review_id).await,
        }
    }

    pub async fn load_more_reviews(&mut self) -> Result<Vec<Review>> {
        match self {
            Self::AppStore(_) => Ok(Vec::new()), // iOS loads all reviews at once
            Self::GooglePlay(client) => client.load_next_page().await,
        }
    }

    pub fn has_more_reviews(&self) -> bool {
        match self {
            Self::AppStore(_) => false, // iOS loads all reviews at once
            Self::GooglePlay(client) => client.has_more_reviews(),
        }
    }

    pub async fn refresh_all_reviews(&mut self) -> Result<Vec<Review>> {
        match self {
            Self::AppStore(client) => client.get_reviews().await,
            Self::GooglePlay(client) => client.refresh_all_reviews().await,
        }
    }
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
        let private_key_path = self
            .config
            .private_key_path
            .as_ref()
            .ok_or_else(|| anyhow!("Private key path not configured for iOS"))?;
        let key_id = self
            .config
            .key_id
            .as_ref()
            .ok_or_else(|| anyhow!("Key ID not configured for iOS"))?;
        let issuer_id = self
            .config
            .issuer_id
            .as_ref()
            .ok_or_else(|| anyhow!("Issuer ID not configured for iOS"))?;

        // read private key file
        let private_key_content = fs::read_to_string(private_key_path)
            .map_err(|e| anyhow!("Failed to read private key file: {}", e))?;

        // create JWT header and claims
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(key_id.clone());

        let now = Utc::now();
        let exp = now + Duration::minutes(20); // Apple recommends max 20 minutes

        let claims = Claims {
            iss: issuer_id.clone(),
            exp: exp.timestamp(),
            aud: "appstoreconnect-v1".to_string(),
        };

        // create encoding key directly from the PEM content
        // App Store Connect uses ES256 (P-256 elliptic curve) keys
        let encoding_key = EncodingKey::from_ec_pem(private_key_content.as_bytes())
            .map_err(|e| anyhow!("Failed to create encoding key from EC private key: {}", e))?;

        // generate JWT
        let token = encode(&header, &claims, &encoding_key)
            .map_err(|e| anyhow!("Failed to encode JWT: {}", e))?;

        Ok(token)
    }

    async fn ensure_valid_token(&mut self) -> Result<()> {
        let now = Utc::now();

        // check if we need a new token
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
        writeln!(
            log_file,
            "DEBUG: Using token (first 20 chars): {}...",
            &token[..20.min(token.len())]
        )
        .ok();

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
        let reviews_response: ReviewsResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                anyhow!(
                    "Failed to parse reviews response: {}. Response was: {}",
                    e,
                    response_text
                )
            })?;

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

    pub async fn get_review_response(
        &mut self,
        review_id: &str,
    ) -> Result<Option<crate::review::ReviewResponse>> {
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
        writeln!(
            log_file,
            "DEBUG: Fetching response for review ID: {}",
            review_id
        )
        .ok();

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
        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response: {}", e))?;
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

    async fn get_response_details(
        &mut self,
        response_id: &str,
    ) -> Result<crate::review::ReviewResponse> {
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
        writeln!(
            log_file,
            "DEBUG: Fetching response details for ID: {}",
            response_id
        )
        .ok();
        writeln!(log_file, "DEBUG: Response details URL: {}", url).ok();

        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch response details: {}", e))?;

        writeln!(
            log_file,
            "DEBUG: Response details status: {}",
            response.status()
        )
        .ok();

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

        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response: {}", e))?;
        writeln!(log_file, "DEBUG: Response details JSON: {}", response_text).ok();
        let response_data: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse response details: {}", e))?;

        if let Some(data) = response_data.get("data") {
            if let Some(attrs) = data.get("attributes") {
                let response_body = attrs
                    .get("responseBody")
                    .and_then(|b| b.as_str())
                    .unwrap_or("")
                    .to_string();

                let last_modified_str = attrs
                    .get("lastModifiedDate")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");

                writeln!(
                    log_file,
                    "DEBUG: Parsing date string: {}",
                    last_modified_str
                )
                .ok();
                let last_modified_date = chrono::DateTime::parse_from_rfc3339(last_modified_str)
                    .map_err(|e| {
                        writeln!(log_file, "DEBUG: Date parse error: {}", e).ok();
                        anyhow!("Failed to parse date: {}", e)
                    })?
                    .with_timezone(&chrono::Utc);
                writeln!(log_file, "DEBUG: Parsed date: {}", last_modified_date).ok();

                let state_str = attrs
                    .get("state")
                    .and_then(|s| s.as_str())
                    .unwrap_or("PENDING");

                let state = match state_str {
                    "PUBLISHED" => crate::review::ResponseState::Published,
                    _ => crate::review::ResponseState::Pending,
                };

                writeln!(
                    log_file,
                    "DEBUG: Creating ReviewResponse with body: {}",
                    response_body
                )
                .ok();
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

impl GooglePlayClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
            access_token: None,
            token_expires_at: None,
            next_page_token: None,
            has_more_pages: true,
        }
    }

    async fn ensure_valid_token(&mut self) -> Result<()> {
        let now = Utc::now();

        let needs_new_token = match &self.token_expires_at {
            Some(expires_at) => now >= *expires_at - Duration::minutes(5),
            None => true,
        };

        if needs_new_token {
            let token = self.generate_access_token().await?;
            let expires_at = now + Duration::minutes(55); // Google tokens expire in 1 hour

            self.access_token = Some(token);
            self.token_expires_at = Some(expires_at);
        }

        Ok(())
    }

    async fn generate_access_token(&self) -> Result<String> {
        let service_account_path = self
            .config
            .service_account_path
            .as_ref()
            .ok_or_else(|| anyhow!("Service account path not configured for Android"))?;

        let service_account_content = fs::read_to_string(service_account_path)
            .map_err(|e| anyhow!("Failed to read service account file: {}", e))?;

        let service_account: ServiceAccountKey = serde_json::from_str(&service_account_content)
            .map_err(|e| anyhow!("Failed to parse service account JSON: {}", e))?;

        // Create JWT for service account authentication
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(service_account.private_key_id.clone());

        let now = Utc::now();
        let exp = now + Duration::minutes(60);

        let claims = serde_json::json!({
            "iss": service_account.client_email,
            "scope": "https://www.googleapis.com/auth/androidpublisher",
            "aud": service_account.token_uri,
            "exp": exp.timestamp(),
            "iat": now.timestamp()
        });

        let encoding_key = EncodingKey::from_rsa_pem(service_account.private_key.as_bytes())
            .map_err(|e| anyhow!("Failed to create encoding key from RSA private key: {}", e))?;

        let jwt_token = encode(&header, &claims, &encoding_key)
            .map_err(|e| anyhow!("Failed to encode JWT: {}", e))?;

        // exchange JWT for access token
        let token_request = serde_json::json!({
            "grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
            "assertion": jwt_token
        });

        let response = self
            .client
            .post(&service_account.token_uri)
            .header("Content-Type", "application/json")
            .json(&token_request)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to request access token: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to get access token with status {}: {}",
                status,
                error_text
            ));
        }

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse token response: {}", e))?;

        let access_token = token_response
            .get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow!("No access token in response"))?;

        Ok(access_token.to_string())
    }

    pub async fn get_reviews(&mut self) -> Result<Vec<Review>> {
        // For initial load, just get the first page
        self.load_next_page().await
    }

    pub async fn load_next_page(&mut self) -> Result<Vec<Review>> {
        if !self.has_more_pages {
            return Ok(Vec::new());
        }

        self.ensure_valid_token().await?;

        let token = self.access_token.as_ref().unwrap();
        let url = format!(
            "{}/applications/{}/reviews",
            GOOGLE_PLAY_API_BASE, self.config.app_id
        );

        let mut query_params = vec![("access_token", token.as_str()), ("maxResults", "100")];

        // add pagination token if we have one
        if let Some(ref page_token) = &self.next_page_token {
            query_params.push(("token", page_token.as_str()));
        }

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch reviews: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "API request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response text: {}", e))?;

        let reviews_response: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse reviews response: {}", e))?;

        let mut page_reviews = Vec::new();

        // Parse reviews from this page
        if let Some(review_items) = reviews_response.get("reviews").and_then(|r| r.as_array()) {
            for item in review_items {
                if let Some(review_data) = self.parse_google_play_review(item) {
                    page_reviews.push(review_data);
                }
            }
        }

        // Update pagination state
        self.next_page_token = reviews_response
            .get("tokenPagination")
            .and_then(|tp| tp.get("nextPageToken"))
            .and_then(|token| token.as_str())
            .map(|s| s.to_string());

        self.has_more_pages = self.next_page_token.is_some();

        Ok(page_reviews)
    }

    pub fn has_more_reviews(&self) -> bool {
        self.has_more_pages
    }

    pub async fn refresh_all_reviews(&mut self) -> Result<Vec<Review>> {
        // Reset pagination state
        self.next_page_token = None;
        self.has_more_pages = true;

        let mut all_reviews = Vec::new();

        // Load all pages
        while self.has_more_pages {
            let page_reviews = self.load_next_page().await?;
            all_reviews.extend(page_reviews);
        }

        Ok(all_reviews)
    }

    fn parse_google_play_review(&self, review_data: &serde_json::Value) -> Option<Review> {
        let review_id = review_data.get("reviewId")?.as_str()?.to_string();

        // Extract author name from the top level
        let reviewer_nickname = review_data
            .get("authorName")
            .and_then(|name| name.as_str())
            .unwrap_or("Anonymous")
            .to_string();

        let comments = review_data.get("comments")?.as_array()?;
        let user_comment = comments.first()?.get("userComment")?;

        let rating = user_comment.get("starRating")?.as_i64()? as i32;
        let body = user_comment.get("text")?.as_str().map(|s| s.to_string());

        // Parse timestamp from seconds field
        let created_timestamp = user_comment
            .get("lastModified")?
            .get("seconds")?
            .as_str()?
            .parse::<i64>()
            .ok()?;
        let created_date =
            chrono::DateTime::from_timestamp(created_timestamp, 0)?.with_timezone(&chrono::Utc);

        // Extract app version info if available
        let app_version_name = user_comment
            .get("appVersionName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let app_version_code = user_comment
            .get("appVersionCode")
            .and_then(|v| v.as_i64())
            .map(|c| c.to_string());

        let version = match (app_version_name, app_version_code) {
            (Some(name), Some(code)) => Some(format!("{} ({})", name, code)),
            (Some(name), None) => Some(name),
            (None, Some(code)) => Some(format!("Build {}", code)),
            (None, None) => None,
        };

        let device = user_comment
            .get("device")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        let android_os_version = user_comment
            .get("androidOsVersion")
            .and_then(|v| v.as_i64())
            .map(|v| format!("API {}", v))
            .unwrap_or_else(|| "".to_string());

        let reviewer_language = user_comment
            .get("reviewerLanguage")
            .and_then(|lang| lang.as_str())
            .unwrap_or("")
            .to_string();

        // combine device, OS, and language info for territory field
        let territory = {
            let mut parts = Vec::new();
            if !device.is_empty() {
                parts.push(device);
            }
            if !android_os_version.is_empty() {
                parts.push(android_os_version);
            }
            if !reviewer_language.is_empty() {
                parts.push(reviewer_language);
            }
            if parts.is_empty() {
                "Unknown".to_string()
            } else {
                parts.join(" | ")
            }
        };

        Some(Review {
            id: review_id,
            rating,
            title: None,
            body,
            reviewer_nickname,
            created_date,
            territory,
            version,
            response: None,
        })
    }

    pub async fn submit_response(&mut self, review_id: &str, response_body: &str) -> Result<()> {
        self.ensure_valid_token().await?;

        let token = self.access_token.as_ref().unwrap();
        let url = format!(
            "{}/applications/{}/reviews/{}:reply",
            GOOGLE_PLAY_API_BASE, self.config.app_id, review_id
        );

        let request_body = serde_json::json!({
            "replyText": response_body
        });

        // Debug logging
        use std::io::Write;
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug.log")
            .unwrap_or_else(|_| std::fs::File::create("debug.log").unwrap());

        let response = self
            .client
            .post(&url)
            .query(&[("access_token", token)])
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                writeln!(log_file, "DEBUG: Android submit request failed: {}", e).ok();
                anyhow!("Failed to submit response: {}", e)
            })?;

        writeln!(
            log_file,
            "DEBUG: Android submit response status: {}",
            response.status()
        )
        .ok();

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            writeln!(
                log_file,
                "DEBUG: Android submit error response: {}",
                error_text
            )
            .ok();
            return Err(anyhow!(
                "Failed to submit response with status {}: {}",
                status,
                error_text
            ));
        }

        let success_text = response.text().await.unwrap_or_default();
        writeln!(
            log_file,
            "DEBUG: Android submit success response: {}",
            success_text
        )
        .ok();

        Ok(())
    }

    pub async fn get_review_response(
        &mut self,
        review_id: &str,
    ) -> Result<Option<crate::review::ReviewResponse>> {
        self.ensure_valid_token().await?;

        let token = self.access_token.as_ref().unwrap();
        let url = format!(
            "{}/applications/{}/reviews/{}",
            GOOGLE_PLAY_API_BASE, self.config.app_id, review_id
        );

        let response = self
            .client
            .get(&url)
            .query(&[("access_token", token)])
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch review: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response text: {}", e))?;

        let review_data: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse review response: {}", e))?;

        // Check if there's a developer reply in the comments
        if let Some(comments) = review_data.get("comments").and_then(|c| c.as_array()) {
            for comment in comments {
                if let Some(developer_comment) = comment.get("developerComment") {
                    let response_body = developer_comment
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();

                    let last_modified_str = developer_comment
                        .get("lastModified")
                        .and_then(|lm| lm.get("seconds"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("0");

                    let last_modified_timestamp = last_modified_str.parse::<i64>().unwrap_or(0);
                    let last_modified_date =
                        chrono::DateTime::from_timestamp(last_modified_timestamp, 0)
                            .unwrap_or_else(|| chrono::Utc::now())
                            .with_timezone(&chrono::Utc);

                    return Ok(Some(crate::review::ReviewResponse {
                        id: format!("{}-response", review_id),
                        response_body,
                        last_modified_date,
                        state: crate::review::ResponseState::Published, // Google Play responses are immediately published
                    }));
                }
            }
        }

        Ok(None)
    }
}
