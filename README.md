# _rustpond_

a CLI tool built in Rust for managing and responding to app store reviews using the App Store Connect API (iOS) and Google Play Developer API (Android).

<figure style="text-align: center;">
  <img width="1089" height="359" alt="image" src="https://github.com/user-attachments/assets/a28579bb-3c7d-46c3-9446-89fce18279a7" />
  <figcaption style="text-align: center;"><em>some recent reviews for the <a href="https://apps.apple.com/us/app/candle-couple-games-photos/id6743355635">candle</a> app, shown using rustpond</em></figcaption>
</figure>

## Features

- **Review Navigation**: Browse reviews with arrow keys in a terminal UI
- **Manual Responses**: Type custom responses to reviews
- **AI-Generated Responses**: Press 'a' to generate AI responses (requires approval)
- **Response Approval**: Review and approve responses before sending
- **Real-time Updates**: Refresh reviews and see the latest feedback

## Prerequisites

### For iOS (Apple App Store)

1. **App Store Connect API Credentials**:
   - App Store Connect API Key ID
   - Issuer ID  
   - Private Key file (.p8 format)
   - Your app's App Store ID

### For Android (Google Play Store)

1. **Google Play Console Service Account**:
   - Service account JSON file with "Reply to reviews" permission
   - Your app's package name (e.g., com.yourcompany.yourapp)

### Optional

2. **OpenAI API Key** (for AI responses):
   - OpenAI API key for generating automated responses

## Installation

1. Clone the repository:

   ```bash
   git clone <repository-url>
   cd rustpond
   ```

2. Build the application:
   ```bash
   cargo build --release
   ```

## Configuration

### Using .env File (Recommended for Development)

1. Copy the example environment file:

   ```bash
   cp .env.example .env
   ```

2. Edit `.env` with your actual credentials:

   **For iOS:**
   ```bash
   # Required: Your app's App Store ID
   APP_STORE_APP_ID=1234567890

   # Required: App Store Connect API credentials
   APP_STORE_CONNECT_KEY_ID=ABCD123456
   APP_STORE_CONNECT_ISSUER_ID=12345678-1234-1234-1234-123456789012
   APP_STORE_CONNECT_PRIVATE_KEY_PATH=/path/to/your/AuthKey_ABCD123456.p8
   ```

   **For Android:**
   ```bash
   # Required: Your app's package name
   GOOGLE_PLAY_PACKAGE_NAME=com.yourcompany.yourapp

   # Required: Google Play Console service account JSON file
   GOOGLE_PLAY_SERVICE_ACCOUNT_PATH=/path/to/your/service-account.json
   ```

   **Optional (both platforms):**
   ```bash
   # Optional: OpenAI API key for AI responses
   OPENAI_API_KEY=sk-your-openai-api-key-here
   ```

### Using Environment Variables

Alternatively, set environment variables directly:

**For iOS:**
```bash
export APP_STORE_APP_ID="your_app_id"
export APP_STORE_CONNECT_KEY_ID="your_key_id"
export APP_STORE_CONNECT_ISSUER_ID="your_issuer_id"
export APP_STORE_CONNECT_PRIVATE_KEY_PATH="/path/to/your/private_key.p8"
```

**For Android:**
```bash
export GOOGLE_PLAY_PACKAGE_NAME="com.yourcompany.yourapp"
export GOOGLE_PLAY_SERVICE_ACCOUNT_PATH="/path/to/your/service-account.json"
```

**Optional (both platforms):**
```bash
export OPENAI_API_KEY="your_openai_api_key"  # Optional, for AI responses
```

### Command Line Arguments

Alternatively, you can pass credentials as command-line arguments:

**For iOS (default):**
```bash
./target/release/rustpond \
  --ios \
  --app-id "your_app_id" \
  --key-id "your_key_id" \
  --issuer-id "your_issuer_id" \
  --private-key "/path/to/your/private_key.p8"
```

**For Android:**
```bash
./target/release/rustpond \
  --android \
  --app-id "com.yourcompany.yourapp" \
  --service-account "/path/to/your/service-account.json"
```

## Quick Start

1. **Set up credentials**:

   ```bash
   cp .env.example .env
   # Edit .env with your App Store Connect (iOS) or Google Play Console (Android) credentials
   ```

2. **Run the application**:

   **For iOS (default):**
   ```bash
   cargo run
   # or if built already using `cargo build --release`
   ./target/release/rustpond
   ```

   **For Android:**
   ```bash
   cargo run -- --android
   # or if built already
   ./target/release/rustpond --android
   ```

3. **Create an alias for easier startup** (optional):

   ```bash
   # Add to your shell profile (~/.zshrc, ~/.bashrc, etc.)
   alias rustpond='./target/release/rustpond'

   # Then you can simply run:
   rustpond
   ```

## Usage

### Controls

**Review Navigation:**

- `↑/↓` - Navigate between reviews
- `Enter` - Write a manual response to the selected review
- `a` - Generate an AI response for the selected review
- `r` - Refresh reviews from the app store
- `q` - Quit the application

**Response Writing:**

- Type your response in the text area
- `Ctrl+Enter` - Submit response for approval
- `Esc` - Cancel and return to review list

**Response Approval:**

- `y` - Approve and send the response
- `n` or `Esc` - Go back to edit the response

## Platform Setup

### App Store Connect API Setup (iOS)

1. **Create API Key**:

   - Go to [App Store Connect](https://appstoreconnect.apple.com)
   - Navigate to Users and Access > Keys
   - Click the "+" button to create a new API key
   - Select "Developer" role (minimum required for customer reviews)
   - Download the private key file (.p8)

2. **Find Your App ID**:

   - Go to App Store Connect > My Apps
   - Select your app
   - The App ID is in the App Information section

3. **Get Issuer ID**:
   - In App Store Connect, go to Users and Access > Keys
   - Your Issuer ID is displayed at the top of the page

### Google Play Console API Setup (Android)

1. **Create Service Account**:
   - Go to [Google Cloud Console](https://console.cloud.google.com)
   - Select your project (or create one)
   - Navigate to IAM & Admin > Service Accounts
   - Click "Create Service Account"
   - Give it a name and description

2. **Enable Google Play Developer API**:
   - In Google Cloud Console, go to APIs & Services > Library
   - Search for "Google Play Developer API" and enable it

3. **Link Service Account to Google Play Console**:
   - Go to [Google Play Console](https://play.google.com/console)
   - Navigate to Setup > API access
   - Link your Google Cloud project
   - Grant "Reply to reviews" permission to your service account

4. **Download Service Account Key**:
   - In Google Cloud Console, go to IAM & Admin > Service Accounts
   - Click on your service account
   - Go to Keys tab and click "Add Key" > "Create new key"
   - Choose JSON format and download

5. **Find Your Package Name**:
   - In Google Play Console, select your app
   - The package name is displayed in the app details (e.g., com.yourcompany.yourapp)

### Private Key Format (iOS)

The tool supports both PKCS#1 and PKCS#8 format private keys:

- PKCS#1: `-----BEGIN RSA PRIVATE KEY-----`
- PKCS#8: `-----BEGIN PRIVATE KEY-----`

App Store Connect provides keys in PKCS#8 format by default.

## AI Response Generation

When you press 'a' to generate an AI response, the tool:

1. Analyzes the review content and rating
2. Generates a contextual response using OpenAI GPT-4.1-nano
3. Incorporates custom keywords naturally when relevant
4. Allows you to edit the response before sending
5. Requires your approval before submitting

### Customizing AI Responses

The AI response generator can be customized by modifying `src/ai.rs`. You can configure:

- **Keywords**: Domain-specific terms to include naturally in responses
- **Support Email**: Your team's support contact for additional feedback
- **Custom Prompt**: Additional instructions for the AI
- **Supporting Info**: Context about your app

#### Example Customization

```rust
impl Default for AIConfig {
    fn default() -> Self {
        Self {
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-4.1-nano".to_string(),
            keywords: vec![
                "your_domain".to_string(),
                "feature".to_string(),
                "users".to_string()
            ],
            support_email: "support@yourapp.com".to_string(),
            custom_prompt: Some("Always mention our latest update".to_string()),
            supporting_info: Some("Our app helps users connect and build relationships".to_string()),
        }
    }
}
```

#### Current Configuration

The tool is currently configured for relationship/dating apps with keywords like:

- relationships
- couples
- love
- partner
- connection

Support contact: candleappteam@gmail.com

## Error Handling

The tool provides error messages for common issues:

- Invalid API credentials
- Network connectivity problems
- Missing private key files
- API rate limiting
- Invalid review IDs

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Development Mode

```bash
cargo run
```

## Dependencies

- `reqwest` - HTTP client for API requests
- `tokio` - Async runtime
- `tui` - Terminal user interface
- `crossterm` - Cross-platform terminal handling
- `jsonwebtoken` - JWT authentication for App Store Connect
- `rsa` - RSA key handling
- `chrono` - Date/time handling
- `serde` - JSON serialization

## License

This project is licensed under the MIT License.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Support

For issues and questions:

- Check existing issues on GitHub
- Review Apple's App Store Connect API documentation
- Ensure your API credentials have the correct permissions
