use chrono::{DateTime, Utc};
use domain::Candle;
use rust_decimal::Decimal;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SwingKind {
    High,
    Low,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct SwingPoint {
    pub(crate) index: usize,
    pub(crate) price: Decimal,
    pub(crate) kind: SwingKind,
    pub(crate) time: DateTime<Utc>,
}

/// N-candle pivot detection. Requires at least 2*n+1 candles.
#[allow(dead_code)]
pub(crate) fn detect_swings(candles: &[Candle], n: usize) -> Vec<SwingPoint> {
    if candles.len() < 2 * n + 1 {
        return vec![];
    }
    let mut result = Vec::new();
    for i in n..candles.len() - n {
        let left = &candles[i - n..i];
        let right = &candles[i + 1..=i + n];
        let c = &candles[i];

        let is_high = left.iter().all(|x| x.high < c.high)
            && right.iter().all(|x| x.high < c.high);
        let is_low = left.iter().all(|x| x.low > c.low)
            && right.iter().all(|x| x.low > c.low);

        if is_high {
            result.push(SwingPoint {
                index: i,
                price: c.high,
                kind: SwingKind::High,
                time: c.time,
            });
        }
        if is_low {
            result.push(SwingPoint {
                index: i,
                price: c.low,
                kind: SwingKind::Low,
                time: c.time,
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candle(o: &str, h: &str, l: &str, c: &str) -> domain::Candle {
        domain::Candle {
            time: chrono::DateTime::default(),
            open: o.parse().unwrap(),
            high: h.parse().unwrap(),
            low: l.parse().unwrap(),
            close: c.parse().unwrap(),
            tick_volume: 100,
            spread: 10,
            real_volume: 0,
        }
    }

    #[test]
    fn swing_high_detected() {
        // candle[2] has the highest high in a 5-candle window (n=2)
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.2", "0.9", "1.0"),
            make_candle("1.0", "1.5", "0.9", "1.0"), // pivot high
            make_candle("1.0", "1.2", "0.9", "1.0"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
        ];
        let swings = detect_swings(&candles, 2);
        assert!(swings
            .iter()
            .any(|s| s.index == 2 && s.kind == SwingKind::High));
    }

    #[test]
    fn swing_low_detected() {
        // candle[2] has the lowest low
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.1", "0.8", "1.0"),
            make_candle("1.0", "1.1", "0.5", "1.0"), // pivot low
            make_candle("1.0", "1.1", "0.8", "1.0"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
        ];
        let swings = detect_swings(&candles, 2);
        assert!(swings
            .iter()
            .any(|s| s.index == 2 && s.kind == SwingKind::Low));
    }

    #[test]
    fn insufficient_candles_returns_empty() {
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.2", "0.8", "1.0"),
            make_candle("1.0", "1.3", "0.7", "1.0"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
        ]; // only 4 candles, need 2*2+1=5
        let swings = detect_swings(&candles, 2);
        assert!(swings.is_empty());
    }
}
