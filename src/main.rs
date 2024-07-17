#![no_std]
#![no_main]

use core::cell::RefCell;

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, pio, uart};
use embassy_rp::peripherals::{PIO0, UART1, WATCHDOG};
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_time::{Duration, Timer};
use serde_json_core::to_string;

use crate::han::{HanPeripherals, init_han, Message, next_message};
use crate::mqtt::{init_mqtt_client, send_message};
use crate::wifi::{init_wifi, WifiPeripherals};

use {defmt_rtt as _, panic_probe as _};

mod wifi;
mod mqtt;
mod han;

static WATCHDOG_COUNTER: Mutex<ThreadModeRawMutex, RefCell<u32>> = Mutex::new(RefCell::new(0));

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
});


#[embassy_executor::task]
async fn watchdog_task(watchdog_peripheral: WATCHDOG) {
    let mut watchdog = Watchdog::new(watchdog_peripheral);
    watchdog.start(Duration::from_millis(1_500));
    loop {
        let counter = WATCHDOG_COUNTER.lock(|f| {
            let val = f.borrow_mut().wrapping_add(1);
            f.replace(val)
        });
        match counter {
            0..=1 => watchdog.feed(),
            2..=30 => {
                watchdog.feed();
                info!("Watchdog {}", counter);

            },
            _ => info!("Watchdog {} not feeding", counter),
        }
        Timer::after(Duration::from_millis(1000)).await;
    }
}

fn clear_watchdog() {
    WATCHDOG_COUNTER.lock(|f| {
        f.replace(0);
    });
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    spawner.spawn(watchdog_task(p.WATCHDOG)).unwrap();

    let wp =
        WifiPeripherals::new(p.PIN_23, p.PIN_25, p.PIN_24, p.PIN_29, p.PIO0, p.DMA_CH0);
    let (mut control, stack) = init_wifi(spawner, wp).await;
    control.gpio_set(0, true).await;

    let mut started_unix_timestamp: Option<i64> = None;

    info!("Starting main loop");
    loop {
        let client = match init_mqtt_client(stack).await {
            Ok(c) => c,
            Err(()) => continue
        };

        let sp = HanPeripherals::new(p.PIN_9, p.UART1);
        let mut han_reader = init_han(sp).await;

        control.gpio_set(0, false).await;
        loop {
            clear_watchdog();

            if let Some(mut message) = next_message(&mut han_reader).await {
                info!("Got message with timestamp {}", message.unix_timestamp());
                if started_unix_timestamp.is_none() {
                    started_unix_timestamp = Some(message.unix_timestamp());
                }
                message.set_uptime(started_unix_timestamp.unwrap());
                let string_message = to_string::<Message, 2048>(&message).unwrap();
                send_message(client, string_message.as_bytes()).await
            }

            control.gpio_set(0, true).await;
            Timer::after(Duration::from_millis(50)).await;

            control.gpio_set(0, false).await;
        }
    }
}

