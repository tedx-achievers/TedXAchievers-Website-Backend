use crate::config::Config;
use mongodb::{
    bson::doc,
    options::{ClientOptions, IndexOptions},
    Client, Database, IndexModel,
};
use tracing::info;

pub async fn connect_db(config: &Config) -> Database {
    let options = ClientOptions::parse(&config.mongodb_uri)
        .await
        .unwrap_or_else(|error| panic!("Failed to parse MongoDB URI: {error}"));
    let client = Client::with_options(options)
        .unwrap_or_else(|error| panic!("Failed to create MongoDB client: {error}"));
    let database = client.database("tedxachievers");
    let unique = |field: &str| {
        IndexModel::builder()
            .keys(doc! { field: 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build()
    };
    let unique_named = |field: &str, name: &str| {
        IndexModel::builder()
            .keys(doc! { field: 1 })
            .options(
                IndexOptions::builder()
                    .name(name.to_owned())
                    .unique(true)
                    .build(),
            )
            .build()
    };
    let unique_sparse_named = |field: &str, name: &str| {
        IndexModel::builder()
            .keys(doc! { field: 1 })
            .options(
                IndexOptions::builder()
                    .name(name.to_owned())
                    .unique(true)
                    .sparse(true)
                    .build(),
            )
            .build()
    };
    let normal = |field: &str| IndexModel::builder().keys(doc! { field: 1 }).build();
    let sparse = |field: &str| {
        IndexModel::builder()
            .keys(doc! { field: 1 })
            .options(IndexOptions::builder().sparse(true).build())
            .build()
    };
    let ttl = IndexModel::builder()
        .keys(doc! { "expires_at": 1 })
        .options(
            IndexOptions::builder()
                .expire_after(std::time::Duration::from_secs(0))
                .build(),
        )
        .build();
    let indexes = [
        (
            "users",
            vec![
                unique("email"),
                sparse("verify_token"),
                sparse("reset_token"),
            ],
        ),
        (
            "tickets",
            vec![
                unique("ticket_code"),
                IndexModel::builder().keys(doc! { "user_id": 1 }).build(),
            ],
        ),
        (
            "refresh_tokens",
            vec![unique("token"), normal("user_id"), ttl],
        ),
        (
            "volunteer_applications",
            vec![
                unique_named("email", "volunteer_email_unique"),
                unique_sparse_named("reference_code", "volunteer_reference_unique"),
                normal("department"),
                normal("preferred_role"),
                normal("status"),
            ],
        ),
    ];
    for (collection, models) in indexes {
        let collection_handle = database.collection::<mongodb::bson::Document>(collection);
        if collection == "volunteer_applications" {
            // Older deployments created a non-unique email_1 index. Remove it
            // so the unique replacement below can be created on startup.
            let _ = collection_handle.drop_index("email_1", None).await;
        }
        collection_handle
            .create_indexes(models, None)
            .await
            .unwrap_or_else(|error| panic!("Failed to create indexes for {collection}: {error}"));
    }
    let applications = database.collection::<mongodb::bson::Document>("volunteer_applications");
    let counters = database.collection::<mongodb::bson::Document>("volunteer_role_counters");
    let preferred_roles = applications
        .distinct("preferred_role", None, None)
        .await
        .unwrap_or_else(|error| panic!("Failed to read volunteer preferred roles: {error}"));
    for preferred_role in preferred_roles {
        if let mongodb::bson::Bson::String(preferred_role) = preferred_role {
            let count = applications
                .count_documents(doc! { "preferred_role": &preferred_role }, None)
                .await
                .unwrap_or_else(|error| {
                    panic!("Failed to count volunteer applications for {preferred_role}: {error}")
                });
            counters
                .update_one(
                    doc! { "_id": &preferred_role },
                    doc! { "$set": { "count": count as i64 } },
                    mongodb::options::UpdateOptions::builder()
                        .upsert(true)
                        .build(),
                )
                .await
                .unwrap_or_else(|error| {
                    panic!("Failed to synchronize volunteer role counter for {preferred_role}: {error}")
                });
        }
    }
    info!("MongoDB connected and indexes created");
    database
}
