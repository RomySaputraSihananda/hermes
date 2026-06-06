mod types;

pub use types::{RiskConfig, RiskDecision};

pub fn evaluate(
    account: &domain::AccountInfo,
    signal: &ict::TradeSignal,
    config: &RiskConfig,
) -> RiskDecision {
    todo!()
}
