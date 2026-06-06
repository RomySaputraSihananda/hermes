mod types;

pub use types::{RiskConfig, RiskDecision};

pub fn evaluate(
    _account: &domain::AccountInfo,
    _signal: &ict::TradeSignal,
    _config: &RiskConfig,
) -> RiskDecision {
    todo!()
}
