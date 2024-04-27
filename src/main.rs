//! This example test the RP Pico W on board LED.
//!
//! It does not work with the RP Pico board. See blinky.rs.

#![no_std]
#![no_main]

use core::str::FromStr;

use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{Config, Ipv4Address, Stack, StackResources};
use embassy_net::tcp::TcpSocket;
use embassy_rp::{bind_interrupts, pio};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::Pio;
use embassy_time::{Duration, Timer};
use rust_mqtt::client::client::MqttClient;
use rust_mqtt::client::client_config::{ClientConfig, MqttVersion};
use rust_mqtt::packet::v5::publish_packet::QualityOfService;
use rust_mqtt::packet::v5::reason_codes::ReasonCode;
use rust_mqtt::utils::rng_generator::CountingRng;
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

use crate::write_to::write_to::show;

mod write_to;

const WIFI_NETWORK: &str = env!("WIFI_NETWORK");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
const MQTT_SERVER_IP: &str = env!("MQTT_SERVER_IP");
const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");
const MQTT_TOPIC: &str = env!("MQTT_TOPIC");

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn wifi_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // To include cyw43 firmware in build (for uf2 builds)
    //let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    //let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download cyw43-firmware/43439A0.bin --format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs download cyw43-firmware/43439A0_clm.bin --format bin --chip RP2040 --base-address 0x10140000
    let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, p.PIN_24, p.PIN_29, p.DMA_CH0);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());
    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    // Init network stack
    static STACK: StaticCell<Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::<2>::new()),
        seed,
    ));

    unwrap!(spawner.spawn(net_task(stack)));

    loop {
        match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address); //dhcp IP address
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

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
        let mut buf = [0u8; 64];
        loop {

            info!("led off!");
            control.gpio_set(0, false).await;
            Timer::after(Duration::from_secs(1)).await;

            info!("led on!");
            control.gpio_set(0, true).await;
            count = count + 1;

            let num: &str = show(&mut buf, format_args!("{}", count)).unwrap();
            match client.send_message(MQTT_TOPIC, num.as_bytes(), QualityOfService::QoS1, false).await {
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