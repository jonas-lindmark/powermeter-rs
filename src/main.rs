//! This example test the RP Pico W on board LED.
//!
//! It does not work with the RP Pico board. See blinky.rs.

#![no_std]
#![no_main]

use core::str::FromStr;

use defmt::*;
use embassy_executor::Spawner;
use embassy_net::Ipv4Address;
use embassy_net::tcp::TcpSocket;
use embassy_rp::{bind_interrupts, pio};
use embassy_rp::peripherals::PIO0;
use embassy_time::{Duration, Timer};
use rust_mqtt::client::client::MqttClient;
use rust_mqtt::client::client_config::{ClientConfig, MqttVersion};
use rust_mqtt::packet::v5::publish_packet::QualityOfService;
use rust_mqtt::packet::v5::reason_codes::ReasonCode;
use rust_mqtt::utils::rng_generator::CountingRng;
use serde::Serialize;
use serde_json_core::to_string;

use {defmt_rtt as _, panic_probe as _};

use crate::wifi::init_wifi;

mod wifi;

const WIFI_NETWORK: &str = env!("WIFI_NETWORK");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
const MQTT_SERVER_IP: &str = env!("MQTT_SERVER_IP");
const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");
const MQTT_TOPIC: &str = env!("MQTT_TOPIC");

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

#[derive(Debug, Serialize)]
struct Payload {
    data: u32,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let (mut control, stack) = init_wifi(spawner, p).await.unwrap();


    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        info!("creating socket");
        Timer::after(Duration::from_secs(1)).await;

        let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
        info!("socket created");
        Timer::after(Duration::from_secs(1)).await;

        socket.set_timeout(Some(Duration::from_secs(10)));
        info!("socket set timeout");
        Timer::after(Duration::from_secs(1)).await;

        let address = Ipv4Address::from_str(MQTT_SERVER_IP).unwrap();

        let remote_endpoint = (address, 1883);

        info!("Connecting...");
        Timer::after(Duration::from_secs(1)).await;

        let connection = socket.connect(remote_endpoint).await;
        if let Err(e) = connection {
            info!("connect error: {:?}", e);
            continue;
        }
        info!("connected!");

        let mut config = ClientConfig::new(
            MqttVersion::MQTTv5,
            CountingRng(20000),
        );
        config.add_max_subscribe_qos(QualityOfService::QoS1);
        config.add_client_id("clientId-8rhWgBODCl");
        config.add_username(MQTT_USER);
        config.add_password(MQTT_PASSWORD);
        config.max_packet_size = 100;
        let mut recv_buffer = [0; 80];
        let mut write_buffer = [0; 80];

        let mut client =
            MqttClient::<_, 5, _>::new(socket, &mut write_buffer, 80, &mut recv_buffer, 80, config);

        match client.connect_to_broker().await {
            Ok(()) => { info!("Connected to broker") }
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    info!("MQTT Network Error");
                    continue;
                }
                _ => {
                    info!("Other MQTT Error: {:?}", mqtt_error);
                    continue;
                }
            },
        }

        let mut count: u32 = 0;
        loop {
            info!("led off!");
            control.gpio_set(0, false).await;
            Timer::after(Duration::from_secs(1)).await;

            info!("led on!");
            control.gpio_set(0, true).await;
            count = count + 1;

            let payload = Payload { data: count };

            let message = to_string::<Payload, 1024>(&payload).unwrap();
            match client.send_message(MQTT_TOPIC, message.as_bytes(), QualityOfService::QoS1, false).await {
                Ok(()) => {}
                Err(mqtt_error) => match mqtt_error {
                    ReasonCode::NetworkError => {
                        info!("MQTT Network Error");
                        Timer::after(Duration::from_secs(1)).await;
                        continue;
                    }
                    _ => {
                        info!("Other MQTT Error: {:?}", mqtt_error);
                        Timer::after(Duration::from_secs(1)).await;
                        continue;
                    }
                },
            }
        }
    }
}