#![allow(dead_code)]

use rust_decimal::Decimal;
use serde::Deserializer;

pub(crate) fn de_decimal<'de, D: Deserializer<'de>>(de: D) -> Result<Decimal, D::Error> {
    use serde::de::Visitor;
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Decimal;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a number")
        }
        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Decimal, E> {
            Decimal::try_from(v).map_err(E::custom)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Decimal, E> {
            Ok(Decimal::from(v))
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Decimal, E> {
            Ok(Decimal::from(v))
        }
    }
    de.deserialize_any(V)
}

pub(crate) mod naive_utc_secs {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    const FMT: &str = "%Y-%m-%dT%H:%M:%S";

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<DateTime<Utc>, D::Error> {
        let s = String::deserialize(de)?;
        NaiveDateTime::parse_from_str(&s, FMT)
            .map(|ndt| ndt.and_utc())
            .map_err(serde::de::Error::custom)
    }

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, se: S) -> Result<S::Ok, S::Error> {
        se.serialize_str(&dt.format(FMT).to_string())
    }
}

pub(crate) mod naive_utc_ms {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    const FMT_PARSE: &str = "%Y-%m-%dT%H:%M:%S%.f";   // accept any precision when reading
    const FMT_EMIT: &str  = "%Y-%m-%dT%H:%M:%S%.3f";  // always write exactly .NNN

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<DateTime<Utc>, D::Error> {
        let s = String::deserialize(de)?;
        NaiveDateTime::parse_from_str(&s, FMT_PARSE)
            .map(|ndt| ndt.and_utc())
            .map_err(serde::de::Error::custom)
    }

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, se: S) -> Result<S::Ok, S::Error> {
        se.serialize_str(&dt.format(FMT_EMIT).to_string())
    }
}
