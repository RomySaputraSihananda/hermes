use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub risk_pct: Decimal,      // 0.01 = 1% of balance
    pub value_per_lot: Decimal, // account currency per 1.0 lot per 1.0 price unit
    pub min_volume: Decimal,    // broker minimum lot, e.g. 0.01
    pub max_volume: Decimal,    // broker maximum lot, e.g. 10.0
    pub volume_step: Decimal,   // broker lot step, e.g. 0.01
}

#[derive(Debug, Clone)]
pub struct RiskDecision {
    pub volume: Decimal,        // calculated lots (0 if !approved)
    pub approved: bool,
    pub reason: Option<String>, // rejection reason if !approved
}
