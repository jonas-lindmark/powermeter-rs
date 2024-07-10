#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, pio, uart};
use embassy_rp::peripherals::{PIO0, UART1};
use embassy_time::{Duration, Timer};
use serde::Serialize;
use serde_json_core::to_string;
use time::OffsetDateTime;

use crate::mqtt::{init_mqtt_client, send_message};
use crate::serial::{init_serial, read_telegram, SerialPeripherals};
use crate::wifi::{init_wifi, WifiPeripherals};


use {defmt_rtt as _, panic_probe as _};

mod wifi;
mod mqtt;
mod serial;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
});

#[derive(Debug, Serialize, Default)]
struct EnergyPayload {
    active_wh: u32,
    reactive_varh: u32,
}

#[derive(Debug, Serialize, Default)]
struct PowerPayload {
    active_w: u32,
    reactive_var: u32,
}

#[derive(Debug, Serialize, Default)]
struct EnergyWithDirection {
    from_grid: EnergyPayload,
    to_grid: EnergyPayload,
}

#[derive(Debug, Serialize, Default)]
struct PowerWithDirection {
    from_grid: PowerPayload,
    to_grid: PowerPayload,
}

#[derive(Debug, Serialize)]
struct Payload {
    time: OffsetDateTime,

    energy: EnergyWithDirection,

    total_power: PowerWithDirection,

    power_l1: PowerWithDirection,
    power_l2: PowerWithDirection,
    power_l3: PowerWithDirection,

    voltage_l1_dv: u16,
    voltage_l2_dv: u16,
    voltage_l3_dv: u16,

    current_l1_da: u16,
    current_l2_da: u16,
    current_l3_da: u16,
}

impl Default for Payload {
    fn default() -> Payload {
        Payload {
            time: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            energy: Default::default(),
            total_power: Default::default(),
            power_l1: Default::default(),
            power_l2: Default::default(),
            power_l3: Default::default(),
            voltage_l1_dv: 0,
            voltage_l2_dv: 0,
            voltage_l3_dv: 0,
            current_l1_da: 0,
            current_l2_da: 0,
            current_l3_da: 0,
        }
    }
}



#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let wp =
        WifiPeripherals::new(p.PIN_23, p.PIN_25, p.PIN_24, p.PIN_29, p.PIO0, p.DMA_CH0);
    let (mut control, stack) = init_wifi(spawner, wp).await;

    info!("Starting main loop");
    loop {
        let client = match init_mqtt_client(stack).await {
            Ok(c) => c,
            Err(()) => continue
        };

        let sp = SerialPeripherals::new(p.PIN_9, p.UART1);
        let mut han_reader = init_serial(sp).await;

        loop {
            if let Some(payload) = read_telegram(&mut han_reader).await {
                let message = to_string::<Payload, 1024>(&payload).unwrap();
                send_message(client, message.as_bytes()).await
            }

            control.gpio_set(0, true).await;
            Timer::after(Duration::from_millis(50)).await;

            control.gpio_set(0, false).await;
        }
    }
}