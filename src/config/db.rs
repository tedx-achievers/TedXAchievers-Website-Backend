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
            vec![normal("email"), normal("status")],
        ),
    ];
    for (collection, models) in indexes {
        database
            .collection::<mongodb::bson::Document>(collection)
            .create_indexes(models, None)
            .await
            .unwrap_or_else(|error| panic!("Failed to create indexes for {collection}: {error}"));
    }
    info!("MongoDB connected and indexes created");
    database
}
