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
use esp_hal::Async;
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{rmt::Rmt, time::Rate};
use esp_hal_smartled::SmartLedsAdapterAsync;
use log::{error, info, warn};
use rgb::{Grb, Rgb};
use smart_leds::{
    RGB8, SmartLedsWriteAsync, brightness, gamma,
    hsv::{Hsv, hsv2rgb},
};

use esp_hal::gpio::{AnyPin, Input, InputConfig, Output};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

enum ButtonState {
    Pressed,
    Released,
}

struct UserButton {
    input: Input<'static>,
    state: ButtonState,
}

static BUTTON_PRESSED: Signal<CriticalSectionRawMutex, ButtonState> = Signal::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.1

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let sk_din_pin = AnyPin::from(peripherals.GPIO21);
    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80))
        .expect("Failed to initialize RMT0")
        .into_async();

    let user_btn = UserButton {
        input: Input::new(
            peripherals.GPIO0,
            InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        ),
        state: ButtonState::Released,
    };

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    spawner.spawn(heartbeat_task(rmt, sk_din_pin)).unwrap();
    spawner.spawn(button_read_task(user_btn)).unwrap();
    spawner.spawn(menu_task()).unwrap();

    loop {
        error!("Message sent from main");
        Timer::after(Duration::from_millis(1500)).await;
    }
}

#[embassy_executor::task]
async fn heartbeat_task(rmt_instance: Rmt<'static, Async>, pin: AnyPin<'static>) {
    let rmt_channel = rmt_instance.channel0;

    let mut rmt_buffer =
        [esp_hal::rmt::PulseCode::default(); esp_hal_smartled::buffer_size_async(1)];

    let mut led = SmartLedsAdapterAsync::new(rmt_channel, pin, &mut rmt_buffer);

    let mut data: RGB8 = Rgb::new(0, 255, 0);
    let level = 10;

    loop {
        data.g = 255;
        led.write(brightness(gamma([data].into_iter()), level))
            .await
            .unwrap();
        Timer::after(Duration::from_millis(50)).await;
        data.g = 0;
        led.write(brightness(gamma([data].into_iter()), level))
            .await
            .unwrap();
        Timer::after(Duration::from_millis(1250)).await;
    }
}

#[embassy_executor::task]
async fn button_read_task(mut button: UserButton) {
    loop {
        button.input.wait_for_falling_edge().await;
        button.state = ButtonState::Pressed;
        BUTTON_PRESSED.signal(button.state);
        Timer::after(Duration::from_millis(250)).await;
    }
}

#[embassy_executor::task]
async fn menu_task() {
    loop {
        let command = BUTTON_PRESSED.wait().await;

        match command {
            ButtonState::Pressed => {
                warn!("Button has been pressed!");
            }
            _ => {
                warn!("Not implemented!");
            }
        }
    }
}
