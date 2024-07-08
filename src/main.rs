//! This example test the RP Pico W on board LED.
//!
//! It does not work with the RP Pico board. See blinky.rs.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, pio, uart};
use embassy_rp::peripherals::{PIO0, UART1};
use embassy_time::{Duration, Timer};
use serde::Serialize;
use serde_json_core::to_string;

use crate::mqtt::{init_mqtt_client, send_message};
use crate::wifi::{init_wifi, WifiPeripherals};

use {defmt_rtt as _, panic_probe as _};
use crate::serial::{init_serial, SerialPeripherals};

mod wifi;
mod mqtt;
mod serial;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
});

#[derive(Debug, Serialize)]
struct Payload {
    data: u32,
}



#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let wp =
        WifiPeripherals::new(p.PIN_23, p.PIN_25, p.PIN_24, p.PIN_29, p.PIO0, p.DMA_CH0);
    let (mut control, stack) = init_wifi(spawner, wp).await;

    let sp = SerialPeripherals::new(p.PIN_9, p.UART1);
    init_serial(spawner, sp).await;

    loop {
        let mut client = match init_mqtt_client(stack).await {
            Ok(c) => c,
            Err(()) => continue
        };

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

            send_message(&mut client, message.as_bytes()).await;
        }
    }
}