# _rustpond_

a CLI tool built in Rust for managing and responding to Apple App Store reviews using the App Store Connect API. Support for Google Play store pending - contributions welcome!

## Features

- **Review Navigation**: Browse reviews with arrow keys in a terminal UI
- **Manual Responses**: Type custom responses to reviews
- **AI-Generated Responses**: Press 'a' to generate AI responses (requires approval)
- **Response Approval**: Review and approve responses before sending
- **Real-time Updates**: Refresh reviews and see the latest feedback

## Prerequisites

1. **App Store Connect API Credentials**:
   - App Store Connect API Key ID
   - Issuer ID
   - Private Key file (.p8 format)
   - Your app's App Store ID

2. **Optional - OpenAI API Key** (for AI responses):
   - OpenAI API key for generating automated responses

## Installation

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd apple-review-responder
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
   ```bash
   # Required: Your app's App Store ID
   APP_STORE_APP_ID=1234567890
   
   # Required: App Store Connect API credentials
   APP_STORE_CONNECT_KEY_ID=ABCD123456
   APP_STORE_CONNECT_ISSUER_ID=12345678-1234-1234-1234-123456789012
   APP_STORE_CONNECT_PRIVATE_KEY_PATH=/path/to/your/AuthKey_ABCD123456.p8
   
   # Optional: OpenAI API key for AI responses
   OPENAI_API_KEY=sk-your-openai-api-key-here
   ```

### Using Environment Variables

Alternatively, set environment variables directly:

```bash
export APP_STORE_APP_ID="your_app_id"
export APP_STORE_CONNECT_KEY_ID="your_key_id"
export APP_STORE_CONNECT_ISSUER_ID="your_issuer_id"
export APP_STORE_CONNECT_PRIVATE_KEY_PATH="/path/to/your/private_key.p8"
export OPENAI_API_KEY="your_openai_api_key"  # Optional, for AI responses
```

### Command Line Arguments

Alternatively, you can pass credentials as command-line arguments:

```bash
./target/release/apple-review-responder \
  --app-id "your_app_id" \
  --key-id "your_key_id" \
  --issuer-id "your_issuer_id" \
  --private-key "/path/to/your/private_key.p8"
```

## Quick Start

1. **Set up credentials**:
   ```bash
   cp .env.example .env
   # Edit .env with your App Store Connect credentials
   ```

2. **Run the application**:
   ```bash
   cargo run
   # or if built
   ./target/release/apple-review-responder
   ```

## Usage

### Controls

**Review Navigation:**
- `↑/↓` - Navigate between reviews
- `Enter` - Write a manual response to the selected review
- `a` - Generate an AI response for the selected review
- `r` - Refresh reviews from App Store Connect
- `q` - Quit the application

**Response Writing:**
- Type your response in the text area
- `Ctrl+Enter` - Submit response for approval
- `Esc` - Cancel and return to review list

**Response Approval:**
- `y` - Approve and send the response
- `n` or `Esc` - Go back to edit the response

## App Store Connect API Setup

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

## Private Key Format

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
