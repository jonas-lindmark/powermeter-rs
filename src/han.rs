use defmt::error;
use embassy_rp::peripherals::UART1;
use embassy_rp::uart;
use embassy_rp::uart::BufferedUartRx;
use han::{AsyncReader, Direction, Error, Line, Object, Power, Telegram};
use serde::Serialize;
use static_cell::StaticCell;

use crate::{HanResources, Irqs};

#[derive(Debug, Serialize)]
pub struct Message {
    unix_timestamp: i64,
    uptime_min: u32,

    energy_active_from_grid_wh: u32,
    energy_reactive_from_grid_varh: u32,
    energy_active_to_grid_wh: u32,
    energy_reactive_to_grid_varh: u32,

    total_power_active_from_grid_w: u32,
    total_power_reactive_from_grid_var: u32,
    total_power_active_to_grid_w: u32,
    total_power_reactive_to_grid_var: u32,

    power_l1_power_active_from_grid_w: u32,
    power_l1_power_reactive_from_grid_var: u32,
    power_l1_power_active_to_grid_w: u32,
    power_l1_power_reactive_to_grid_var: u32,
    voltage_l1_dv: u16,
    current_l1_da: u16,

    power_l2_power_active_from_grid_w: u32,
    power_l2_power_reactive_from_grid_var: u32,
    power_l2_power_active_to_grid_w: u32,
    power_l2_power_reactive_to_grid_var: u32,
    voltage_l2_dv: u16,
    current_l2_da: u16,

    power_l3_power_active_from_grid_w: u32,
    power_l3_power_reactive_from_grid_var: u32,
    power_l3_power_active_to_grid_w: u32,
    power_l3_power_reactive_to_grid_var: u32,
    voltage_l3_dv: u16,
    current_l3_da: u16,

}

impl Message {
    pub fn set_uptime(&mut self, started_unix_timestamp: i64) {
        let mins = (self.unix_timestamp - started_unix_timestamp) / 60;
        self.uptime_min = u32::try_from(mins).unwrap();
    }

    pub fn unix_timestamp(&self) -> i64 {
        self.unix_timestamp
    }
}

impl Default for Message {
    fn default() -> Message {
        Message {
            unix_timestamp: 0,
            uptime_min: 0,
            energy_active_from_grid_wh: 0,
            energy_reactive_from_grid_varh: 0,
            energy_active_to_grid_wh: 0,
            energy_reactive_to_grid_varh: 0,
            total_power_active_from_grid_w: 0,
            total_power_reactive_from_grid_var: 0,
            total_power_active_to_grid_w: 0,
            total_power_reactive_to_grid_var: 0,
            power_l1_power_active_from_grid_w: 0,
            power_l1_power_reactive_from_grid_var: 0,
            power_l1_power_active_to_grid_w: 0,
            power_l1_power_reactive_to_grid_var: 0,
            voltage_l1_dv: 0,
            current_l1_da: 0,
            power_l2_power_active_from_grid_w: 0,
            power_l2_power_reactive_from_grid_var: 0,
            power_l2_power_active_to_grid_w: 0,
            power_l2_power_reactive_to_grid_var: 0,
            voltage_l2_dv: 0,
            current_l2_da: 0,
            power_l3_power_active_from_grid_w: 0,
            power_l3_power_reactive_from_grid_var: 0,
            power_l3_power_active_to_grid_w: 0,
            power_l3_power_reactive_to_grid_var: 0,
            voltage_l3_dv: 0,
            current_l3_da: 0,
        }
    }
}

pub async fn init_han(
    p: HanResources,
) -> &'static mut AsyncReader<BufferedUartRx<'static, UART1>> {
    static RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; 1024])[..];
    let rx = BufferedUartRx::new(p.uart, Irqs, p.rx_pin, rx_buf, uart::Config::default());
    static READER: StaticCell<AsyncReader<BufferedUartRx<UART1>>> = StaticCell::new();
    let reader = &mut *READER.init(AsyncReader::new(rx));

    return reader;
}

pub async fn next_message<'a>(
    reader: &'a mut AsyncReader<BufferedUartRx<'_, UART1>>,
) -> Option<Message> {
    let readout = reader.next_readout().await.map_err(|e| {
        error!("Read error: {:?}", e);
        e
    }).ok()??;

    let telegram = readout.to_telegram().map_err(|e| match e {
        Error::InvalidFormat => error!("InvalidFormat"),
        Error::Checksum => error!("Checksum"),
        Error::UnrecognizedReference => error!("UnrecognizedReference"),
    }).ok()?;

    return Some(build_message(telegram));
}

fn build_message(telegram: Telegram) -> Message {
    let mut message = Message::default();
    telegram.objects().map(|r| r.unwrap()).for_each(|o| {
        match o {
            Object::DateTime(dt) => message.unix_timestamp = dt.unix_timestamp(),
            Object::Energy(p, d, v) => match (p, d) {
                (Power::Reactive, Direction::FromGrid) => message.energy_reactive_from_grid_varh = v,
                (Power::Reactive, Direction::ToGrid) => message.energy_reactive_to_grid_varh = v,
                (Power::Active, Direction::FromGrid) => message.energy_active_from_grid_wh = v,
                (Power::Active, Direction::ToGrid) => message.energy_active_to_grid_wh = v,
            }
            Object::TotalPower(p, d, v) => match (p, d) {
                (Power::Reactive, Direction::FromGrid) => message.total_power_reactive_from_grid_var = v,
                (Power::Reactive, Direction::ToGrid) => message.total_power_reactive_to_grid_var = v,
                (Power::Active, Direction::FromGrid) => message.total_power_active_from_grid_w = v,
                (Power::Active, Direction::ToGrid) => message.total_power_active_to_grid_w = v,
            }
            Object::Power(l, p, d, v) => match (l, p, d) {
                (Line::L1, Power::Reactive, Direction::FromGrid) => message.power_l1_power_reactive_from_grid_var = v,
                (Line::L1, Power::Reactive, Direction::ToGrid) => message.power_l1_power_reactive_to_grid_var = v,
                (Line::L1, Power::Active, Direction::FromGrid) => message.power_l1_power_active_from_grid_w = v,
                (Line::L1, Power::Active, Direction::ToGrid) => message.power_l1_power_active_to_grid_w = v,
                (Line::L2, Power::Reactive, Direction::FromGrid) => message.power_l2_power_reactive_from_grid_var = v,
                (Line::L2, Power::Reactive, Direction::ToGrid) => message.power_l2_power_reactive_to_grid_var = v,
                (Line::L2, Power::Active, Direction::FromGrid) => message.power_l2_power_active_from_grid_w = v,
                (Line::L2, Power::Active, Direction::ToGrid) => message.power_l2_power_active_to_grid_w = v,
                (Line::L3, Power::Reactive, Direction::FromGrid) => message.power_l3_power_reactive_from_grid_var = v,
                (Line::L3, Power::Reactive, Direction::ToGrid) => message.power_l3_power_reactive_to_grid_var = v,
                (Line::L3, Power::Active, Direction::FromGrid) => message.power_l3_power_active_from_grid_w = v,
                (Line::L3, Power::Active, Direction::ToGrid) => message.power_l3_power_active_to_grid_w = v,
            }
            Object::Voltage(l, v) => match l {
                Line::L1 => message.voltage_l1_dv = v,
                Line::L2 => message.voltage_l2_dv = v,
                Line::L3 => message.voltage_l3_dv = v,
            }
            Object::Current(l, v) => match l {
                Line::L1 => message.current_l1_da = v,
                Line::L2 => message.current_l2_da = v,
                Line::L3 => message.current_l2_da = v,
            }
        }
    });
    return message;
}