use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub email_verified: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Plan {
    pub id: String,
    pub code: String,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub period: String,
    pub messages_per_month: i32,
    pub features: Vec<String>,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Post {
    pub id: String,
    pub user_id: String,
    pub alias: String,
    pub content: String,
    pub rating: Option<i32>,
    pub status: String,
    pub created_at: String,
    pub plan: Option<String>,
}
