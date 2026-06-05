use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::serde_helpers;

#[derive(Debug, Clone, Deserialize)]
pub struct Candle {
    #[serde(with = "serde_helpers::naive_utc_secs")]
    pub time: DateTime<Utc>,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub open: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub high: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub low: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub close: Decimal,
    pub tick_volume: u64,
    pub spread: i32,
    pub real_volume: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn deserializes_from_api_json() {
        // JSON real from MT5 bridge /rates/from-pos?symbol=BTCUSDm&timeframe=TIMEFRAME_M5
        let json = r#"{
            "time": "2026-06-05T19:05:00",
            "open": 59374.08,
            "high": 59490.02,
            "low": 59225.56,
            "close": 59377.06,
            "tick_volume": 1869,
            "spread": 1008,
            "real_volume": 0
        }"#;
        let c: Candle = serde_json::from_str(json).unwrap();
        assert_eq!(c.time, chrono::Utc.with_ymd_and_hms(2026, 6, 5, 19, 5, 0).unwrap());
        assert_eq!(c.open,  "59374.08".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(c.high,  "59490.02".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(c.low,   "59225.56".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(c.close, "59377.06".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(c.tick_volume, 1869);
        assert_eq!(c.spread, 1008);
        assert_eq!(c.real_volume, 0);
    }
}
