use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: String,
    pub rating: i32,
    pub title: Option<String>,
    pub body: Option<String>,
    pub reviewer_nickname: String,
    pub created_date: DateTime<Utc>,
    pub territory: String,
    pub version: Option<String>,
    pub response: Option<ReviewResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResponse {
    pub id: String,
    pub response_body: String,
    pub last_modified_date: DateTime<Utc>,
    pub state: ResponseState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ResponseState {
    Published,
    Pending,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewsResponse {
    pub data: Vec<ReviewData>,
    pub links: Option<Links>,
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewData {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub attributes: ReviewAttributes,
    pub relationships: Option<ReviewRelationships>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewAttributes {
    pub rating: i32,
    pub title: Option<String>,
    pub body: Option<String>,
    #[serde(rename = "reviewerNickname")]
    pub reviewer_nickname: String,
    #[serde(rename = "createdDate")]
    pub created_date: DateTime<Utc>,
    pub territory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRelationships {
    pub response: Option<ResponseRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseRelationship {
    pub data: Option<ResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Links {
    #[serde(rename = "self")]
    pub self_: Option<String>,
    pub next: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub paging: Option<Paging>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Paging {
    pub total: i32,
    pub limit: i32,
}

impl From<ReviewData> for Review {
    fn from(data: ReviewData) -> Self {
        Review {
            id: data.id,
            rating: data.attributes.rating,
            title: data.attributes.title,
            body: data.attributes.body,
            reviewer_nickname: data.attributes.reviewer_nickname,
            created_date: data.attributes.created_date,
            territory: data.attributes.territory,
            version: None, // This would need to be extracted from relationships if needed
            response: None, // This will be populated on-demand when entering response mode
        }
    }
}