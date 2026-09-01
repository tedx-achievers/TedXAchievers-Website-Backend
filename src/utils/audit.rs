use chrono::Utc;
use mongodb::Database;
use tracing::{error, info};
use uuid::Uuid;

use crate::{errors::AppError, models::audit_log::AuditLog};

pub async fn log_event(
    db: &Database,
    event_type: &str,
    actor: Option<&str>,
    metadata: serde_json::Value,
) -> Result<(), AppError> {
    let log = AuditLog {
        id: Uuid::new_v4().to_string(),
        event_type: event_type.to_owned(),
        actor: actor.map(str::to_owned),
        metadata,
        created_at: Utc::now(),
    };
    match db
        .collection::<AuditLog>("audit_logs")
        .insert_one(log, None)
        .await
    {
        Ok(_) => {
            info!(event_type, "Audit log recorded");
        }
        Err(error) => {
            error!(%error, event_type, "Failed to record audit log");
        }
    }
    Ok(())
}
