#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, WaitResult};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig};
use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

// Размер очереди: 2, 2 подписчика и 1 продюсер
static SHARED_CNT: PubSubChannel<CriticalSectionRawMutex, u8, 2, 2, 2> = PubSubChannel::new();

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

    spawner.spawn(subscriber_task_1()).unwrap();
    spawner.spawn(subscriber_task_2()).unwrap();
    spawner.spawn(publisher_task_1()).unwrap();

    let pub1 = SHARED_CNT.publisher().unwrap();
    let mut cnt = 0;
    loop {
        // Отправка без ожидания с возможным "затиранием" данных, которые не были прочитаны подписчиками
        pub1.publish_immediate(cnt);
        error!("Message sent from main");
        cnt += 1;
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn subscriber_task_1() {
    let mut sub_1 = SHARED_CNT.subscriber().unwrap();
    loop {
        let data = sub_1.next_message().await;

        if let WaitResult::Message(msg) = data {
            warn!("T1 received: {}", msg);
        }
    }
}

#[embassy_executor::task]
async fn subscriber_task_2() {
    let mut sub_2 = SHARED_CNT.subscriber().unwrap();

    loop {
        let val = sub_2.next_message_pure().await;
        warn!("T2 received: {}", val);
    }
}

#[embassy_executor::task]
async fn publisher_task_1() {
    let pub2 = SHARED_CNT.publisher().unwrap();

    loop {
        pub2.publish_immediate(77);
        error!("Message sent from pbt");
        Timer::after(Duration::from_millis(250)).await;
    }
}
