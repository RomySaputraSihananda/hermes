use domain::{AccountInfo, Candle, Position, Symbol, Tick, Timeframe};

use crate::error::Mt5Error;
use crate::types::{ApiErrorBody, DataOne, DataVec, HealthStatus, OrderCheckResult, TradeRequest, TradeResult};

pub struct Mt5Client {
    base_url: String,
    http: reqwest::Client,
}

impl Mt5Client {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), http: reqwest::Client::new() }
    }

    async fn fetch_text(&self, req: reqwest::RequestBuilder) -> Result<String, Mt5Error> {
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            let detail = serde_json::from_str::<ApiErrorBody>(&body)
                .map(|e| e.detail)
                .unwrap_or(body);
            return Err(Mt5Error::Api { status: code, detail });
        }
        Ok(resp.text().await?)
    }

    pub async fn health(&self) -> Result<HealthStatus, Mt5Error> {
        let url = format!("{}/health", self.base_url);
        let text = self.fetch_text(self.http.get(&url)).await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn account(&self) -> Result<AccountInfo, Mt5Error> {
        let url = format!("{}/account", self.base_url);
        let text = self.fetch_text(self.http.get(&url)).await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataVec<AccountInfo> = serde_json::from_str(&text)?;
        w.data.into_iter().next().ok_or(Mt5Error::Empty { endpoint: url })
    }

    pub async fn symbol(&self, name: &str) -> Result<Symbol, Mt5Error> {
        let url = format!("{}/symbols/{name}", self.base_url);
        let text = self.fetch_text(self.http.get(&url)).await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataVec<Symbol> = serde_json::from_str(&text)?;
        w.data.into_iter().next().ok_or(Mt5Error::Empty { endpoint: url })
    }

    pub async fn tick(&self, symbol: &str) -> Result<Tick, Mt5Error> {
        let url = format!("{}/symbols/{symbol}/tick", self.base_url);
        let text = self.fetch_text(self.http.get(&url)).await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataVec<Tick> = serde_json::from_str(&text)?;
        w.data.into_iter().next().ok_or(Mt5Error::Empty { endpoint: url })
    }

    pub async fn rates_from_pos(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start_pos: u32,
        count: u32,
    ) -> Result<Vec<Candle>, Mt5Error> {
        let url = format!("{}/rates/from-pos", self.base_url);
        let text = self
            .fetch_text(self.http.get(&url).query(&[
                ("symbol", symbol),
                ("timeframe", timeframe.as_api_str()),
                ("start_pos", &start_pos.to_string()),
                ("count", &count.to_string()),
            ]))
            .await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataVec<Candle> = serde_json::from_str(&text)?;
        Ok(w.data)
    }

    pub async fn positions(&self) -> Result<Vec<Position>, Mt5Error> {
        let url = format!("{}/positions", self.base_url);
        let text = self.fetch_text(self.http.get(&url)).await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataVec<Position> = serde_json::from_str(&text)?;
        Ok(w.data)
    }

    pub async fn order_check(&self, request: &TradeRequest) -> Result<OrderCheckResult, Mt5Error> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            request: &'a TradeRequest,
        }
        let url = format!("{}/order/check", self.base_url);
        let text = self
            .fetch_text(self.http.post(&url).json(&Body { request }))
            .await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataOne<OrderCheckResult> = serde_json::from_str(&text)?;
        Ok(w.data)
    }

    pub async fn place_order(
        &self,
        request: &TradeRequest,
    ) -> Result<TradeResult, Mt5Error> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            request: &'a TradeRequest,
        }
        let url = format!("{}/order/send", self.base_url);
        let text = self
            .fetch_text(self.http.post(&url).json(&Body { request }))
            .await?;
        tracing::debug!(endpoint = %url, "mt5 response ok");
        let w: DataOne<TradeResult> = serde_json::from_str(&text)?;
        Ok(w.data)
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{DataOne, DataVec, HealthStatus, OrderCheckResult, TradeRequest, TradeResult};
    use domain::{AccountInfo, Candle, Position, Symbol, Tick};

    #[test]
    fn parse_health() {
        let raw = r#"{"status":"healthy","mt5_connected":true,"mt5_version":"unknown","api_version":"1.0.0"}"#;
        let h: HealthStatus = serde_json::from_str(raw).unwrap();
        assert!(h.mt5_connected);
        assert_eq!(h.status, "healthy");
    }

    #[test]
    fn parse_account() {
        let raw = r#"{"data":[{"login":415817698,"trade_mode":0,"leverage":2000,"limit_orders":1024,"margin_so_mode":0,"trade_allowed":true,"trade_expert":true,"margin_mode":2,"currency_digits":2,"fifo_close":false,"balance":5000.0,"credit":0.0,"profit":0.0,"equity":5000.0,"margin":0.0,"margin_free":5000.0,"margin_level":0.0,"margin_so_call":60.0,"margin_so_so":0.0,"margin_initial":0.0,"margin_maintenance":0.0,"assets":0.0,"liabilities":0.0,"commission_blocked":0.0,"name":"Standard","server":"Exness-MT5Trial14","currency":"USD","company":"Exness Technologies Ltd"}],"count":1,"format":"json"}"#;
        let w: DataVec<AccountInfo> = serde_json::from_str(raw).unwrap();
        assert_eq!(w.data[0].login, 415817698);
        assert_eq!(w.data[0].leverage, 2000);
    }

    #[test]
    fn parse_symbol() {
        let raw = r#"{"data":[{"name":"BTCUSDm","description":"Bitcoin vs US Dollar","digits":2,"point":0.01,"bid":60708.14,"ask":60718.22,"spread":1008,"spread_float":true,"volume_min":0.01,"volume_max":200.0,"volume_step":0.01,"trade_contract_size":1.0,"currency_base":"BTC","currency_profit":"USD","category":"Crypto"}],"count":1,"format":"json"}"#;
        let w: DataVec<Symbol> = serde_json::from_str(raw).unwrap();
        assert_eq!(w.data[0].name, "BTCUSDm");
        assert_eq!(w.data[0].digits, 2);
    }

    #[test]
    fn parse_tick() {
        let raw = r#"{"data":[{"time":"2026-06-05T20:11:04","bid":60718.33,"ask":60728.41,"last":0.0,"volume":0,"time_msc":"2026-06-05T20:11:04.503000","flags":6,"volume_real":0.0}],"count":1,"format":"json"}"#;
        let w: DataVec<Tick> = serde_json::from_str(raw).unwrap();
        assert_eq!(w.data.len(), 1);
        assert_eq!(w.data[0].flags, 6);
    }

    #[test]
    fn parse_rates() {
        let raw = r#"{"data":[{"time":"2026-06-05T19:40:00","open":59765.98,"high":59874.83,"low":59516.94,"close":59707.76,"tick_volume":1371,"spread":1008,"real_volume":0},{"time":"2026-06-05T19:45:00","open":59707.6,"high":60144.13,"low":59699.21,"close":60132.58,"tick_volume":665,"spread":1008,"real_volume":0},{"time":"2026-06-05T20:10:00","open":60703.53,"high":60703.53,"low":60703.53,"close":60703.53,"tick_volume":1,"spread":1008,"real_volume":0}],"count":3,"format":"json"}"#;
        let w: DataVec<Candle> = serde_json::from_str(raw).unwrap();
        assert_eq!(w.data.len(), 3);
        assert_eq!(w.data[0].tick_volume, 1371);
    }

    #[test]
    fn parse_positions_empty() {
        let raw = r#"{"data":[],"count":0,"format":"json"}"#;
        let w: DataVec<Position> = serde_json::from_str(raw).unwrap();
        assert!(w.data.is_empty());
    }

    #[test]
    fn parse_order_check() {
        let raw = r#"{"data":{"retcode":0,"balance":5000.0,"equity":5000.0,"profit":0.0,"margin":1.52,"margin_free":4998.48,"margin_level":328947.36842105264,"comment":"Done","request":{"action":1,"magic":0,"order":0,"symbol":"BTCUSDm","volume":0.01,"price":60720.0,"stoplimit":0.0,"sl":0.0,"tp":0.0,"deviation":0,"type":0,"type_filling":0,"type_time":0,"expiration":0,"comment":"","position":0,"position_by":0}},"count":1,"format":"json"}"#;
        let w: DataOne<OrderCheckResult> = serde_json::from_str(raw).unwrap();
        assert_eq!(w.data.retcode, 0);
        assert_eq!(w.data.comment, "Done");
    }

    #[test]
    fn account_empty_data_is_error() {
        use crate::error::Mt5Error;
        let w: DataVec<AccountInfo> = serde_json::from_str(r#"{"data":[],"count":0,"format":"json"}"#).unwrap();
        let result = w.data.into_iter().next().ok_or(Mt5Error::Empty { endpoint: "/account".to_string() });
        assert!(matches!(result, Err(Mt5Error::Empty { .. })));
    }

    #[test]
    fn serialize_trade_request() {
        let tr = TradeRequest {
            action: 1,
            symbol: "BTCUSDm".into(),
            volume: 0.01,
            order_type: 0,
            price: 60720.0,
            sl: None,
            tp: None,
            magic: None,
            comment: None,
        };
        let json = serde_json::to_string(&tr).unwrap();
        assert!(json.contains(r#""type":0"#), "order_type must serialize as \"type\"");
        assert!(json.contains(r#""action":1"#));
        assert!(!json.contains(r#""sl""#), "None fields must be omitted");
        assert!(!json.contains(r#""magic""#), "None fields must be omitted");
    }

    #[test]
    fn parse_trade_result() {
        let raw = r#"{"data":{"retcode":10009,"order":123456789,"comment":"Request executed"},"count":1,"format":"json"}"#;
        let w: DataOne<TradeResult> = serde_json::from_str(raw).unwrap();
        assert_eq!(w.data.retcode, 10009);
        assert_eq!(w.data.order, 123456789);
        assert_eq!(w.data.comment, "Request executed");
    }
}
