use core::str::FromStr;

use cyw43::NetDriver;
use defmt::{error, info};
use embassy_net::{Ipv4Address, Stack};
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Timer};
use rust_mqtt::client::client::MqttClient;
use rust_mqtt::client::client_config::{ClientConfig, MqttVersion};
use rust_mqtt::packet::v5::publish_packet::QualityOfService;
use rust_mqtt::packet::v5::reason_codes::ReasonCode;
use rust_mqtt::utils::rng_generator::CountingRng;
use static_cell::StaticCell;

const MQTT_SERVER_IP: &str = env!("MQTT_SERVER_IP");
const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");
const MQTT_TOPIC: &str = env!("MQTT_TOPIC");

struct MqttResources {
    rx_buffer: [u8; 4096],
    tx_buffer: [u8; 4096],
    client_rx_buffer: [u8; 2048],
    client_tx_buffer: [u8; 2048],
}

pub async fn init_mqtt_client(
    stack: &'static Stack<NetDriver<'static>>,
) -> Result<&'static mut MqttClient<TcpSocket, 5, CountingRng>, ()> {

    static MQTT_RESOURCES: StaticCell<MqttResources> = StaticCell::new();
    let resources = &mut *MQTT_RESOURCES.init(MqttResources {
        rx_buffer: [0; 4096],
        tx_buffer: [0; 4096],
        client_rx_buffer: [0; 2048],
        client_tx_buffer: [0; 2048],
    });

    info!("creating socket");

    let mut socket = TcpSocket::new(&stack, &mut resources.rx_buffer, &mut resources.tx_buffer);
    info!("socket created");

    socket.set_timeout(Some(Duration::from_secs(10)));
    info!("socket set timeout");

    let address = Ipv4Address::from_str(MQTT_SERVER_IP).unwrap();

    let remote_endpoint = (address, 1883);

    info!("Connecting...");

    let connection = socket.connect(remote_endpoint).await;
    if let Err(e) = connection {
        info!("connect error: {:?}", e);
        return Err(());
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

    static CLIENT: StaticCell<MqttClient<TcpSocket, 5, CountingRng>> = StaticCell::new();
    let client = &mut *CLIENT.init(MqttClient::<_, 5, _>::new(
        socket,
        &mut resources.client_tx_buffer,
        2048,
        &mut resources.client_rx_buffer,
        2048,
        config
    ));

    match client.connect_to_broker().await {
        Ok(()) => { info!("Connected to broker") }
        Err(mqtt_error) => return match mqtt_error {
            ReasonCode::NetworkError => {
                error!("MQTT Network Error");
                Err(())
            }
            _ => {
                error!("Other MQTT Error: {:?}", mqtt_error);
                Err(())
            }
        },
    }
    Ok(client)
}

pub async fn send_message<'a>(client: &mut MqttClient<'a, TcpSocket<'a>, 5, CountingRng>, message: &[u8]) {

    match client.send_message(MQTT_TOPIC, message, QualityOfService::QoS1, false).await {
        Ok(()) => {}
        Err(mqtt_error) => match mqtt_error {
            ReasonCode::NetworkError => {
                error!("MQTT Network Error");
                Timer::after(Duration::from_secs(1)).await;
            }
            _ => {
                error!("Other MQTT Error: {:?}", mqtt_error);
                Timer::after(Duration::from_secs(1)).await;
            }
        },
    }
}
