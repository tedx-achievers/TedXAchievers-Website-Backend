use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RefreshToken {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub token: String,
    pub user_id: ObjectId,
    #[serde(with = "crate::utils::datetime::bson_datetime")]
    pub expires_at: DateTime<Utc>,
    #[serde(with = "crate::utils::datetime::optional_bson_datetime")]
    pub created_at: Option<DateTime<Utc>>,
}
