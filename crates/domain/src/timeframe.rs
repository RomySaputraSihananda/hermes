use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    #[serde(rename = "TIMEFRAME_M1")]  M1,
    #[serde(rename = "TIMEFRAME_M2")]  M2,
    #[serde(rename = "TIMEFRAME_M3")]  M3,
    #[serde(rename = "TIMEFRAME_M4")]  M4,
    #[serde(rename = "TIMEFRAME_M5")]  M5,
    #[serde(rename = "TIMEFRAME_M6")]  M6,
    #[serde(rename = "TIMEFRAME_M10")] M10,
    #[serde(rename = "TIMEFRAME_M12")] M12,
    #[serde(rename = "TIMEFRAME_M15")] M15,
    #[serde(rename = "TIMEFRAME_M20")] M20,
    #[serde(rename = "TIMEFRAME_M30")] M30,
    #[serde(rename = "TIMEFRAME_H1")]  H1,
    #[serde(rename = "TIMEFRAME_H2")]  H2,
    #[serde(rename = "TIMEFRAME_H3")]  H3,
    #[serde(rename = "TIMEFRAME_H4")]  H4,
    #[serde(rename = "TIMEFRAME_H6")]  H6,
    #[serde(rename = "TIMEFRAME_H8")]  H8,
    #[serde(rename = "TIMEFRAME_H12")] H12,
    #[serde(rename = "TIMEFRAME_D1")]  D1,
    #[serde(rename = "TIMEFRAME_W1")]  W1,
    #[serde(rename = "TIMEFRAME_MN1")] Mn1,
}

impl Timeframe {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::M1  => "TIMEFRAME_M1",
            Self::M2  => "TIMEFRAME_M2",
            Self::M3  => "TIMEFRAME_M3",
            Self::M4  => "TIMEFRAME_M4",
            Self::M5  => "TIMEFRAME_M5",
            Self::M6  => "TIMEFRAME_M6",
            Self::M10 => "TIMEFRAME_M10",
            Self::M12 => "TIMEFRAME_M12",
            Self::M15 => "TIMEFRAME_M15",
            Self::M20 => "TIMEFRAME_M20",
            Self::M30 => "TIMEFRAME_M30",
            Self::H1  => "TIMEFRAME_H1",
            Self::H2  => "TIMEFRAME_H2",
            Self::H3  => "TIMEFRAME_H3",
            Self::H4  => "TIMEFRAME_H4",
            Self::H6  => "TIMEFRAME_H6",
            Self::H8  => "TIMEFRAME_H8",
            Self::H12 => "TIMEFRAME_H12",
            Self::D1  => "TIMEFRAME_D1",
            Self::W1  => "TIMEFRAME_W1",
            Self::Mn1 => "TIMEFRAME_MN1",
        }
    }
}

impl std::str::FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "M1"  => Ok(Self::M1),
            "M2"  => Ok(Self::M2),
            "M3"  => Ok(Self::M3),
            "M4"  => Ok(Self::M4),
            "M5"  => Ok(Self::M5),
            "M6"  => Ok(Self::M6),
            "M10" => Ok(Self::M10),
            "M12" => Ok(Self::M12),
            "M15" => Ok(Self::M15),
            "M20" => Ok(Self::M20),
            "M30" => Ok(Self::M30),
            "H1"  => Ok(Self::H1),
            "H2"  => Ok(Self::H2),
            "H3"  => Ok(Self::H3),
            "H4"  => Ok(Self::H4),
            "H6"  => Ok(Self::H6),
            "H8"  => Ok(Self::H8),
            "H12" => Ok(Self::H12),
            "D1"  => Ok(Self::D1),
            "W1"  => Ok(Self::W1),
            "MN1" => Ok(Self::Mn1),
            other => Err(format!("unknown timeframe: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_api_str_spot_check() {
        assert_eq!(Timeframe::M5.as_api_str(), "TIMEFRAME_M5");
        assert_eq!(Timeframe::H4.as_api_str(), "TIMEFRAME_H4");
        assert_eq!(Timeframe::D1.as_api_str(), "TIMEFRAME_D1");
        assert_eq!(Timeframe::Mn1.as_api_str(), "TIMEFRAME_MN1");
    }

    #[test]
    fn serde_round_trip() {
        let tf = Timeframe::H1;
        let json = serde_json::to_string(&tf).unwrap();
        assert_eq!(json, r#""TIMEFRAME_H1""#);
        let back: Timeframe = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tf);
    }

    #[test]
    fn from_str_known_variant() {
        let tf: Timeframe = "M15".parse().unwrap();
        assert_eq!(tf, Timeframe::M15);
    }

    #[test]
    fn from_str_unknown_returns_err() {
        let result = "invalid".parse::<Timeframe>();
        assert!(result.is_err());
    }
}
