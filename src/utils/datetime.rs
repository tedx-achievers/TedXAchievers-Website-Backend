use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod bson_datetime {
    use super::*;

    pub fn serialize<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            value.serialize(serializer)
        } else {
            mongodb::bson::DateTime::from_chrono(*value).serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            DateTime::<Utc>::deserialize(deserializer)
        } else {
            Ok(mongodb::bson::DateTime::deserialize(deserializer)?.to_chrono())
        }
    }
}

pub mod optional_bson_datetime {
    use super::*;

    pub fn serialize<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            value.serialize(serializer)
        } else {
            value
                .map(mongodb::bson::DateTime::from_chrono)
                .serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            Option::<DateTime<Utc>>::deserialize(deserializer)
        } else {
            Ok(
                Option::<mongodb::bson::DateTime>::deserialize(deserializer)?
                    .map(|value| value.to_chrono()),
            )
        }
    }
}
