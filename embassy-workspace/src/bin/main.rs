#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

mod state_machine;

use embassy_executor::Spawner;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};

use esp_hal::clock::CpuClock;

use esp_hal::gpio::Input;

use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

enum ButtonState {
    Pressed,
    Released,
}

struct UserButton {
    input: Input<'static>,
    state: ButtonState,
}

static BUTTON_PRESSED: Signal<CriticalSectionRawMutex, ButtonState> = Signal::new();

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

    let user_btn = UserButton {
        input: Input::new(
            peripherals.GPIO0,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        ),
        state: ButtonState::Released,
    };

    warn!("=== ESP32 Config Demo ===");
    warn!("Device: {}", Config::DEVICE_NAME);
    warn!("Update interval: {}ms", Config::UPDATE_INTERVAL_MS);
    warn!("Max retries: {}", Config::MAX_RETRIES);
    warn!("WiFi SSID: {}", Config::WIFI_SSID);
    warn!("LED PIN: {}", Config::LED_PIN);
    warn!("Log level: {}", Config::LOG_LEVEL);

    spawner
        .spawn(button_read_task(user_btn))
        .expect("Failed to spawn button task");
    spawner
        .spawn(heartbeat_task())
        .expect("Failed to spawn heartbeat task");
    spawner
        .spawn(state_machine::state_machine_task())
        .expect("Failed to spawn handle event task");

    loop {
        let command = BUTTON_PRESSED.wait().await;

        match command {
            ButtonState::Pressed => {
                error!("Button has been pressed!");
            }
            ButtonState::Released => {
                error!("Button has been released!");
            }
        }
    }
}

#[embassy_executor::task]
async fn button_read_task(mut button: UserButton) {
    loop {
        button.input.wait_for_falling_edge().await;
        button.state = ButtonState::Pressed;
        BUTTON_PRESSED.signal(button.state);
        button.input.wait_for_rising_edge().await;
        button.state = ButtonState::Released;
        BUTTON_PRESSED.signal(button.state);
        Timer::after(Duration::from_millis(250)).await;
    }
}

#[embassy_executor::task]
async fn heartbeat_task() {
    loop {
        info!("We are alive");
        Timer::after(Duration::from_millis(2000)).await;
    }
}
