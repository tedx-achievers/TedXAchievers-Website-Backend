use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLog {
    #[serde(rename = "_id")]
    pub id: String,
    pub event_type: String,
    pub actor: Option<String>,
    pub metadata: serde_json::Value,
    #[serde(with = "crate::utils::datetime::bson_datetime")]
    pub created_at: DateTime<Utc>,
}
