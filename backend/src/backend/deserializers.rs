use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Helper function to deserialize integer (0/1) or boolean to boolean
/// Accepts both u8 (0/1) from Steam API and boolean from frontend
pub fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Bool(b) => Ok(b),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                match u {
                    0 => Ok(false),
                    1 => Ok(true),
                    other => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(other),
                        &"0 or 1",
                    )),
                }
            } else {
                Err(serde::de::Error::invalid_type(
                    serde::de::Unexpected::Other("number that cannot be represented as u64"),
                    &"a boolean or u8 (0/1)",
                ))
            }
        }
        other => Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Other(&format!("{:?}", other)),
            &"a boolean or u8 (0/1)",
        )),
    }
}

/// Helper function to deserialize string or number to u64
pub fn u64_from_str_or_int<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => s.parse::<u64>().map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&s),
                &"a string representing a u64",
            )
        }),
        Value::Number(n) => n.as_u64().ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Other("number that cannot be represented as u64"),
                &"a u64",
            )
        }),
        other => Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Other(&format!("{:?}", other)),
            &"a string or number",
        )),
    }
}

/// Helper function to deserialize string or number to i64
pub fn i64_from_str_or_int<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => s.parse::<i64>().map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&s),
                &"a string representing an i64",
            )
        }),
        Value::Number(n) => n.as_i64().ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Other("number that cannot be represented as i64"),
                &"an i64",
            )
        }),
        other => Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Other(&format!("{:?}", other)),
            &"a string or number",
        )),
    }
}

/// Helper function to deserialize string or number to i32
pub fn i32_from_str_or_int<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => s.parse::<i32>().map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&s),
                &"a string representing an i32",
            )
        }),
        Value::Number(n) => n.as_i64().and_then(|i| i32::try_from(i).ok()).ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Other("number that cannot be represented as i32"),
                &"an i32",
            )
        }),
        other => Err(serde::de::Error::invalid_type(
            serde::de::Unexpected::Other(&format!("{:?}", other)),
            &"a string or number",
        )),
    }
}

