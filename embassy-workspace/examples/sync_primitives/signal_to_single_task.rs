#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig};
use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

enum SomeCommand {
    On,
    Off,
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static SHARED_CNT: Signal<CriticalSectionRawMutex, SomeCommand> = Signal::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.1

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let mut _button = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    info!("Embassy initialized!");

    spawner.spawn(simple_task()).unwrap();

    loop {
        let command = SHARED_CNT.wait().await;
        match command {
            SomeCommand::On => warn!("Must be ON!"),
            SomeCommand::Off => warn!("Must be OFF!"),
        }

        info!("Hello from a main task!");
    }
}

#[embassy_executor::task]
async fn simple_task() {
    loop {
        info!("Hello from a simple task #1!");
        Timer::after(Duration::from_millis(2000)).await;
        SHARED_CNT.signal(SomeCommand::On);
    }
}
