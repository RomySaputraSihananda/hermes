mod types;
use rust_decimal::Decimal;
pub use types::{RiskConfig, RiskDecision};

pub fn evaluate(
    account: &domain::AccountInfo,
    signal: &ict::TradeSignal,
    config: &RiskConfig,
) -> RiskDecision {
    let sl_distance = (signal.entry - signal.sl).abs();

    if sl_distance == Decimal::ZERO {
        return RiskDecision {
            volume: Decimal::ZERO,
            approved: false,
            reason: Some("SL equals entry".to_string()),
        };
    }

    let risk_amount = account.balance * config.risk_pct;
    let raw_volume  = risk_amount / (sl_distance * config.value_per_lot);

    // floor to nearest volume_step
    let volume = (raw_volume / config.volume_step).floor() * config.volume_step;

    if volume < config.min_volume {
        return RiskDecision {
            volume: Decimal::ZERO,
            approved: false,
            reason: Some("volume below minimum".to_string()),
        };
    }

    let volume = volume.min(config.max_volume);

    // NOTE: Only checks margin is non-zero. Actual margin required for `volume`
    // lots is not computed here — the broker will reject on entry if insufficient.
    if account.margin_free <= Decimal::ZERO {
        return RiskDecision {
            volume: Decimal::ZERO,
            approved: false,
            reason: Some("insufficient margin".to_string()),
        };
    }

    RiskDecision {
        volume,
        approved: true,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_account(balance: &str, margin_free: &str) -> domain::AccountInfo {
        domain::AccountInfo {
            login: 1,
            leverage: 100,
            trade_allowed: true,
            trade_expert: true,
            currency: "USD".to_string(),
            currency_digits: 2,
            server: "Test".to_string(),
            name: "Test".to_string(),
            company: "Test".to_string(),
            balance:      balance.parse().unwrap(),
            equity:       balance.parse().unwrap(),
            profit:       "0".parse().unwrap(),
            credit:       "0".parse().unwrap(),
            margin:       "0".parse().unwrap(),
            margin_free:  margin_free.parse().unwrap(),
            margin_level: "0".parse().unwrap(),
        }
    }

    fn make_signal(entry: &str, sl: &str) -> ict::TradeSignal {
        ict::TradeSignal {
            side: domain::Side::Long,
            entry: entry.parse().unwrap(),
            sl:    sl.parse().unwrap(),
            tp:    "1.2000".parse().unwrap(),
            confluence: ict::ConfluenceFlags::default(),
        }
    }

    fn make_config() -> RiskConfig {
        RiskConfig {
            risk_pct:      "0.01".parse().unwrap(),
            value_per_lot: "10000".parse().unwrap(),
            min_volume:    "0.01".parse().unwrap(),
            max_volume:    "10.0".parse().unwrap(),
            volume_step:   "0.01".parse().unwrap(),
        }
    }

    #[test]
    fn calculates_volume_correctly() {
        // balance=5000, risk_pct=1% → risk_amount=50
        // sl_distance=0.0050, value_per_lot=10000
        // raw_volume = 50 / (0.005 * 10000) = 1.00
        let account = make_account("5000", "5000");
        let signal  = make_signal("1.1000", "1.0950");
        let config  = make_config();
        let d = evaluate(&account, &signal, &config);
        assert!(d.approved);
        assert_eq!(d.volume, "1.00".parse::<Decimal>().unwrap());
        assert!(d.reason.is_none());
    }

    #[test]
    fn volume_floored_to_step() {
        // risk_amount=50, value_per_lot=1000, sl_distance=1.35
        // raw_volume = 50 / (1.35 * 1000) = 0.037037...
        // floor(0.037037 / 0.01) * 0.01 = 0.03  (NOT 0.04)
        let account = make_account("5000", "5000");
        let signal  = make_signal("10.00", "8.65"); // sl_distance = 1.35
        let config  = RiskConfig {
            risk_pct:      "0.01".parse().unwrap(),
            value_per_lot: "1000".parse().unwrap(),
            min_volume:    "0.01".parse().unwrap(),
            max_volume:    "10.0".parse().unwrap(),
            volume_step:   "0.01".parse().unwrap(),
        };
        let d = evaluate(&account, &signal, &config);
        assert!(d.approved);
        assert_eq!(d.volume, "0.03".parse::<Decimal>().unwrap());
    }

    #[test]
    fn rejects_when_sl_equals_entry() {
        let account = make_account("5000", "5000");
        let signal  = make_signal("1.1000", "1.1000"); // sl == entry
        let config  = make_config();
        let d = evaluate(&account, &signal, &config);
        assert!(!d.approved);
        assert_eq!(d.volume, Decimal::ZERO);
        assert!(d.reason.as_deref().unwrap_or("").contains("SL"));
    }

    #[test]
    fn rejects_when_volume_below_minimum() {
        // balance=100, risk_pct=1% → risk_amount=1
        // sl_distance=0.10, value_per_lot=10000
        // raw_volume = 1 / (0.10 * 10000) = 0.001 < min_volume=0.01
        let account = make_account("100", "100");
        let signal  = make_signal("1.1000", "1.0000"); // sl_distance = 0.10
        let config  = make_config();
        let d = evaluate(&account, &signal, &config);
        assert!(!d.approved);
        assert_eq!(d.volume, Decimal::ZERO);
        assert!(d.reason.as_deref().unwrap_or("").contains("minimum"));
    }

    #[test]
    fn rejects_when_margin_free_zero() {
        let account = make_account("5000", "0"); // margin_free = 0
        let signal  = make_signal("1.1000", "1.0950");
        let config  = make_config();
        let d = evaluate(&account, &signal, &config);
        assert!(!d.approved);
        assert_eq!(d.volume, Decimal::ZERO);
        assert!(d.reason.as_deref().unwrap_or("").contains("margin"));
    }

    #[test]
    fn volume_clamped_to_max() {
        // balance=500000, risk_pct=1% → risk_amount=5000
        // sl_distance=0.0050, value_per_lot=10000
        // raw_volume = 5000 / (0.005 * 10000) = 100.0 > max_volume=10.0
        // → clamped to 10.0, approved=true (NOT rejected)
        let account = make_account("500000", "500000");
        let signal  = make_signal("1.1000", "1.0950"); // sl_distance = 0.0050
        let config  = make_config(); // max_volume = 10.0
        let d = evaluate(&account, &signal, &config);
        assert!(d.approved);
        assert_eq!(d.volume, "10.0".parse::<Decimal>().unwrap());
        assert!(d.reason.is_none());
    }
}
