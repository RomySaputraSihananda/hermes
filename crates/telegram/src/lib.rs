use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub token:     String,
    pub chat_id:   i64,
    pub thread_id: Option<i64>,
}

#[derive(Deserialize)]
struct TgResp<T> {
    ok:          bool,
    result:      Option<T>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct TgMsg {
    message_id: i64,
}

impl TelegramConfig {
    fn url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }

    pub async fn send(&self, http: &reqwest::Client, text: &str) -> Result<i64> {
        let mut body = serde_json::json!({
            "chat_id":                  self.chat_id,
            "text":                     text,
            "parse_mode":               "HTML",
            "disable_web_page_preview": true,
        });
        if let Some(tid) = self.thread_id {
            body["message_thread_id"] = serde_json::json!(tid);
        }
        let resp: TgResp<TgMsg> = http
            .post(self.url("sendMessage"))
            .json(&body)
            .send().await?
            .json().await?;
        if !resp.ok {
            anyhow::bail!("sendMessage: {}", resp.description.unwrap_or_default());
        }
        Ok(resp.result.context("no result")?.message_id)
    }

    pub async fn edit(&self, http: &reqwest::Client, msg_id: i64, text: &str) -> Result<()> {
        let body = serde_json::json!({
            "chat_id":                  self.chat_id,
            "message_id":               msg_id,
            "text":                     text,
            "parse_mode":               "HTML",
            "disable_web_page_preview": true,
        });
        let resp: TgResp<serde_json::Value> = http
            .post(self.url("editMessageText"))
            .json(&body)
            .send().await?
            .json().await?;
        if !resp.ok {
            tracing::warn!("Telegram editMessageText: {}", resp.description.unwrap_or_default());
        }
        Ok(())
    }
}
