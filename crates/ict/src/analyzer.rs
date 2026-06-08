use domain::{Candle, Side};
use rust_decimal::Decimal;

use crate::detect::{compute_ote, compute_pd, detect_fvg, detect_ob, detect_structure, detect_sweeps};
use crate::swing::{SwingKind, SwingPoint, detect_swings};
use crate::types::{BosChoch, ConfluenceFlags, Fvg, LiquiditySweep, OrderBlock, Ote, PdArray, PdZone, TradeSignal};

pub struct IctAnalyzer<'a> {
    candles:         &'a [Candle],
    min_sl_distance: Decimal,
    swing_period:    usize,
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
        Self { candles, min_sl_distance, swing_period: 2 }
    }

    pub fn with_swing_period(mut self, n: usize) -> Self {
        self.swing_period = n.max(1);
        self
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

        let swings       = detect_swings(self.candles, self.swing_period);
        let fvgs         = detect_fvg(self.candles);
        let order_blocks = detect_ob(self.candles, &swings);
        let structure    = detect_structure(self.candles, &swings);
        let sweeps       = detect_sweeps(self.candles, &swings);
        let bias         = structure.last().map(|s| s.side);
        let pd_array     = compute_pd(self.candles, &swings, bias);
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

    // Condition 4: At least one of OB or FVG must overlap the OTE zone (OB preferred).
    // Entry is clamped to the intersection of the chosen zone and OTE so the limit
    // order is always placed inside the confirmed retracement area.
    let overlapping_ob = order_blocks
        .iter()
        .rfind(|o| !o.mitigated && o.side == bias && o.top > ote.bottom && o.bottom < ote.top);

    let overlapping_fvg = fvgs
        .iter()
        .rfind(|f| !f.mitigated && f.side == bias && f.top > ote.bottom && f.bottom < ote.top);

    let (zone_bottom, zone_top) = match (overlapping_ob, overlapping_fvg) {
        (Some(ob), _)     => (ob.bottom, ob.top),   // OB preferred
        (None, Some(fvg)) => (fvg.bottom, fvg.top),
        (None, None)      => return None,
    };

    let two = Decimal::from(2u32);
    // Clamp zone to OTE so the entry midpoint is always inside the retracement window.
    let inter_bottom = zone_bottom.max(ote.bottom);
    let inter_top    = zone_top.min(ote.top);
    let entry        = (inter_top + inter_bottom) / two;

    let sl = match bias {
        Side::Long  => pd.range_low,
        Side::Short => pd.range_high,
    };
    let valid_sl = match bias {
        Side::Long  => sl < entry,
        Side::Short => sl > entry,
    };
    if !valid_sl {
        return None;
    }

    // TP = the furthest prior swing extreme in the window (global high for Long, global low
    // for Short). This is the "draw on liquidity" — the old high/low that price has not yet
    // swept. Targeting anchor_high alone (R:R ≈2.4:1) was tested and produced PF < 1 because
    // winning trades consistently run past anchor_high to the global extreme.
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

    // Condition 5: liquidity sweep in the CORRECT DIRECTION must have occurred in the last 5
    // candles. Long needs a bullish sweep (stops below a swing low taken out), Short needs a
    // bearish sweep (stops above a swing high taken out).
    let last_5_start = candles.len().saturating_sub(5);
    let cutoff_time  = candles[last_5_start].time;
    let sweep_present = sweeps
        .iter()
        .any(|s| s.swept_at >= cutoff_time && s.side == bias);
    if !sweep_present {
        return None;
    }

    let confluence = ConfluenceFlags {
        has_bos_choch: true,
        in_pd_zone:    true,
        ob_in_ote:     overlapping_ob.is_some(),
        fvg_in_ote:    overlapping_fvg.is_some(),
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
        // OTE for Long = [1.214, 1.382] (range low=1.0, high=2.0).
        // [2].high=1.30 < [4].low=1.32 → bullish FVG: bottom=1.30, top=1.32 (inside OTE).
        // [10] close=1.35 > FVG.top=1.32 → FVG not mitigated (close-based mitigation).
        vec![
            make_candle("1.5", "1.6", "1.4", "1.4"),    // [0] bearish
            make_candle("1.4", "1.45", "1.25", "1.3"),   // [1] bearish → OB (top=1.45, bottom=1.25)
            make_candle("1.3", "1.30", "1.0", "1.25"),   // [2] swing_low=1.0
            make_candle("1.25", "1.35", "1.20", "1.3"),  // [3]
            make_candle("1.3", "1.40", "1.32", "1.35"),  // [4] low=1.32 > [2].high=1.30 → FVG
            make_candle("1.35", "1.50", "1.35", "1.45"), // [5]
            make_candle("1.45", "1.80", "1.45", "1.70"), // [6]
            make_candle("1.7", "2.00", "1.70", "1.90"),  // [7] swing_high=2.0
            make_candle("1.9", "1.95", "1.80", "1.90"),  // [8]
            make_candle("1.9", "1.95", "1.90", "2.10"),  // [9] BOS Long (close=2.1 > 2.0)
            make_candle("2.1", "2.10", "0.95", "1.35"),  // [10] sweep + Discount + OTE (close=1.35)
        ]
    }

    fn two_swing_lows_candles() -> Vec<Candle> {
        // Two swing lows below entry=1.35 with only ONE OB overlapping OTE:
        //   swing low A at 1.00 → pd.range_low  (OB before it: candle[1] top=1.45, bottom=1.25 → overlaps OTE [1.214,1.382])
        //   swing low B at 1.22 → nearest below entry (OB before it: high=1.90, bottom=1.70 → does NOT overlap OTE)
        //
        // detect_swings(period=2) on 15 candles, n=2, loop i=2..12:
        //   [2].low=1.00: left lows [0]=1.40,[1]=1.25 both > 1.00 ✓; right lows [3]=1.20,[4]=1.24 both > 1.00 ✓
        //   [9].high=2.00: left highs [7]=1.50,[8]=1.80 both < 2.00 ✓; right highs [10]=1.95,[11]=1.95 both < 2.00 ✓
        //   [12].low=1.22: left lows [10]=1.80,[11]=1.90 both > 1.22 ✓; right lows [13]=1.28,[14]=1.26 both > 1.22 ✓
        //
        // ICT confluence: BOS Long at some candle close > 2.00,
        //   OTE=[1.214,1.382] (range=[1.00,2.00]), last_close=1.35 ∈ OTE,
        //   OB from swing_low_A overlaps OTE: entry=1.35 (midpoint of 1.45/1.25)
        //   OB from swing_low_B: top=1.90, bottom=1.70 → bottom=1.70 > ote.top=1.382 → no overlap
        //   FVG [13/15]: bottom=1.32, top=1.34 → overlaps OTE, unmitigated (close[17]=1.35 > 1.34)
        vec![
            make_candle("1.5",  "1.60", "1.40", "1.40"),  // [0]  low=1.40
            make_candle("1.40", "1.45", "1.25", "1.30"),  // [1]  bearish OB-A: top=1.45, bottom=1.25 → overlaps OTE
            make_candle("1.30", "1.32", "1.00", "1.25"),  // [2]  swing_low=1.00 (pd.range_low)
            make_candle("1.25", "1.30", "1.20", "1.28"),  // [3]  low=1.20
            make_candle("1.28", "1.35", "1.24", "1.32"),  // [4]  low=1.24
            make_candle("1.32", "1.40", "1.28", "1.38"),  // [5]  low=1.28
            make_candle("1.38", "1.50", "1.35", "1.45"),  // [6]  low=1.35
            make_candle("1.45", "1.50", "1.42", "1.48"),  // [7]  low=1.42 (high < 2.00)
            make_candle("1.48", "1.80", "1.50", "1.75"),  // [8]  high=1.80
            make_candle("1.75", "2.00", "1.75", "1.90"),  // [9]  swing_high=2.00
            make_candle("1.90", "1.95", "1.80", "1.85"),  // [10] bearish OB-B: top=1.95,bottom=1.80 (>ote.top=1.382)
            make_candle("1.85", "1.95", "1.90", "1.92"),  // [11] low=1.90
            make_candle("1.92", "1.95", "1.22", "1.25"),  // [12] swing_low=1.22 (nearest below 1.35)
            make_candle("1.25", "1.32", "1.28", "1.32"),  // [13] high=1.32 (min allowed: max(1.25,1.32))
            make_candle("1.32", "1.40", "1.26", "1.35"),  // [14] middle of FVG
            make_candle("1.35", "1.45", "1.34", "1.42"),  // [15] low=1.34 > [13].high=1.32 → FVG bottom=1.32 top=1.34
            make_candle("1.42", "2.10", "1.40", "2.05"),  // [16] BOS Long: close=2.05 > swing_high=2.00
            make_candle("2.05", "2.10", "0.95", "1.35"),  // [17] sweep + Discount + OTE, close=1.35 > FVG.top=1.34
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
        // entry = intersection(OB, OTE) midpoint: inter=[1.25, 1.382], mid=1.316
        // sl    = pd.range_low = 1.0
        // tp    = max swing high in window = 2.0 (prior liquidity pool)
        assert_eq!(sig.entry, "1.316".parse::<rust_decimal::Decimal>().unwrap(), "entry should be OB∩OTE midpoint");
        assert_eq!(sig.sl,    "1.0".parse::<rust_decimal::Decimal>().unwrap(),   "sl should be pd.range_low");
        assert_eq!(sig.tp,    "2.0".parse::<rust_decimal::Decimal>().unwrap(),   "tp should be max swing high");
        assert!(sig.confluence.ob_in_ote,  "OB must be in OTE");
        assert!(sig.confluence.fvg_in_ote, "FVG must be in OTE");
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
    fn ob_without_fvg_still_generates_signal() {
        // Mitigate FVG (close=1.31 ≤ FVG.top=1.32) but keep OB intact.
        // With OR logic, OB alone is sufficient for a signal.
        let mut candles = full_confluence_candles();
        let last = candles.pop().unwrap();
        candles.push(make_candle("1.35", "1.40", "1.28", "1.31")); // mitigates FVG
        candles.push(last);
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        assert!(analysis.signal.is_some(), "OB alone (OR logic) should produce a signal");
        let sig = analysis.signal.unwrap();
        assert!(sig.confluence.ob_in_ote,   "OB must be flagged");
        assert!(!sig.confluence.fvg_in_ote, "FVG must NOT be flagged (mitigated)");
    }

    #[test]
    fn sl_too_close_filtered_out() {
        // entry=1.316, sl=pd.range_low=1.0 → distance=0.316
        // min_sl_distance=0.5 > 0.316 → signal dibuang
        let candles = full_confluence_candles();
        let analysis = IctAnalyzer::new(&candles, "0.5".parse().unwrap()).analyze();
        assert!(analysis.signal.is_none());
    }

    #[test]
    fn sl_wide_enough_passes() {
        // entry=1.316, sl=pd.range_low=1.0 → distance=0.316
        // min_sl_distance=0.05 < 0.316 → signal lolos
        let candles = full_confluence_candles();
        let analysis = IctAnalyzer::new(&candles, "0.05".parse().unwrap()).analyze();
        assert!(analysis.signal.is_some());
        let sig = analysis.signal.unwrap();
        assert_eq!(sig.side, Side::Long);
        assert_eq!(sig.entry, "1.316".parse::<Decimal>().unwrap());
        assert_eq!(sig.sl,    "1.0".parse::<Decimal>().unwrap());
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

    #[test]
    fn sl_uses_pd_range_low_not_nearest_swing() {
        // two_swing_lows_candles has two swing lows below entry=1.35:
        //   1.00 (pd.range_low) and 1.22 (nearest swing low)
        // Phase 12: sl must be pd.range_low=1.00, not nearest=1.22
        let candles = two_swing_lows_candles();
        let analysis = IctAnalyzer::new(&candles, Decimal::ZERO).analyze();
        let sig = analysis.signal.expect("two_swing_lows_candles should produce a signal");
        assert_eq!(
            sig.sl,
            "1.0".parse::<Decimal>().unwrap(),
            "sl should be pd.range_low (1.0), not nearest swing low (1.22)"
        );
    }
}
