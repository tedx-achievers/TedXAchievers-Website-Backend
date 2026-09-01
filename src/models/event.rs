use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Event {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub title: String,
    #[serde(with = "crate::utils::datetime::bson_datetime")]
    pub date: DateTime<Utc>,
    pub venue: String,
    pub capacity: u32,
    pub price: f64,
    pub status: EventStatus,
    #[serde(with = "crate::utils::datetime::optional_bson_datetime")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::utils::datetime::optional_bson_datetime")]
    pub updated_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub enum EventStatus {
    Active,
    Closed,
}
