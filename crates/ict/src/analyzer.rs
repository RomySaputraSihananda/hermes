use domain::{Candle, Side};
use rust_decimal::Decimal;

use crate::detect::{compute_ote, compute_pd, detect_fvg, detect_ob, detect_structure, detect_sweeps};
use crate::swing::{SwingKind, SwingPoint, detect_swings};
use crate::types::{BosChoch, ConfluenceFlags, Fvg, LiquiditySweep, OrderBlock, Ote, PdArray, PdZone, TradeSignal};

pub struct IctAnalyzer<'a> {
    candles:         &'a [Candle],
    min_sl_distance: Decimal,
}

pub struct IctAnalysis {
    pub fvgs: Vec<Fvg>,
    pub order_blocks: Vec<OrderBlock>,
    pub structure: Vec<BosChoch>,
    pub sweeps: Vec<LiquiditySweep>,
    pub pd_array: Option<PdArray>,
    pub ote: Option<Ote>,
    pub signal: Option<TradeSignal>,
}

impl<'a> IctAnalyzer<'a> {
    pub fn new(candles: &'a [Candle], min_sl_distance: Decimal) -> Self {
        Self { candles, min_sl_distance }
    }

    pub fn analyze(&self) -> IctAnalysis {
        if self.candles.len() < 5 {
            return IctAnalysis {
                fvgs: vec![],
                order_blocks: vec![],
                structure: vec![],
                sweeps: vec![],
                pd_array: None,
                ote: None,
                signal: None,
            };
        }

        let swings       = detect_swings(self.candles, 2);
        let fvgs         = detect_fvg(self.candles);
        let order_blocks = detect_ob(self.candles, &swings);
        let structure    = detect_structure(self.candles, &swings);
        let sweeps       = detect_sweeps(self.candles, &swings);
        let pd_array     = compute_pd(self.candles, &swings);
        let bias         = structure.last().map(|s| s.side);
        let ote          = bias.and_then(|b| compute_ote(&swings, b));
        let signal       = check_confluence(
            self.candles,
            &fvgs,
            &order_blocks,
            &structure,
            &sweeps,
            &pd_array,
            &ote,
            &swings,
        );

        let signal = signal.filter(|s| {
            self.min_sl_distance == Decimal::ZERO
                || (s.entry - s.sl).abs() >= self.min_sl_distance
        });

        IctAnalysis {
            fvgs,
            order_blocks,
            structure,
            sweeps,
            pd_array,
            ote,
            signal,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_confluence(
    candles: &[Candle],
    fvgs: &[Fvg],
    order_blocks: &[OrderBlock],
    structure: &[BosChoch],
    sweeps: &[LiquiditySweep],
    pd_array: &Option<PdArray>,
    ote: &Option<Ote>,
    swings: &[SwingPoint],
) -> Option<TradeSignal> {
    // Condition 1: structure non-empty → take last BosChoch → bias
    let last_structure = structure.last()?;
    let bias = last_structure.side;

    // Condition 2: PD zone must match bias
    let pd = pd_array.as_ref()?;
    let zone_ok = match bias {
        Side::Long => pd.current_zone == PdZone::Discount,
        Side::Short => pd.current_zone == PdZone::Premium,
    };
    if !zone_ok {
        return None;
    }

    // Condition 3: Last close must be inside OTE zone [ote.bottom, ote.top]
    let ote = ote.as_ref()?;
    let last_close = candles.last()?.close;
    if last_close < ote.bottom || last_close > ote.top {
        return None;
    }

    // Condition 4: Find overlapping unmitigated structure (OB preferred over FVG)
    let overlapping_ob = order_blocks
        .iter()
        .rfind(|o| !o.mitigated && o.side == bias && o.top > ote.bottom && o.bottom < ote.top);

    let overlapping_fvg = fvgs
        .iter()
        .rfind(|f| !f.mitigated && f.side == bias && f.top > ote.bottom && f.bottom < ote.top);

    let (entry_top, entry_bottom) = if let Some(ob) = overlapping_ob {
        (ob.top, ob.bottom)
    } else if let Some(fvg) = overlapping_fvg {
        (fvg.top, fvg.bottom)
    } else {
        return None;
    };

    let two = Decimal::from(2u32);
    let entry = (entry_top + entry_bottom) / two;

    let sl = match bias {
        Side::Long => swings
            .iter()
            .filter(|s| s.kind == SwingKind::Low && s.price < entry)
            .map(|s| s.price)
            .max()?,
        Side::Short => swings
            .iter()
            .filter(|s| s.kind == SwingKind::High && s.price > entry)
            .map(|s| s.price)
            .min()?,
    };

    // The filter predicates (price < entry for Long, price > entry for Short) already
    // guarantee this invariant by construction; the assert documents it for readers.
    debug_assert!(
        match bias { Side::Long => sl < entry, Side::Short => sl > entry },
        "SL must be on the protective side of entry"
    );

    let tp = match bias {
        Side::Long => swings
            .iter()
            .filter(|s| s.kind == SwingKind::High)
            .map(|s| s.price)
            .max()?,
        Side::Short => swings
            .iter()
            .filter(|s| s.kind == SwingKind::Low)
            .map(|s| s.price)
            .min()?,
    };

    // sweep_present: any sweep in last 5 candles (by index using cutoff_time)
    let last_5_start = candles.len().saturating_sub(5);
    let cutoff_time = candles[last_5_start].time;
    let sweep_present = sweeps.iter().any(|s| s.swept_at >= cutoff_time);

    let confluence = ConfluenceFlags {
        has_bos_choch: true,
        in_pd_zone: true,
        ob_in_ote: overlapping_ob.is_some(),
        fvg_in_ote: overlapping_fvg.is_some(),
        sweep_present,
    };

    Some(TradeSignal {
        side: bias,
        entry,
        sl,
        tp,
        confluence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn full_confluence_candles() -> Vec<Candle> {
        vec![
            make_candle("1.5", "1.6", "1.4", "1.4"),    // [0] bearish
            make_candle("1.4", "1.45", "1.25", "1.3"),   // [1] bearish → OB (top=1.45, bottom=1.25)
            make_candle("1.3", "1.30", "1.0", "1.25"),   // [2] swing_low=1.0
            make_candle("1.25", "1.35", "1.20", "1.3"),  // [3]
            make_candle("1.3", "1.40", "1.25", "1.35"),  // [4]
            make_candle("1.35", "1.50", "1.35", "1.45"), // [5]
            make_candle("1.45", "1.80", "1.45", "1.70"), // [6]
            make_candle("1.7", "2.00", "1.70", "1.90"),  // [7] swing_high=2.0
            make_candle("1.9", "1.95", "1.80", "1.90"),  // [8]
            make_candle("1.9", "1.95", "1.90", "2.10"),  // [9] BOS Long (close=2.1 > 2.0)
            make_candle("2.1", "2.10", "0.95", "1.30"),  // [10] sweep + Discount + OTE
        ]
    }

    #[test]
    fn full_confluence_generates_signal() {
        let candles = full_confluence_candles();
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        assert!(analysis.signal.is_some(), "expected a TradeSignal but got None");
        let sig = analysis.signal.unwrap();
        assert_eq!(sig.side, Side::Long);
        assert!(sig.entry > Decimal::ZERO);
        assert!(sig.sl < sig.entry);
        assert!(sig.tp > sig.entry);
        assert_eq!(sig.entry, "1.35".parse::<rust_decimal::Decimal>().unwrap(), "entry should be OB midpoint");
        assert_eq!(sig.sl, "1.0".parse::<rust_decimal::Decimal>().unwrap(), "sl should be nearest swing low below entry");
        assert_eq!(sig.tp, "2.0".parse::<rust_decimal::Decimal>().unwrap(), "tp should be max swing high");
    }

    #[test]
    fn no_bos_returns_no_signal() {
        // 5 flat candles — no swing points form, no structure breaks
        let candles = vec![
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.2", "0.9", "1.0"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
            make_candle("1.0", "1.1", "0.9", "1.0"),
        ];
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        assert!(analysis.signal.is_none());
    }

    #[test]
    fn wrong_pd_zone_returns_no_signal() {
        // Replace last candle with close=1.8 → Premium (> equilibrium=1.5), bias=Long needs Discount
        let mut candles = full_confluence_candles();
        *candles.last_mut().unwrap() = make_candle("1.9", "2.0", "1.7", "1.8");
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        assert!(analysis.signal.is_none());
    }

    #[test]
    fn no_structure_in_ote_returns_no_signal() {
        // Replace last candle with close=1.1 → Discount but below OTE bottom (1.214)
        let mut candles = full_confluence_candles();
        *candles.last_mut().unwrap() = make_candle("1.3", "1.3", "1.0", "1.1");
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        assert!(analysis.signal.is_none());
    }

    #[test]
    fn no_ob_or_fvg_in_ote_returns_no_signal() {
        // Mitigate the OB by inserting a candle that closes below ob.bottom=1.25
        // Conditions 1 (BOS), 2 (Discount), 3 (OTE) still pass — but no OB/FVG overlaps OTE
        let mut candles = full_confluence_candles();
        let last = candles.pop().unwrap();
        // close=1.2 < ob.bottom=1.25 → mitigates the bullish OB
        candles.push(make_candle("1.3", "1.3", "1.1", "1.2"));
        candles.push(last);
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        assert!(analysis.signal.is_none(), "expected None when no OB/FVG overlaps OTE");
    }

    #[test]
    fn sl_too_close_filtered_out() {
        // entry=1.35, sl=1.0 → distance=0.35
        // min_sl_distance=0.5 > 0.35 → signal dibuang
        let candles = full_confluence_candles();
        let analysis = IctAnalyzer::new(&candles, "0.5".parse().unwrap()).analyze();
        assert!(analysis.signal.is_none());
    }

    #[test]
    fn sl_wide_enough_passes() {
        // entry=1.35, sl=1.0 → distance=0.35
        // min_sl_distance=0.05 < 0.35 → signal lolos
        let candles = full_confluence_candles();
        let analysis = IctAnalyzer::new(&candles, "0.05".parse().unwrap()).analyze();
        assert!(analysis.signal.is_some());
        let sig = analysis.signal.unwrap();
        assert_eq!(sig.side, Side::Long);
        assert_eq!(sig.entry, "1.35".parse::<Decimal>().unwrap());
        assert_eq!(sig.sl, "1.0".parse::<Decimal>().unwrap());
    }

    #[test]
    fn sl_is_on_correct_side_of_entry() {
        // Long case only — Short fixture not yet available
        let candles = full_confluence_candles();
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        let sig = analysis.signal.expect("full_confluence_candles should produce a signal");
        match sig.side {
            Side::Long  => assert!(sig.sl < sig.entry, "Long SL must be below entry"),
            Side::Short => assert!(sig.sl > sig.entry, "Short SL must be above entry"),
        }
    }
}
