use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVote {
    pub action: Action,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct AgentDecision {
    pub action: Action,
    pub confidence: f64,
    pub reasoning: String,
    pub votes: [AgentVote; 4],
    pub from_quorum: bool,
}

pub(crate) fn agent_vote_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action":     { "type": "string", "enum": ["buy", "sell", "hold"] },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "reasoning":  { "type": "string" }
        },
        "required": ["action", "confidence", "reasoning"],
        "additionalProperties": false
    })
}
