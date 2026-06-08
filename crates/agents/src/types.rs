use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_confidence<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    use serde::de::Error;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Num(f64),
        Str(String),
    }
    match NumOrStr::deserialize(d)? {
        NumOrStr::Num(v) => Ok(v.clamp(0.0, 1.0)),
        NumOrStr::Str(s) => s
            .trim()
            .parse::<f64>()
            .map(|v| v.clamp(0.0, 1.0))
            .map_err(|_| Error::custom(format!("cannot parse confidence: {s}"))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Buy,
    Sell,
    Hold,
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.to_lowercase().as_str() {
            "buy" | "long" => Ok(Action::Buy),
            "sell" | "short" => Ok(Action::Sell),
            "hold" | "neutral" | "wait" => Ok(Action::Hold),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["buy", "sell", "hold"],
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVote {
    pub action: Action,
    #[serde(deserialize_with = "deserialize_confidence")]
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

#[cfg(test)]
mod tests {
    use super::Action;

    #[test]
    fn action_aliases() {
        let cases = [
            (r#""buy""#, Action::Buy),
            (r#""long""#, Action::Buy),
            (r#""sell""#, Action::Sell),
            (r#""short""#, Action::Sell),
            (r#""hold""#, Action::Hold),
            (r#""neutral""#, Action::Hold),
            (r#""wait""#, Action::Hold),
            (r#""BUY""#, Action::Buy),
            (r#""SHORT""#, Action::Sell),
        ];
        for (input, expected) in cases {
            let got: Action = serde_json::from_str(input).unwrap();
            assert_eq!(got, expected, "input={input}");
        }
    }
}
