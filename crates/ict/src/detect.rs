use domain::{Candle, Side};

use crate::swing::{SwingKind, SwingPoint};
use crate::types::{BosChoch, Fvg, LiquiditySweep, OrderBlock, Ote, PdArray, PdZone, StructureEvent};

pub(crate) fn detect_fvg(candles: &[Candle]) -> Vec<Fvg> {
    let mut result = Vec::new();
    for i in 2..candles.len() {
        // Bullish FVG: gap up — candle[i-2].high < candle[i].low
        if candles[i - 2].high < candles[i].low {
            let top = candles[i].low;
            let bottom = candles[i - 2].high;
            let formed_at = candles[i - 1].time;
            // Mitigated when price CLOSES into the gap (wick-only touches don't consume the zone).
            let mitigated = candles[i + 1..].iter().any(|c| c.close <= top);
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
            let mitigated = candles[i + 1..].iter().any(|c| c.close >= bottom);
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

pub(crate) fn detect_ob(candles: &[Candle], swings: &[SwingPoint]) -> Vec<OrderBlock> {
    let mut result = Vec::new();
    for swing in swings {
        match swing.kind {
            // SwingLow → bullish OB: last bearish candle before swing index
            SwingKind::Low => {
                if let Some(ob_candle) = candles[..swing.index]
                    .iter()
                    .rev()
                    .find(|c| c.close < c.open)
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
                if let Some(ob_candle) = candles[..swing.index]
                    .iter()
                    .rev()
                    .find(|c| c.close > c.open)
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

pub(crate) fn detect_structure(candles: &[Candle], swings: &[SwingPoint]) -> Vec<BosChoch> {
    let mut result = Vec::new();
    let mut last_structure_side: Option<Side> = None;

    // Process swings in candle-index order so we never reference a future swing.
    let mut sorted: Vec<&SwingPoint> = swings.iter().collect();
    sorted.sort_by_key(|s| s.index);
    let mut ptr = 0usize;

    // Most-recent unbroken swing high/low formed strictly before the current candle.
    let mut cur_high: Option<&SwingPoint> = None;
    let mut cur_low: Option<&SwingPoint> = None;

    for (candle_idx, candle) in candles.iter().enumerate() {
        // Absorb swings that formed on previous candles only.
        while ptr < sorted.len() && sorted[ptr].index < candle_idx {
            match sorted[ptr].kind {
                SwingKind::High => cur_high = Some(sorted[ptr]),
                SwingKind::Low => cur_low = Some(sorted[ptr]),
            }
            ptr += 1;
        }

        // Bullish BOS / CHoCH
        if let Some(sh) = cur_high
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
            cur_high = None; // level consumed; next swing high becomes the new reference
        }

        // Bearish BOS / CHoCH
        if let Some(sl) = cur_low
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
            cur_low = None; // level consumed
        }
    }
    result
}

pub(crate) fn detect_sweeps(candles: &[Candle], swings: &[SwingPoint]) -> Vec<LiquiditySweep> {
    let mut result = Vec::new();
    for (candle_idx, candle) in candles.iter().enumerate() {
        for swing in swings {
            // Swing must be formed strictly before this candle (no lookahead).
            if swing.index >= candle_idx {
                continue;
            }
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

pub(crate) fn compute_pd(candles: &[Candle], swings: &[SwingPoint], bias: Option<Side>) -> Option<PdArray> {
    let (range_high, range_low) = match bias {
        Some(Side::Long) => {
            // Use the most-recent swing high (by candle index) and the most-recent swing low
            // that preceded it — this is the impulse range of the last bullish move.
            let anchor_high = swings.iter().filter(|s| s.kind == SwingKind::High).max_by_key(|s| s.index)?;
            let preceding_low = swings.iter().filter(|s| s.kind == SwingKind::Low && s.index < anchor_high.index).max_by_key(|s| s.index)?;
            (anchor_high.price, preceding_low.price)
        }
        Some(Side::Short) => {
            // Most-recent swing low and the most-recent swing high that preceded it.
            let anchor_low = swings.iter().filter(|s| s.kind == SwingKind::Low).max_by_key(|s| s.index)?;
            let preceding_high = swings.iter().filter(|s| s.kind == SwingKind::High && s.index < anchor_low.index).max_by_key(|s| s.index)?;
            (preceding_high.price, anchor_low.price)
        }
        None => {
            let h = swings.iter().filter(|s| s.kind == SwingKind::High).map(|s| s.price).max()?;
            let l = swings.iter().filter(|s| s.kind == SwingKind::Low).map(|s| s.price).min()?;
            (h, l)
        }
    };
    if range_high <= range_low { return None; }
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

pub(crate) fn compute_ote(swings: &[SwingPoint], bias: Side) -> Option<Ote> {
    let fib618: rust_decimal::Decimal = "0.618".parse().unwrap();
    let fib786: rust_decimal::Decimal = "0.786".parse().unwrap();

    // Use the most-recent swing pair by candle index, not global price extremes.
    // For Long: most-recent swing HIGH anchors the top; the most-recent swing LOW
    // strictly before it is the impulse base.
    // For Short: mirror — most-recent LOW anchors the bottom.
    let (swing_high, swing_low) = match bias {
        Side::Long => {
            let anchor = swings.iter().filter(|s| s.kind == SwingKind::High).max_by_key(|s| s.index)?;
            let base   = swings.iter().filter(|s| s.kind == SwingKind::Low && s.index < anchor.index).max_by_key(|s| s.index)?;
            (anchor.price, base.price)
        }
        Side::Short => {
            let anchor = swings.iter().filter(|s| s.kind == SwingKind::Low).max_by_key(|s| s.index)?;
            let base   = swings.iter().filter(|s| s.kind == SwingKind::High && s.index < anchor.index).max_by_key(|s| s.index)?;
            (base.price, anchor.price)
        }
    };

    let range = swing_high - swing_low;
    if range <= rust_decimal::Decimal::ZERO { return None; }

    let (top, bottom) = match bias {
        Side::Long  => (swing_high - fib618 * range, swing_high - fib786 * range),
        Side::Short => (swing_low  + fib786 * range, swing_low  + fib618 * range),
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
            make_candle("1.2", "1.2", "1.05", "1.0"), // close=1.0 <= fvg.top=1.1 → mitigated
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
        // Swing at index 0; sweep candle at index 1 (strictly after).
        let candles = vec![
            make_candle("1.3", "1.3", "1.2", "1.3"), // [0] swing high context
            make_candle("1.2", "1.4", "1.1", "1.2"), // [1] wicks above 1.3, closes below → bearish sweep
        ];
        let swings = vec![make_swing(0, "1.3", SwingKind::High)];
        let sweeps = detect_sweeps(&candles, &swings);
        assert!(sweeps.iter().any(|s| s.side == Side::Short));
    }

    #[test]
    fn sweep_below_low() {
        // Swing at index 0; sweep candle at index 1 (strictly after).
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"), // [0] swing low context
            make_candle("1.0", "1.1", "0.8", "1.0"), // [1] wicks below 0.9, closes above → bullish sweep
        ];
        let swings = vec![make_swing(0, "0.9", SwingKind::Low)];
        let sweeps = detect_sweeps(&candles, &swings);
        assert!(sweeps.iter().any(|s| s.side == Side::Long));
    }

    #[test]
    fn pd_discount() {
        // Long context: anchor_high at index 1, preceding_low at index 0.
        // equilibrium = (1.4+1.0)/2 = 1.2; close=1.1 < 1.2 → Discount.
        let candles = vec![make_candle("1.0", "1.1", "0.9", "1.1")];
        let swings = vec![
            make_swing(0, "1.0", SwingKind::Low),
            make_swing(1, "1.4", SwingKind::High),
        ];
        let pd = compute_pd(&candles, &swings, Some(Side::Long)).unwrap();
        assert_eq!(pd.current_zone, PdZone::Discount);
    }

    #[test]
    fn pd_premium() {
        // Short context: anchor_low at index 1, preceding_high at index 0.
        // equilibrium = (1.4+1.0)/2 = 1.2; close=1.3 > 1.2 → Premium.
        let candles = vec![make_candle("1.2", "1.4", "1.2", "1.3")];
        let swings = vec![
            make_swing(0, "1.4", SwingKind::High),
            make_swing(1, "1.0", SwingKind::Low),
        ];
        let pd = compute_pd(&candles, &swings, Some(Side::Short)).unwrap();
        assert_eq!(pd.current_zone, PdZone::Premium);
    }

    #[test]
    fn ote_long() {
        // Long: anchor_high at index 1, preceding_low at index 0.
        let swings = vec![
            make_swing(0, "1.0", SwingKind::Low),
            make_swing(1, "2.0", SwingKind::High),
        ];
        let ote = compute_ote(&swings, Side::Long).unwrap();
        assert_eq!(ote.side, Side::Long);
        assert_eq!(ote.top,    "1.382".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(ote.bottom, "1.214".parse::<rust_decimal::Decimal>().unwrap());
    }

    #[test]
    fn ote_short() {
        // Short: anchor_low at index 1, preceding_high at index 0.
        let swings = vec![
            make_swing(0, "2.0", SwingKind::High),
            make_swing(1, "1.0", SwingKind::Low),
        ];
        let ote = compute_ote(&swings, Side::Short).unwrap();
        assert_eq!(ote.side, Side::Short);
        assert_eq!(ote.top,    "1.786".parse::<rust_decimal::Decimal>().unwrap());
        assert_eq!(ote.bottom, "1.618".parse::<rust_decimal::Decimal>().unwrap());
    }
}
