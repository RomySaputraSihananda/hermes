use chrono::{DateTime, Utc};
use domain::Side;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct Fvg {
    pub top: Decimal,
    pub bottom: Decimal,
    pub formed_at: DateTime<Utc>,
    pub side: Side,
    pub mitigated: bool,
}

#[derive(Debug, Clone)]
pub struct OrderBlock {
    pub top: Decimal,
    pub bottom: Decimal,
    pub formed_at: DateTime<Utc>,
    pub side: Side,
    pub mitigated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureEvent {
    Bos,
    Choch,
}

#[derive(Debug, Clone)]
pub struct BosChoch {
    pub kind: StructureEvent,
    pub level: Decimal,
    pub broken_at: DateTime<Utc>,
    pub side: Side,
}

#[derive(Debug, Clone)]
pub struct LiquiditySweep {
    pub level: Decimal,
    pub swept_at: DateTime<Utc>,
    pub side: Side,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdZone {
    Premium,
    Discount,
    Equilibrium,
}

#[derive(Debug, Clone)]
pub struct PdArray {
    pub range_high: Decimal,
    pub range_low: Decimal,
    pub equilibrium: Decimal,
    pub current_zone: PdZone,
}

#[derive(Debug, Clone)]
pub struct Ote {
    pub top: Decimal,
    pub bottom: Decimal,
    pub side: Side,
}

#[derive(Debug, Clone, Default)]
pub struct ConfluenceFlags {
    pub has_bos_choch: bool,
    pub in_pd_zone: bool,
    pub ob_in_ote: bool,
    pub fvg_in_ote: bool,
    pub sweep_present: bool,
}

#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub side: Side,
    pub entry: Decimal,
    pub sl: Decimal,
    pub tp: Decimal,
    pub confluence: ConfluenceFlags,
}
