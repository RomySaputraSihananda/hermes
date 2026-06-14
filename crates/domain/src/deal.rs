use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Deal {
    pub ticket:     u64,
    pub order:      u64,
    pub time:       String,
    #[serde(rename = "type")]
    pub kind:       u32,   // 0=buy, 1=sell, 2=balance
    pub entry:      u32,   // 0=in, 1=out
    pub symbol:     String,
    pub volume:     Decimal,
    pub price:      Decimal,
    pub commission: Decimal,
    pub swap:       Decimal,
    pub profit:     Decimal,
    pub comment:    String,
    pub magic:      u64,
}
