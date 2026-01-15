#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use esp_backtrace as _;

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;

use esp_hal::main;
use esp_hal::time::Duration;

use esp_hal::timer::timg::{MwdtStage, TimerGroup};

use log::{error, info, warn};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    // generator version: 1.0.1

    // esp_println::logger::init_logger_from_env();
    esp_println::logger::init_logger(log::LevelFilter::Trace);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    info!("Running at {:?} MHz", CpuClock::max());

    // Инициализация таймера
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    // Запуск Watchdog
    let mut wdt_timer = timg0.wdt;
    wdt_timer.set_timeout(MwdtStage::Stage0, Duration::from_millis(5_000));
    wdt_timer.enable();

    info!("Main thread has started...");

    let delay = Delay::new();

    loop {
        wdt_timer.feed();
        delay.delay_millis(1000);
    }
}
