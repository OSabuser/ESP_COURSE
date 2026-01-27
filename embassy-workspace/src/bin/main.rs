#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

mod button;
mod keyboard;
mod manager;
mod state_machine;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use log::{info, warn};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// Включаем сгенерированный конфиг файл
include!(concat!(env!("OUT_DIR"), "/config.rs"));

esp_bootloader_esp_idf::esp_app_desc!();
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let button = esp_hal::gpio::Input::new(
        peripherals.GPIO0,
        esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    let keyboard = keyboard::Keyboard::new(
        peripherals.GPIO8,
        peripherals.GPIO9,
        peripherals.GPIO11,
        peripherals.GPIO13,
        peripherals.GPIO15,
        peripherals.GPIO3,
        peripherals.GPIO4,
        peripherals.GPIO5,
        peripherals.GPIO6,
        peripherals.GPIO7,
    );

    // Button - Define and spawn async task
    let button_info = [(0_u8, button)];

    warn!("=== ESP32 Config Demo ===");
    warn!("Device: {}", AppConfig::DEVICE_NAME);
    warn!(
        "Button's long press threshold: {}",
        AppConfig::BTN_LONG_PRESS_THRESHOLD_MS
    );
    warn!(
        "Button's long hold threshold: {}",
        AppConfig::BTN_LONG_HOLD_THRESHOLD_MS
    );
    warn!("Log level: {}", AppConfig::LOG_LEVEL);

    // TODO: connect with manager
    spawner
        .spawn(keyboard::start_keyboard_scan(keyboard))
        .expect("Failed to spawn keyboard scan task");

    // Task to check if all system components are ready to go
    spawner
        .spawn(manager::wait_for_system_ready())
        .expect("Failed to spawn wait_for_system_ready");

    spawner
        .spawn(state_machine::state_machine_task())
        .expect("Failed to spawn handle event task");
    spawner
        .spawn(button::start_button_monitor(button_info))
        .expect("Failed spawning button_consumer");

    loop {
        info!("=== ESP32 Config Demo ===");
        Timer::after(Duration::from_millis(10000)).await;
    }
}
