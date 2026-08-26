use chrono::{DateTime, SecondsFormat, Utc};

pub fn sanitize_name(name: &str) -> String {
    let mut sanitized = String::new();
    let mut replaced = false;
    for character in name.chars() {
        if character.is_alphanumeric() || matches!(character, '.' | '_' | '-') {
            sanitized.push(character);
            replaced = false;
        } else if !replaced {
            sanitized.push('_');
            replaced = true;
        }
    }
    if sanitized.is_empty() {
        "_".to_owned()
    } else {
        sanitized
    }
}

pub fn timestamp_name(received_at: DateTime<Utc>) -> String {
    received_at
        .to_rfc3339_opts(SecondsFormat::Micros, true)
        .replace(['-', ':'], "")
}

pub fn archive_filename(received_at: DateTime<Utc>, hash: &str, original_name: &str) -> String {
    format!(
        "{}--{}--{}",
        timestamp_name(received_at),
        hash,
        sanitize_name(original_name)
    )
}
