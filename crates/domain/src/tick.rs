use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::serde_helpers;

#[derive(Debug, Clone, Deserialize)]
pub struct Tick {
    #[serde(with = "serde_helpers::naive_utc_secs")]
    pub time: DateTime<Utc>,
    #[serde(with = "serde_helpers::naive_utc_ms")]
    pub time_msc: DateTime<Utc>,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub bid: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub ask: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub last: Decimal,
    pub volume: u64,
    pub flags: u32,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub volume_real: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn deserializes_from_api_json() {
        // JSON real from MT5 bridge /ticks/from?symbol=BTCUSDm
        let json = r#"{
            "time": "2026-06-05T00:00:00",
            "bid": 63801.02,
            "ask": 63811.1,
            "last": 0.0,
            "volume": 0,
            "time_msc": "2026-06-05T00:00:00.234000",
            "flags": 134,
            "volume_real": 0.0
        }"#;
        let t: Tick = serde_json::from_str(json).unwrap();
        assert_eq!(t.time, chrono::Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap());
        assert_eq!(t.bid,  "63801.02".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(t.ask,  "63811.1".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(t.flags, 134);
        // time_msc must preserve 234ms
        assert_eq!(t.time_msc.timestamp_subsec_millis(), 234);
    }
}
