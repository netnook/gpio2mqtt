use std::fmt;

use serde::{
    de::{self, Visitor},
    Deserialize,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HighLowToggle {
    High,
    Low,
    Toggle,
}

impl TryFrom<serde_json::Value> for HighLowToggle {
    type Error = String;
    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        match value {
            serde_json::Value::Bool(false) => Ok(HighLowToggle::Low),
            serde_json::Value::Bool(true) => Ok(HighLowToggle::High),
            serde_json::Value::Number(n) => match n.as_i64() {
                Some(0) => Ok(HighLowToggle::Low),
                Some(1) => Ok(HighLowToggle::High),
                _ => Err(format!("Cannot convert number \"{}\" to high/low/toggle", n)),
            },
            serde_json::Value::String(s) => match &s[..] {
                "off" | "low" => Ok(HighLowToggle::Low),
                "on" | "high" => Ok(HighLowToggle::High),
                "toggle" => Ok(HighLowToggle::Toggle),
                _ => Err(format!("Cannot convert string \"{}\" to high/low/toggle", s)),
            },
            _ => Err(format!("Cannot convert \"{}\" to high/low/toggle", value)),
        }
    }
}

const VARIANTS: &[&str] = &["high", "low", "on", "off", "1", "0", "true", "false", "toggle"];

impl<'de> Deserialize<'de> for HighLowToggle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HighLowToggleVisitor;
        impl<'de> Visitor<'de> for HighLowToggleVisitor {
            type Value = HighLowToggle;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(&format!("Expecting one of {:?}", VARIANTS))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    "high" | "on" | "1" => Ok(HighLowToggle::High),
                    "low" | "off" | "0" => Ok(HighLowToggle::Low),
                    "toggle" => Ok(HighLowToggle::Toggle),
                    _ => Err(de::Error::unknown_variant(value, VARIANTS)),
                }
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    true => Ok(HighLowToggle::High),
                    false => Ok(HighLowToggle::Low),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    1 => Ok(HighLowToggle::High),
                    0 => Ok(HighLowToggle::Low),
                    _ => Err(de::Error::unknown_variant(&format!("{}", value), VARIANTS)),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    1 => Ok(HighLowToggle::High),
                    0 => Ok(HighLowToggle::Low),
                    _ => Err(de::Error::unknown_variant(&format!("{}", value), VARIANTS)),
                }
            }
        }

        deserializer.deserialize_any(HighLowToggleVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_serde_high_low_toggle2() {
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""high""#).unwrap(), HighLowToggle::High);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""on""#).unwrap(), HighLowToggle::High);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""1""#).unwrap(), HighLowToggle::High);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#"1"#).unwrap(), HighLowToggle::High);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""low""#).unwrap(), HighLowToggle::Low);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""off""#).unwrap(), HighLowToggle::Low);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""0""#).unwrap(), HighLowToggle::Low);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#"0"#).unwrap(), HighLowToggle::Low);
        assert_eq!(serde_json::from_str::<HighLowToggle>(r#""toggle""#).unwrap(), HighLowToggle::Toggle);
        assert!(serde_json::from_str::<HighLowToggle>(r#"3"#).is_err());
        assert!(serde_json::from_str::<HighLowToggle>(r#"bad"#).is_err());
    }
}
