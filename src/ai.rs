use anyhow::{anyhow, Result};
use async_openai::{
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs},
    Client,
};
use serde::{Deserialize, Serialize};
use std::env;

use crate::review::Review;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub openai_api_key: String,
    pub model: String,
    pub keywords: Vec<String>,
    pub support_email: String,
    pub custom_prompt: Option<String>,
    pub supporting_info: Option<String>,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-4.1-nano".to_string(),
            keywords: vec!["relationships".to_string(), "couples".to_string(), "love".to_string(), "partner".to_string(), "connection".to_string()],
            support_email: "candleappteam@gmail.com".to_string(),
            custom_prompt: Some("Try to encourage users to join our Reddit at r/candleapp when possible.".to_string()),
            supporting_info: None,
        }
    }
}

pub struct AIResponseGenerator {
    client: Client<async_openai::config::OpenAIConfig>,
    config: AIConfig,
}

impl AIResponseGenerator {
    pub fn new(config: AIConfig) -> Result<Self> {
        if config.openai_api_key.is_empty() {
            return Err(anyhow!("OpenAI API key is required. Set OPENAI_API_KEY environment variable"));
        }

        let client = Client::with_config(
            async_openai::config::OpenAIConfig::new().with_api_key(&config.openai_api_key)
        );

        Ok(Self { client, config })
    }

    pub async fn generate_response(&self, review: &Review) -> Result<String> {
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(review);

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages([
                ChatCompletionRequestMessage::System(async_openai::types::ChatCompletionRequestSystemMessage {
                    content: system_prompt.into(),
                    name: None,
                }),
                ChatCompletionRequestMessage::User(async_openai::types::ChatCompletionRequestUserMessage {
                    content: user_prompt.into(),
                    name: None,
                }),
            ])
            .max_tokens(500u32)
            .temperature(0.7)
            .build()?;

        let response = self.client.chat().create(request).await?;

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| anyhow!("No response content from OpenAI"))?;

        Ok(content.clone())
    }

    fn build_system_prompt(&self) -> String {
        let keywords_text = if !self.config.keywords.is_empty() {
            format!("\n- Naturally incorporate these keywords when relevant: {}", self.config.keywords.join(", "))
        } else {
            String::new()
        };

        let support_text = format!("\n- Encourage users to email {} for additional feedback or feature requests", self.config.support_email);

        let custom_instructions = if let Some(ref custom) = self.config.custom_prompt {
            format!("\n- Additional instructions: {}", custom)
        } else {
            String::new()
        };

        let supporting_info = if let Some(ref info) = self.config.supporting_info {
            format!("\n- Context about the app: {}", info)
        } else {
            String::new()
        };

        format!(
            "You are a professional app developer responding to App Store reviews. Your responses should be:
- Professional, friendly, and appreciative
- Acknowledge the user's specific feedback
- Keep responses under 350 characters (App Store limit)
- Thank users for their time and feedback{}{}{}{}

Always be genuine and avoid overly promotional language.",
            keywords_text,
            support_text,
            custom_instructions,
            supporting_info
        )
    }

    fn build_user_prompt(&self, review: &Review) -> String {
        let rating_context = match review.rating {
            5 => "This is a 5-star positive review",
            4 => "This is a 4-star mostly positive review",
            3 => "This is a 3-star neutral review",
            2 => "This is a 2-star negative review", 
            1 => "This is a 1-star very negative review",
            _ => "This is a review",
        };

        let title_text = review.title.as_deref().unwrap_or("(No title)");
        let body_text = review.body.as_deref().unwrap_or("(No review text)");

        format!(
            "{}.

Review title: \"{}\"
Review text: \"{}\"

Please generate a professional response to this review.",
            rating_context,
            title_text,
            body_text
        )
    }

}