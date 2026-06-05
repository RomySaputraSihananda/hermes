use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};

use crate::serde_helpers;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Long,
    Short,
}

fn de_side<'de, D: Deserializer<'de>>(de: D) -> Result<Side, D::Error> {
    match u8::deserialize(de)? {
        0 => Ok(Side::Long),
        1 => Ok(Side::Short),
        n => Err(serde::de::Error::custom(format!("unknown position type: {n}"))),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub ticket: u64,
    pub symbol: String,
    #[serde(rename = "type", deserialize_with = "de_side")]
    pub side: Side,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub volume: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub price_open: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub sl: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub tp: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub price_current: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub swap: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub profit: Decimal,
    pub comment: String,
    pub magic: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_type_0_maps_to_long() {
        // MT5 "type": 0 = BUY = Long. Extra fields (time, identifier, reason) are ignored by serde.
        let json = r#"{
            "ticket": 123456789,
            "symbol": "BTCUSDm",
            "type": 0,
            "volume": 0.1,
            "price_open": 59000.0,
            "sl": 58000.0,
            "tp": 61000.0,
            "price_current": 59500.0,
            "swap": -5.0,
            "profit": 50.0,
            "comment": "test",
            "magic": 0,
            "time": "2026-06-01T10:00:00",
            "identifier": 123456789,
            "reason": 0
        }"#;
        let p: Position = serde_json::from_str(json).unwrap();
        assert_eq!(p.side, Side::Long);
        assert_eq!(p.ticket, 123456789);
        assert_eq!(p.symbol, "BTCUSDm");
        assert_eq!(p.volume, "0.1".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(p.sl, "58000".parse::<rust_decimal::Decimal>().unwrap());
    }

    #[test]
    fn position_type_1_maps_to_short() {
        let json = r#"{
            "ticket": 999,
            "symbol": "ETHUSDm",
            "type": 1,
            "volume": 0.5,
            "price_open": 1500.0,
            "sl": 1600.0,
            "tp": 1400.0,
            "price_current": 1490.0,
            "swap": 0.0,
            "profit": 5.0,
            "comment": "",
            "magic": 42,
            "time": "2026-06-01T12:00:00",
            "identifier": 999,
            "reason": 0
        }"#;
        let p: Position = serde_json::from_str(json).unwrap();
        assert_eq!(p.side, Side::Short);
        assert_eq!(p.magic, 42);
    }

    #[test]
    fn side_serializes_as_lowercase() {
        assert_eq!(serde_json::to_string(&Side::Long).unwrap(),  r#""long""#);
        assert_eq!(serde_json::to_string(&Side::Short).unwrap(), r#""short""#);
    }
}
