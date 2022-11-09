mod config;
mod data;

use config::{Config, Level};
use log::info;
use rppal::gpio::{Gpio, InputPin, Trigger};
use rumqttc::{AsyncClient, ConnectionError, Event, MqttOptions, Outgoing, QoS};
use rumqttc::{Incoming, Packet};
use serde_json::Value;
use std::collections::HashMap;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;

use crate::config::Pull;
use crate::data::HighLowToggle;

type SetType = HashMap<String, serde_json::Value>;
type DataType = HashMap<String, serde_json::Value>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // setup logging
    let env = env_logger::Env::new().filter_or("LOG", "info");
    env_logger::Builder::from_env(env).init();

    let config = config::get()
        .map_err(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        })
        .unwrap();

    log::info!("Starting");
    let (data_tx, data_rx) = mpsc::channel(2);
    let (cmd_tx, cmd_rx) = mpsc::channel(2);

    let gpio = Gpio::new().expect("Error getting gpio");

    let h1 = setup_inputs(config.clone(), gpio.clone(), data_tx).unwrap();
    let h2 = setup_outputs(config.clone(), gpio.clone(), cmd_rx).unwrap();

    start_mqtt(config, data_rx, cmd_tx).await.unwrap();

    h1.join().unwrap();
    h2.join().unwrap();
}

async fn start_mqtt(config: Config, mut data_rx: mpsc::Receiver<DataType>, cmd_tx: mpsc::Sender<SetType>) -> Result<(), tokio::io::Error> {
    let mut mqttoptions = MqttOptions::new(config.mqtt.client_id, config.mqtt.host, config.mqtt.port);
    mqttoptions.set_credentials(config.mqtt.username.unwrap(), config.mqtt.password.unwrap());
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_connection_timeout(5);
    mqttoptions.set_clean_session(true);

    log::info!("MQTT connecting.");

    let set_topic = config.mqtt.topic.to_string() + "/set";

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let loop_client = client.clone();
    task::spawn(async move {
        while let Some(data) = data_rx.recv().await {
            let msg = serde_json::to_string(&data).expect("Error serializing gpio to json");

            loop_client
                .publish(config.mqtt.topic.clone(), QoS::AtLeastOnce, false, msg)
                .await
                .map_err(|e| log::warn!("Error publishing message: {}", e))
                .ok();
        }
    });

    loop {
        let event = eventloop.poll().await;

        match event {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                log::warn!("**** Received packet {:?}", p);

                if p.topic == set_topic {
                    let cmd: Option<SetType> = serde_json::from_slice(&p.payload)
                        .map_err(|e| log::warn!("Error deserializing cmd from '{:?}': {}", p.payload, e))
                        .ok();

                    if let Some(cmd) = cmd {
                        // FIXME: await here could be dangerous as it means eventloop no longer being processed !
                        cmd_tx.send(cmd).await.expect("Cmd could not be sent");
                    }
                }
            }
            Ok(Event::Incoming(Incoming::PingReq)) => (),
            Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                log::info!("MQTT connected.  Subscribing");
                client.subscribe(&set_topic, QoS::AtMostOnce).await.unwrap();
            }
            Ok(Event::Incoming(Incoming::PingResp)) => (),
            Ok(Event::Outgoing(Outgoing::PingReq)) => (),
            Ok(Event::Outgoing(Outgoing::PingResp)) => (),
            Err(ConnectionError::Io(_)) => {
                // log::info!("Connection error : {:?}", &ce);
                // log::info!("Connection error kind: {:?}", &ce.kind());
                // if ce.kind() == std::io::ErrorKind::ConnectionRefused {
                // log::info!("Connection refused");
                // }
                log::info!("MQTT connection error. Waiting for 2 secs before trying again");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(ConnectionError::MqttState(rumqttc::StateError::Io(e))) if e.kind() == std::io::ErrorKind::ConnectionAborted => {
                log::info!("MQTT connection aborted.  Waiting for 2 secs before trying again");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(ConnectionError::ConnectionRefused(reason)) => {
                log::info!("MQTT connection refused: {:?}.  Aborting.", reason);
                return Ok(());
            }
            other => {
                log::info!("Other: {:?}", other);
            }
        }
    }
}

fn setup_outputs(config: Config, gpio: Gpio, mut commands: mpsc::Receiver<SetType>) -> Result<JoinHandle<()>, String> {
    let mut pins = HashMap::new();

    for (name, output) in config.outputs {
        let pin = gpio.get(output.pin).map_err(|e| format!("Pin {} not available: {}", output.pin, e))?;
        let output_pin = match output.default {
            Some(Level::High) => pin.into_output_high(),
            Some(Level::Low) => pin.into_output_low(),
            None => pin.into_output(),
        };

        pins.insert(name, output_pin);
    }

    let h = thread::spawn(move || {
        info!("Started output thread");

        while let Some(set) = commands.blocking_recv() {
            log::info!("Command was '{:?}'", set);

            for (set_key, set_val) in set {
                match pins.get_mut(&set_key) {
                    Some(pin) => match set_val.try_into() {
                        Ok(HighLowToggle::Low) => pin.set_low(),
                        Ok(HighLowToggle::High) => pin.set_high(),
                        Ok(HighLowToggle::Toggle) => pin.toggle(),
                        Err(e) => {
                            log::warn!("{}", e);
                        }
                    },
                    None => {
                        log::warn!("Unknown output pin '{}'", set_key);
                    }
                }
            }
        }
    });

    Ok(h)
}

fn setup_inputs(config: Config, gpio: Gpio, data_tx: mpsc::Sender<DataType>) -> Result<JoinHandle<()>, String> {
    let mut pins = HashMap::new();

    for (name, input) in config.inputs {
        let pin = gpio.get(input.pin).expect(&format!("Pin {} not available", input.pin));
        let mut input_pin = match input.pull {
            Some(Pull::Up) => pin.into_input_pullup(),
            Some(Pull::Down) => pin.into_input_pulldown(),
            None => pin.into_input(),
        };

        input_pin
            .set_interrupt(Trigger::Both)
            .map_err(|e| format!("Unable to setup pin interrupt: {}", e))
            .unwrap();

        pins.insert(name, input_pin);
    }

    let h = thread::spawn(move || {
        info!("Started input thread");

        let interrupt_pins: Vec<&InputPin> = pins.iter().map(|(_, v)| v).collect();
        let pins_by_id: HashMap<u8, &String> = pins.iter().map(|(n, v)| (v.pin(), n)).collect();

        let timeout = Duration::from_secs(10);
        loop {
            match gpio
                .poll_interrupts(&interrupt_pins[..], false, Some(timeout))
                .map_err(|e| log::warn!("polling error: {}", e))
                .unwrap()
            {
                Some((pin, level)) => {
                    let mut data = HashMap::new();
                    log::warn!("Interrupt triggered pin {:?} {:?}", pin.pin(), level);

                    let name = pins_by_id
                        .get(&pin.pin())
                        // .map(|v| v.clone())
                        .map_or_else(|| format!("pin-{}", pin.pin()), |v| v.to_string());
                    // let name = format!("{}", pin.pin());
                    let value = match level {
                        rppal::gpio::Level::Low => serde_json::Value::Bool(false),
                        rppal::gpio::Level::High => Value::Bool(true),
                    };
                    // let value = format!("{}", level);
                    data.insert(name, value);

                    data_tx.blocking_send(data).unwrap();
                }
                None => {
                    // timeout - just publish status
                    let mut data = HashMap::new();
                    for (name, pin) in pins.iter() {
                        let value = serde_json::Value::Bool(pin.is_high());
                        data.insert(name.clone(), value);
                    }

                    // log::warn!("Timeout.  Publishing {:?}", gpio);

                    data_tx.blocking_send(data).unwrap();
                }
            };
        }
    });

    log::warn!("Interrupts configured");

    Ok(h)
}
