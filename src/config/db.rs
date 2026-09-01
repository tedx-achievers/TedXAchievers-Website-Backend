use crate::config::Config;
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, IndexOptions},
    Client, Database, IndexModel,
};
use tracing::info;

const DATE_FIELDS: &[(&str, &[&str])] = &[
    (
        "users",
        &[
            "createdAt",
            "updatedAt",
            "emailVerificationCodeExpiry",
            "passwordResetCodeExpiry",
            "setPasswordTokenExpiry",
        ],
    ),
    ("tickets", &["createdAt", "updatedAt", "checkedInAt"]),
    ("refresh_tokens", &["expiresAt", "createdAt"]),
    ("volunteer_applications", &["createdAt", "updatedAt"]),
];

async fn migrate_date_field(
    database: &Database,
    collection_name: &str,
    field: &str,
) -> Result<(), mongodb::error::Error> {
    let collection = database.collection::<Document>(collection_name);
    let mut set = Document::new();
    set.insert(
        field,
        doc! {
            "$convert": {
                "input": format!("${field}"),
                "to": "date",
                "onError": format!("${field}"),
                "onNull": mongodb::bson::Bson::Null,
            }
        },
    );
    let mut update = Document::new();
    update.insert("$set", set);
    let mut filter = Document::new();
    filter.insert(field, doc! { "$type": "string" });
    let result = collection.update_many(filter, vec![update], None).await?;
    if result.modified_count > 0 {
        info!(
            collection = collection_name,
            field,
            modified = result.modified_count,
            "Converted legacy date strings"
        );
    }
    Ok(())
}

async fn verify_date_fields(database: &Database) -> Result<(), mongodb::error::Error> {
    for (collection_name, fields) in DATE_FIELDS {
        let collection = database.collection::<Document>(collection_name);
        for field in *fields {
            let mut filter = Document::new();
            filter.insert(*field, doc! { "$type": "string" });
            let remaining = collection.count_documents(filter, None).await?;
            if remaining > 0 {
                panic!(
                    "Date migration left {remaining} string value(s) in {collection_name}.{field}"
                );
            }
        }
    }
    Ok(())
}

async fn migrate_and_verify_dates(database: &Database) {
    for (collection_name, fields) in DATE_FIELDS {
        for field in *fields {
            migrate_date_field(database, collection_name, field)
                .await
                .unwrap_or_else(|error| {
                    panic!("Failed to migrate {collection_name}.{field} to BSON DateTime: {error}")
                });
        }
    }
    verify_date_fields(database)
        .await
        .unwrap_or_else(|error| panic!("Failed to verify MongoDB date fields: {error}"));
}

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
    let ttl = IndexModel::builder()
        .keys(doc! { "expiresAt": 1 })
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
                unique_sparse_named("setPasswordToken", "user_set_password_token_unique"),
            ],
        ),
        (
            "tickets",
            vec![
                unique("ticketCode"),
                unique("paymentRef"),
                IndexModel::builder().keys(doc! { "userId": 1 }).build(),
                normal("status"),
            ],
        ),
        (
            "refresh_tokens",
            vec![unique("token"), normal("userId"), ttl],
        ),
        (
            "volunteer_applications",
            vec![
                unique_named("email", "volunteer_email_unique"),
                unique_sparse_named("referenceCode", "volunteer_reference_unique"),
                normal("department"),
                normal("preferredRole"),
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
    migrate_and_verify_dates(&database).await;
    info!("MongoDB connected and indexes created");
    database
}
