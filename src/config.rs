use clap::Parser;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;

pub fn get() -> Result<Config, String> {
    let args = Args::parse();

    let config: Config = {
        let mut f = File::open(&args.config).map_err(|_| format!("Missing config file {}", args.config))?;

        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| format!("Error reading config: {}", e))?;
        toml::from_slice(&buf).map_err(|e| format!("Invalid config file: {}", e))?
    };

    config.validate()
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value = "./gpio2mqtt.conf")]
    pub config: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub mqtt: MqttConfig,
    #[serde(default = "PublishConfig::default")]
    pub publish: PublishConfig,
    #[serde(default = "HashMap::new", rename = "input")]
    pub inputs: HashMap<String, GpioInputConfig>,
    #[serde(default = "HashMap::new", rename = "output")]
    pub outputs: HashMap<String, GpioOutputConfig>,
    #[serde(default = "HashMap::new", rename = "i2c")]
    pub i2cs: HashMap<String, GpioI2CConfig>,
}

impl Config {
    fn validate(self) -> Result<Self, String> {
        let mut pins = HashSet::new();
        for (_, input) in &self.inputs {
            if !pins.insert(&input.pin) {
                return Err(format!("Duplicate use of pin {}", input.pin));
            }
        }
        for (_, output) in &self.outputs {
            if !pins.insert(&output.pin) {
                return Err(format!("Duplicate use of pin {}", output.pin));
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MqttConfig {
    pub host: String,
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_topic")]
    pub topic: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishConfig {
    pub interval: Option<u64>,
    pub on_change: bool,
}

impl Default for PublishConfig {
    fn default() -> Self {
        PublishConfig {
            interval: None,
            on_change: true,
        }
    }
}

fn default_mqtt_port() -> u16 {
    1883
}

fn default_client_id() -> String {
    "gpio2mqtt".to_string()
}

fn default_topic() -> String {
    "gpio2mqtt".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GpioInputConfig {
    pub pin: u8,
    // pub topic: Option<String>,
    pub pull: Option<Pull>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum Pull {
    #[serde(alias = "up")]
    Up,
    #[serde(alias = "down")]
    Down,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GpioOutputConfig {
    pub pin: u8,
    // pub topic: Option<String>,
    pub default: Option<Level>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum Level {
    #[serde(alias = "low")]
    Low,
    #[serde(alias = "high")]
    High,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GpioI2CConfig {
    pub bus: u8,
    pub module: Option<String>,
    pub address: Option<u16>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_toml_minimal() {
        let input = r#"
            [mqtt]
            host = "the.host"
            "#;

        let actual: Config = toml::from_slice(input.as_bytes()).expect("Error deserializing config");
        let expected = Config {
            mqtt: MqttConfig {
                host: "the.host".to_string(),
                port: 1883,
                username: None,
                password: None,
                client_id: "gpio2mqtt".to_string(),
                topic: "gpio2mqtt".to_string(),
            },
            outputs: HashMap::new(),
            inputs: HashMap::new(),
            i2cs: HashMap::new(),
            publish: PublishConfig {
                interval: None,
                on_change: true,
            },
        };

        assert_eq!(actual, expected);
        assert!(actual.validate().is_ok());
    }

    #[test]
    fn test_read_toml() {
        let input = r#"
            [mqtt]
            host = "the.host"
            port = 4321
            username = "uuuu"
            password = "pppp"
            client_id = "the.id"
            topic = "the.topic"
        
            [publish]
            interval = 60
            on_change = true
    
            [output.out1]
            pin = 24
        
            [input.in1]
            pin = 23
            #topic = "the.topic.1"
            pull = "up"
                
            [output.out2]
            pin = 25
            default = "low"
        
            [i2c.climate]
            bus = 1
            module = "sht22"
            address = 32
            "#;

        let actual: Config = toml::from_slice(input.as_bytes()).expect("Error deserializing config");
        let expected = Config {
            mqtt: MqttConfig {
                host: "the.host".to_string(),
                port: 4321,
                username: Some("uuuu".to_string()),
                password: Some("pppp".to_string()),
                client_id: "the.id".to_string(),
                topic: "the.topic".to_string(),
            },
            outputs: HashMap::from([
                ("out1".to_string(), GpioOutputConfig { pin: 24, default: None }),
                (
                    "out2".to_string(),
                    GpioOutputConfig {
                        pin: 25,
                        default: Some(Level::Low),
                    },
                ),
            ]),
            inputs: HashMap::from([("in1".to_string(), GpioInputConfig { pin: 23, pull: Some(Pull::Up) })]),
            i2cs: HashMap::from([(
                "climate".to_string(),
                GpioI2CConfig {
                    bus: 1,
                    module: Some("sht22".to_string()),
                    address: Some(32),
                },
            )]),
            publish: PublishConfig {
                interval: Some(60),
                on_change: true,
            },
        };

        assert_eq!(actual, expected);
        assert!(actual.validate().is_ok());
    }

    #[test]
    fn test_read_bad_toml() {
        let input = r#"
            [mqtt]
            host = "the.host"
        
            bad = "field"
            "#;

        let r: Result<Config, toml::de::Error> = toml::from_slice(input.as_bytes());
        // if let Err(e) = r {
        //     println!("err: '{:?}'", e);
        //     println!("err: '{}'", e);
        // }
        assert!(r.is_err());
    }

    #[test]
    fn test_valid() {
        let input = r#"
            [mqtt]
            host = "the.host"
        
            [publish]
            interval = 60
            on_change = true
    
            [input.in1]
            pin = 23
        
            [input.in2]
            pin = 24
                
            [output.out1]
            pin = 25
        
            [output.out2]
            pin = 26
        
            [i2c.climate]
            bus = 1
            "#;

        let actual: Config = toml::from_slice(input.as_bytes()).expect("Error deserializing config");

        assert!(actual.validate().is_ok());
    }

    #[test]
    fn test_invalid_duplicate_pin() {
        let input = r#"
            [mqtt]
            host = "the.host"
        
            [input.in1]
            pin = 24
                
            [output.out1]
            pin = 24
            "#;

        let actual: Config = toml::from_slice(input.as_bytes()).expect("Error deserializing config");

        assert!(actual.validate().unwrap_err().contains("Duplicate"));
    }
}
