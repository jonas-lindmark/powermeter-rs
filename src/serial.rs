use defmt::{error, info};
use embassy_rp::peripherals::{PIN_9, UART1};
use embassy_rp::uart;
use embassy_rp::uart::BufferedUartRx;
use han::{AsyncReader, Direction, Error, Line, Object, Power, Telegram};
use static_cell::StaticCell;

use crate::{Irqs, Payload};

pub struct SerialPeripherals {
    rx_pin: PIN_9,
    uart: UART1,
}

impl SerialPeripherals {
    pub fn new(rx_pin: PIN_9, uart: UART1) -> Self {
        Self { rx_pin, uart }
    }
}

pub async fn init_serial(
    p: SerialPeripherals,
) -> &'static mut AsyncReader<BufferedUartRx<'static, UART1>> {
    static RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; 1024])[..];
    let rx = BufferedUartRx::new(p.uart, Irqs, p.rx_pin, rx_buf, uart::Config::default());
    static READER: StaticCell<AsyncReader<BufferedUartRx<UART1>>> = StaticCell::new();
    let reader = &mut *READER.init(AsyncReader::new(rx));

    return reader;
}

pub async fn read_telegram<'a>(
    reader: &'a mut AsyncReader<BufferedUartRx<'_, UART1>>,
) -> Option<Payload> {
    let readout = reader.next_readout().await.map_err(|e| {
        error!("Read error: {:?}", e);
        e
    }).ok()??;

    let telegram = readout.to_telegram().map_err(|e| match e {
        Error::InvalidFormat => error!("InvalidFormat"),
        Error::Checksum => error!("Checksum"),
        Error::UnrecognizedReference => error!("UnrecognizedReference"),
    }).ok()?;

    info!("Got telegram: {=str}", telegram.identification);
    return Some(build_payload(telegram));
}

fn build_payload(telegram: Telegram) -> Payload {
    let mut payload = Payload::default();
    telegram.objects().map(|r| r.unwrap()).for_each(|o| {
        match o {
            Object::DateTime(dt) => payload.time = dt,
            Object::Energy(p, d, v) => match (p, d) {
                (Power::Reactive, Direction::FromGrid) => payload.energy.from_grid.reactive_varh = v,
                (Power::Reactive, Direction::ToGrid) => payload.energy.to_grid.reactive_varh = v,
                (Power::Active, Direction::FromGrid) => payload.energy.from_grid.active_wh = v,
                (Power::Active, Direction::ToGrid) => payload.energy.to_grid.active_wh = v,
            }
            Object::TotalPower(p, d, v) => match (p, d) {
                (Power::Reactive, Direction::FromGrid) => payload.total_power.from_grid.reactive_var = v,
                (Power::Reactive, Direction::ToGrid) => payload.total_power.to_grid.reactive_var = v,
                (Power::Active, Direction::FromGrid) => payload.total_power.from_grid.active_w = v,
                (Power::Active, Direction::ToGrid) => payload.total_power.to_grid.active_w = v,
            }
            Object::Power(l, p, d, v) => match (l, p, d) {
                (Line::L1, Power::Reactive, Direction::FromGrid) => payload.power_l1.from_grid.reactive_var = v,
                (Line::L1, Power::Reactive, Direction::ToGrid) => payload.power_l1.to_grid.reactive_var = v,
                (Line::L1, Power::Active, Direction::FromGrid) => payload.power_l1.from_grid.active_w = v,
                (Line::L1, Power::Active, Direction::ToGrid) => payload.power_l1.to_grid.active_w = v,
                (Line::L2, Power::Reactive, Direction::FromGrid) => payload.power_l2.from_grid.reactive_var = v,
                (Line::L2, Power::Reactive, Direction::ToGrid) => payload.power_l2.to_grid.reactive_var = v,
                (Line::L2, Power::Active, Direction::FromGrid) => payload.power_l2.from_grid.active_w = v,
                (Line::L2, Power::Active, Direction::ToGrid) => payload.power_l2.to_grid.active_w = v,
                (Line::L3, Power::Reactive, Direction::FromGrid) => payload.power_l3.from_grid.reactive_var = v,
                (Line::L3, Power::Reactive, Direction::ToGrid) => payload.power_l3.to_grid.reactive_var = v,
                (Line::L3, Power::Active, Direction::FromGrid) => payload.power_l3.from_grid.active_w = v,
                (Line::L3, Power::Active, Direction::ToGrid) => payload.power_l3.to_grid.active_w = v,
            }
            Object::Voltage(l, v) => match l {
                Line::L1 => payload.voltage_l1_dv = v,
                Line::L2 => payload.voltage_l2_dv = v,
                Line::L3 => payload.voltage_l3_dv = v,
            }
            Object::Current(l, v) => match l {
                Line::L1 => payload.current_l1_da = v,
                Line::L2 => payload.current_l2_da = v,
                Line::L3 => payload.current_l2_da = v,
            }
        }
    });
    return payload;
}