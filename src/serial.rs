use defmt::{error, info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::peripherals::{PIN_9, UART1};
use embassy_rp::uart;
use embassy_rp::uart::BufferedUartRx;
use han::{AsyncReader, Error};
use static_cell::StaticCell;

use crate::Irqs;

pub struct SerialPeripherals {
    rx_pin: PIN_9,
    uart: UART1,
}

impl SerialPeripherals {
    pub fn new(rx_pin: PIN_9, uart: UART1) -> Self {
        Self { rx_pin, uart }
    }
}

#[embassy_executor::task]
async fn serial_task(reader: &'static mut AsyncReader<BufferedUartRx<'_, UART1>>) -> ! {
    loop {
        match reader.next_readout().await {
            Ok(o) => {
                match o {
                    None => info!("No HAN readout"),
                    Some(r) => {
                        match r.to_telegram() {
                            Ok(t) => info!("Got telegram: {=str}", t.identification),
                            Err(e) => match e {
                                Error::InvalidFormat => error!("InvalidFormat"),
                                Error::Checksum => error!("Checksum"),
                                Error::UnrecognizedReference => error!("UnrecognizedReference"),
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Read error: {:?}", e),
        };
    }
}

pub async fn init_serial(
    spawner: Spawner,
    p: SerialPeripherals,
) {
    static RX_BUF: StaticCell<[u8; 16]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; 16])[..];
    let rx = BufferedUartRx::new(p.uart, Irqs, p.rx_pin, rx_buf, uart::Config::default());
    static READER: StaticCell<AsyncReader<BufferedUartRx<UART1>>> = StaticCell::new();
    let reader= &mut *READER.init(AsyncReader::new(rx));
    unwrap!(spawner.spawn(serial_task(reader)));
}

