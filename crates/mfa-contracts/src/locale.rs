use crate::ContractError;
use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDate(pub NaiveDate);

impl LocalDate {
    pub fn as_naive(&self) -> NaiveDate {
        self.0
    }
}

impl From<NaiveDate> for LocalDate {
    fn from(value: NaiveDate) -> Self {
        Self(value)
    }
}

impl FromStr for LocalDate {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Self)
            .map_err(|error| ContractError::new("invalid_local_date", error.to_string()))
    }
}

impl fmt::Display for LocalDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.format("%Y-%m-%d"))
    }
}

impl Serialize for LocalDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LocalDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime(pub NaiveDateTime);

impl LocalDateTime {
    pub fn as_naive(&self) -> NaiveDateTime {
        self.0
    }
}

impl From<NaiveDateTime> for LocalDateTime {
    fn from(value: NaiveDateTime) -> Self {
        Self(value)
    }
}

impl FromStr for LocalDateTime {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
            .iter()
            .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
            .map(Self)
            .ok_or_else(|| ContractError::new("invalid_local_datetime", value))
    }
}

impl fmt::Display for LocalDateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.format("%Y-%m-%dT%H:%M:%S%.f"))
    }
}

impl Serialize for LocalDateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LocalDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcInstant(pub DateTime<Utc>);

impl UtcInstant {
    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
}

impl From<DateTime<Utc>> for UtcInstant {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

impl FromStr for UtcInstant {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        DateTime::parse_from_rfc3339(value)
            .map(|date| Self(date.with_timezone(&Utc)))
            .map_err(|error| ContractError::new("invalid_utc_instant", error.to_string()))
    }
}

impl fmt::Display for UtcInstant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0.to_rfc3339_opts(SecondsFormat::AutoSi, true))
    }
}

impl Serialize for UtcInstant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for UtcInstant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}
