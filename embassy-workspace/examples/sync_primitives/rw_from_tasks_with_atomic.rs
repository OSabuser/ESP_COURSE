#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::sync::atomic::{AtomicU32, Ordering};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig};
use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static SHARED_CNT: AtomicU32 = AtomicU32::new(0);

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
    spawner.spawn(simple_task_second()).unwrap();

    loop {
        info!(
            "Hello from a main task! Counter value is: {}",
            SHARED_CNT.load(Ordering::Relaxed)
        );
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn simple_task() {
    loop {
        warn!("Hello from a simple task #1!");
        let shared_var = SHARED_CNT.load(Ordering::Relaxed);
        SHARED_CNT.store(shared_var.wrapping_sub(1), Ordering::Relaxed);
        Timer::after(Duration::from_millis(1000)).await;
    }
}

#[embassy_executor::task]
async fn simple_task_second() {
    loop {
        let shared_var = SHARED_CNT.load(Ordering::Relaxed);
        SHARED_CNT.store(shared_var.wrapping_add(4), Ordering::Relaxed);
        error!("Hello from a simple task #2!");
        Timer::after(Duration::from_millis(750)).await;
    }
}
