use domain::{Candle, Side};

use crate::swing::{SwingKind, SwingPoint};
use crate::types::{BosChoch, Fvg, LiquiditySweep, OrderBlock, Ote, PdArray, PdZone, StructureEvent};

#[allow(dead_code)]
pub(crate) fn detect_fvg(candles: &[Candle]) -> Vec<Fvg> {
    let mut result = Vec::new();
    for i in 2..candles.len() {
        // Bullish FVG: gap up — candle[i-2].high < candle[i].low
        if candles[i - 2].high < candles[i].low {
            let top = candles[i].low;
            let bottom = candles[i - 2].high;
            let formed_at = candles[i - 1].time;
            let mitigated = candles[i + 1..].iter().any(|c| c.low <= top);
            result.push(Fvg {
                top,
                bottom,
                formed_at,
                side: Side::Long,
                mitigated,
            });
        }
        // Bearish FVG: gap down — candle[i-2].low > candle[i].high
        if candles[i - 2].low > candles[i].high {
            let top = candles[i - 2].low;
            let bottom = candles[i].high;
            let formed_at = candles[i - 1].time;
            let mitigated = candles[i + 1..].iter().any(|c| c.high >= bottom);
            result.push(Fvg {
                top,
                bottom,
                formed_at,
                side: Side::Short,
                mitigated,
            });
        }
    }
    result
}

#[allow(dead_code)]
pub(crate) fn detect_ob(candles: &[Candle], swings: &[SwingPoint]) -> Vec<OrderBlock> {
    let mut result = Vec::new();
    for swing in swings {
        match swing.kind {
            // SwingLow → bullish OB: last bearish candle before swing index
            SwingKind::Low => {
                if let Some((_idx, ob_candle)) = candles[..swing.index]
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, c)| c.close < c.open)
                {
                    let top = ob_candle.high;
                    let bottom = ob_candle.low;
                    let formed_at = ob_candle.time;
                    let mitigated = candles.iter().skip(swing.index).any(|c| c.close < bottom);
                    result.push(OrderBlock {
                        top,
                        bottom,
                        formed_at,
                        side: Side::Long,
                        mitigated,
                    });
                }
            }
            // SwingHigh → bearish OB: last bullish candle before swing index
            SwingKind::High => {
                if let Some((_idx, ob_candle)) = candles[..swing.index]
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, c)| c.close > c.open)
                {
                    let top = ob_candle.high;
                    let bottom = ob_candle.low;
                    let formed_at = ob_candle.time;
                    let mitigated = candles.iter().skip(swing.index).any(|c| c.close > top);
                    result.push(OrderBlock {
                        top,
                        bottom,
                        formed_at,
                        side: Side::Short,
                        mitigated,
                    });
                }
            }
        }
    }
    result
}

#[allow(dead_code)]
pub(crate) fn detect_structure(candles: &[Candle], swings: &[SwingPoint]) -> Vec<BosChoch> {
    if swings.is_empty() {
        return vec![];
    }

    let last_swing_high = swings.iter().rfind(|s| s.kind == SwingKind::High);
    let last_swing_low = swings.iter().rfind(|s| s.kind == SwingKind::Low);

    let mut result = Vec::new();
    let mut last_structure_side: Option<Side> = None;

    for candle in candles {
        if let Some(sh) = last_swing_high
            && candle.close > sh.price
        {
            let kind = if last_structure_side == Some(Side::Short) {
                StructureEvent::Choch
            } else {
                StructureEvent::Bos
            };
            result.push(BosChoch {
                kind,
                level: sh.price,
                broken_at: candle.time,
                side: Side::Long,
            });
            last_structure_side = Some(Side::Long);
        }
        if let Some(sl) = last_swing_low
            && candle.close < sl.price
        {
            let kind = if last_structure_side == Some(Side::Long) {
                StructureEvent::Choch
            } else {
                StructureEvent::Bos
            };
            result.push(BosChoch {
                kind,
                level: sl.price,
                broken_at: candle.time,
                side: Side::Short,
            });
            last_structure_side = Some(Side::Short);
        }
    }
    result
}

#[allow(dead_code)]
pub(crate) fn detect_sweeps(candles: &[Candle], swings: &[SwingPoint]) -> Vec<LiquiditySweep> {
    let mut result = Vec::new();
    for candle in candles {
        for swing in swings {
            match swing.kind {
                SwingKind::High => {
                    if candle.high > swing.price && candle.close < swing.price {
                        result.push(LiquiditySweep {
                            level: swing.price,
                            swept_at: candle.time,
                            side: Side::Short,
                        });
                    }
                }
                SwingKind::Low => {
                    if candle.low < swing.price && candle.close > swing.price {
                        result.push(LiquiditySweep {
                            level: swing.price,
                            swept_at: candle.time,
                            side: Side::Long,
                        });
                    }
                }
            }
        }
    }
    result
}

#[allow(dead_code)]
pub(crate) fn compute_pd(candles: &[Candle], swings: &[SwingPoint]) -> Option<PdArray> {
    let range_high = swings
        .iter()
        .filter(|s| s.kind == SwingKind::High)
        .map(|s| s.price)
        .max()?;
    let range_low = swings
        .iter()
        .filter(|s| s.kind == SwingKind::Low)
        .map(|s| s.price)
        .min()?;
    let equilibrium = (range_high + range_low) / rust_decimal::Decimal::from(2u32);
    let last_close = candles.last()?.close;
    let current_zone = if last_close > equilibrium {
        PdZone::Premium
    } else if last_close < equilibrium {
        PdZone::Discount
    } else {
        PdZone::Equilibrium
    };
    Some(PdArray {
        range_high,
        range_low,
        equilibrium,
        current_zone,
    })
}

#[allow(dead_code)]
pub(crate) fn compute_ote(swings: &[SwingPoint], bias: Side) -> Option<Ote> {
    let fib618: rust_decimal::Decimal = "0.618".parse().unwrap();
    let fib786: rust_decimal::Decimal = "0.786".parse().unwrap();

    let swing_high = swings
        .iter()
        .filter(|s| s.kind == SwingKind::High)
        .map(|s| s.price)
        .max()?;
    let swing_low = swings
        .iter()
        .filter(|s| s.kind == SwingKind::Low)
        .map(|s| s.price)
        .min()?;
    let range = swing_high - swing_low;

    let (top, bottom) = match bias {
        Side::Long => {
            let top = (swing_high - fib618 * range).normalize();
            let bottom = (swing_high - fib786 * range).normalize();
            (top, bottom)
        }
        Side::Short => {
            let top = (swing_low + fib786 * range).normalize();
            let bottom = (swing_low + fib618 * range).normalize();
            (top, bottom)
        }
    };

    Some(Ote { top, bottom, side: bias })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swing::SwingKind;

    fn make_candle(o: &str, h: &str, l: &str, c: &str) -> Candle {
        Candle {
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

    fn make_swing(index: usize, price: &str, kind: SwingKind) -> SwingPoint {
        SwingPoint {
            index,
            price: price.parse().unwrap(),
            kind,
            time: chrono::DateTime::default(),
        }
    }

    #[test]
    fn bullish_fvg() {
        let candles = vec![
            make_candle("1.0", "1.0", "0.9", "1.0"),
            make_candle("1.1", "1.2", "1.0", "1.1"), // middle
            make_candle("1.1", "1.3", "1.1", "1.2"),
        ];
        let fvgs = detect_fvg(&candles);
        assert_eq!(fvgs.len(), 1);
        assert_eq!(fvgs[0].side, Side::Long);
        assert_eq!(fvgs[0].top.to_string(), "1.1");
        assert_eq!(fvgs[0].bottom.to_string(), "1.0");
        assert!(!fvgs[0].mitigated);
    }

    #[test]
    fn bearish_fvg() {
        let candles = vec![
            make_candle("1.2", "1.3", "1.1", "1.2"),
            make_candle("1.0", "1.1", "0.9", "1.0"), // middle
            make_candle("0.9", "1.0", "0.8", "0.9"),
        ];
        let fvgs = detect_fvg(&candles);
        assert_eq!(fvgs.len(), 1);
        assert_eq!(fvgs[0].side, Side::Short);
        assert_eq!(fvgs[0].top.to_string(), "1.1");
        assert_eq!(fvgs[0].bottom.to_string(), "1.0");
        assert!(!fvgs[0].mitigated);
    }

    #[test]
    fn fvg_mitigated_after_touch() {
        let candles = vec![
            make_candle("1.0", "1.0", "0.9", "1.0"),
            make_candle("1.1", "1.2", "1.0", "1.1"),
            make_candle("1.1", "1.3", "1.1", "1.2"),
            make_candle("1.2", "1.2", "1.05", "1.2"), // low=1.05 <= fvg.top=1.1 → mitigated
        ];
        let fvgs = detect_fvg(&candles);
        assert_eq!(fvgs.len(), 1);
        assert!(fvgs[0].mitigated);
    }

    #[test]
    fn bullish_ob() {
        let candles = vec![
            make_candle("1.2", "1.3", "1.1", "1.2"),
            make_candle("1.1", "1.2", "1.0", "1.1"),
            make_candle("1.1", "1.2", "0.9", "1.0"), // bearish (c < o)
            make_candle("1.0", "1.1", "0.8", "0.9"),
            make_candle("1.0", "1.2", "0.9", "1.1"),
        ];
        let swings = vec![make_swing(3, "0.8", SwingKind::Low)];
        let obs = detect_ob(&candles, &swings);
        assert!(obs.iter().any(|o| o.side == Side::Long));
    }

    #[test]
    fn bearish_ob() {
        let candles = vec![
            make_candle("0.9", "1.0", "0.8", "0.9"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.2", "1.0", "1.1"), // bullish (c > o)
            make_candle("1.1", "1.4", "1.1", "1.3"),
            make_candle("1.3", "1.3", "1.1", "1.2"),
        ];
        let swings = vec![make_swing(3, "1.4", SwingKind::High)];
        let obs = detect_ob(&candles, &swings);
        assert!(obs.iter().any(|o| o.side == Side::Short));
    }

    #[test]
    fn bos_bullish() {
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.1", "1.3", "1.0", "1.2"),
            make_candle("1.2", "1.5", "1.2", "1.4"), // close=1.4 > swing_high=1.3
        ];
        let swings = vec![make_swing(1, "1.3", SwingKind::High)];
        let structure = detect_structure(&candles, &swings);
        assert!(structure.iter().any(|s| s.side == Side::Long));
    }

    #[test]
    fn choch_bearish() {
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.1", "1.3", "1.0", "1.2"),
            make_candle("1.2", "1.5", "1.2", "1.4"), // BOS Long (close=1.4 > swing_high=1.3)
            make_candle("1.4", "1.4", "0.7", "0.8"), // CHoCH Short (close=0.8 < swing_low=0.9)
        ];
        let swings = vec![
            make_swing(0, "0.9", SwingKind::Low),
            make_swing(1, "1.3", SwingKind::High),
        ];
        let structure = detect_structure(&candles, &swings);
        assert!(structure
            .iter()
            .any(|s| s.kind == StructureEvent::Choch && s.side == Side::Short));
    }

    #[test]
    fn sweep_above_high() {
        let candles = vec![make_candle("1.2", "1.4", "1.1", "1.2")];
        let swings = vec![make_swing(0, "1.3", SwingKind::High)];
        let sweeps = detect_sweeps(&candles, &swings);
        assert!(sweeps.iter().any(|s| s.side == Side::Short));
    }

    #[test]
    fn sweep_below_low() {
        let candles = vec![make_candle("1.0", "1.1", "0.8", "1.0")];
        let swings = vec![make_swing(0, "0.9", SwingKind::Low)];
        let sweeps = detect_sweeps(&candles, &swings);
        assert!(sweeps.iter().any(|s| s.side == Side::Long));
    }

    #[test]
    fn pd_discount() {
        let candles = vec![make_candle("1.0", "1.1", "0.9", "1.1")];
        let swings = vec![
            make_swing(0, "1.0", SwingKind::Low),
            make_swing(0, "1.4", SwingKind::High),
        ];
        let pd = compute_pd(&candles, &swings).unwrap();
        assert_eq!(pd.current_zone, PdZone::Discount);
    }

    #[test]
    fn pd_premium() {
        let candles = vec![make_candle("1.2", "1.4", "1.2", "1.3")];
        let swings = vec![
            make_swing(0, "1.0", SwingKind::Low),
            make_swing(0, "1.4", SwingKind::High),
        ];
        let pd = compute_pd(&candles, &swings).unwrap();
        assert_eq!(pd.current_zone, PdZone::Premium);
    }

    #[test]
    fn ote_long() {
        let swings = vec![
            make_swing(0, "1.0", SwingKind::Low),
            make_swing(1, "2.0", SwingKind::High),
        ];
        let ote = compute_ote(&swings, Side::Long).unwrap();
        assert_eq!(ote.side, Side::Long);
        assert_eq!(ote.top.to_string(), "1.382");
        assert_eq!(ote.bottom.to_string(), "1.214");
    }

    #[test]
    fn ote_short() {
        let swings = vec![
            make_swing(0, "1.0", SwingKind::Low),
            make_swing(1, "2.0", SwingKind::High),
        ];
        let ote = compute_ote(&swings, Side::Short).unwrap();
        assert_eq!(ote.side, Side::Short);
        assert_eq!(ote.top.to_string(), "1.786");
        assert_eq!(ote.bottom.to_string(), "1.618");
    }
}
