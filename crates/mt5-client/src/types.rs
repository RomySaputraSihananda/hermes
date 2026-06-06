use serde::{Deserialize, Serialize};

// ── public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub mt5_connected: bool,
    pub mt5_version: String,
    pub api_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeRequest {
    pub action: u32,
    pub symbol: String,
    pub volume: f64,
    #[serde(rename = "type")]
    pub order_type: u32,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magic: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderCheckResult {
    pub retcode: u32,
    pub balance: f64,
    pub equity: f64,
    pub profit: f64,
    pub margin: f64,
    pub margin_free: f64,
    pub margin_level: f64,
    pub comment: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradeResult {
    pub retcode: u32,
    pub order: u64,
    pub comment: String,
}

// ── internal types (crate-visible only) ───────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct DataVec<T> {
    pub(crate) data: Vec<T>,
}

#[derive(Deserialize)]
pub(crate) struct DataOne<T> {
    pub(crate) data: T,
}

#[derive(Deserialize)]
pub(crate) struct ApiErrorBody {
    pub(crate) detail: String,
}
