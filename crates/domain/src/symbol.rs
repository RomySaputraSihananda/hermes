use rust_decimal::Decimal;
use serde::Deserialize;

use crate::serde_helpers;

#[derive(Debug, Clone, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub description: String,
    pub digits: u8,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub point: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub bid: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub ask: Decimal,
    pub spread: i32,
    pub spread_float: bool,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub volume_min: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub volume_max: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub volume_step: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub trade_contract_size: Decimal,
    pub currency_base: String,
    pub currency_profit: String,
    pub category: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_subset_ignores_extra_fields() {
        // JSON real from MT5 bridge /symbols — BCHUSDm has 80+ fields, we only capture 15
        let json = r#"{
            "name": "BCHUSDm",
            "description": "Bitcoin Cash vs US Dollar",
            "digits": 2,
            "point": 0.01,
            "bid": 202.51,
            "ask": 217.51,
            "spread": 1500,
            "spread_float": true,
            "volume_min": 0.1,
            "volume_max": 20.0,
            "volume_step": 0.01,
            "trade_contract_size": 1.0,
            "currency_base": "BCH",
            "currency_profit": "USD",
            "category": "Crypto",
            "custom": false,
            "chart_mode": 0,
            "select": true,
            "visible": true,
            "session_deals": 0,
            "unknown_future_field": "ignored"
        }"#;
        let s: Symbol = serde_json::from_str(json).unwrap();
        assert_eq!(s.name, "BCHUSDm");
        assert_eq!(s.digits, 2);
        assert_eq!(s.bid,  "202.51".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(s.ask,  "217.51".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(s.currency_base, "BCH");
        assert_eq!(s.category, "Crypto");
    }
}
