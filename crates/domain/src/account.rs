use rust_decimal::Decimal;
use serde::Deserialize;

use crate::serde_helpers;

#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub login: u64,
    pub leverage: u32,
    pub trade_allowed: bool,
    pub trade_expert: bool,
    pub currency: String,
    pub currency_digits: u8,
    pub server: String,
    pub name: String,
    pub company: String,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub balance: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub equity: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub profit: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub credit: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub margin: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub margin_free: Decimal,
    #[serde(deserialize_with = "serde_helpers::de_decimal")]
    pub margin_level: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_from_api_json() {
        // JSON real from MT5 bridge /account endpoint (live data)
        let json = r#"{
            "login": 415817698,
            "trade_mode": 0,
            "leverage": 2000,
            "limit_orders": 1024,
            "margin_so_mode": 0,
            "trade_allowed": true,
            "trade_expert": true,
            "margin_mode": 2,
            "currency_digits": 2,
            "fifo_close": false,
            "balance": 5000.0,
            "credit": 0.0,
            "profit": 0.0,
            "equity": 5000.0,
            "margin": 0.0,
            "margin_free": 5000.0,
            "margin_level": 0.0,
            "margin_so_call": 60.0,
            "name": "Standard",
            "server": "Exness-MT5Trial14",
            "currency": "USD",
            "company": "Exness Technologies Ltd"
        }"#;
        let a: AccountInfo = serde_json::from_str(json).unwrap();
        assert_eq!(a.login, 415817698);
        assert_eq!(a.leverage, 2000);
        assert_eq!(a.balance, "5000".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(a.currency, "USD");
        assert_eq!(a.server, "Exness-MT5Trial14");
        assert!(a.trade_allowed);
    }
}
