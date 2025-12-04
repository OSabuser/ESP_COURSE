#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::cell::{Cell, RefCell};

use esp_backtrace as _;

use critical_section::Mutex;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Event, Input, InputConfig, Io, Level, Output, OutputConfig};
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal::{handler, main};
use esp_hal_smartled::{SmartLedsAdapter, smart_led_buffer};
use log::{info, warn};
use smart_leds::{
    RGB8, SmartLedsWrite, brightness, gamma,
    hsv::{Hsv, hsv2rgb},
};

static BUTTON: Mutex<RefCell<Option<Input>>> = Mutex::new(RefCell::new(None));
static BTN_PRESSED: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));

#[handler]
fn button_irq_handler() {
    critical_section::with(|cs| {
        // Очистка флага прерывания
        BUTTON
            .borrow_ref_mut(cs)
            .as_mut()
            .unwrap()
            .clear_interrupt();

        // Установка флага
        BTN_PRESSED.borrow(cs).set(true);
    });
}

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    // generator version: 1.0.1

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    info!("Running at {:?} MHz", CpuClock::max());

    let mut io = Io::new(peripherals.IO_MUX);

    // Установка обработчика прерываний
    io.set_interrupt_handler(button_irq_handler);
    io.set_interrupt_priority(esp_hal::interrupt::Priority::Priority1);

    let mut led = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());

    let mut button = Input::new(
        peripherals.GPIO9,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    // Запуск обработки прерываний, передача handle button в глобальный контекст BUTTON для работы в другом потоке выполнения
    button.listen(Event::FallingEdge);
    critical_section::with(|cs| BUTTON.borrow_ref_mut(cs).replace(button));

    // Инициализация SmartLeds
    // Configure RMT (Remote Control Transceiver) peripheral globally
    let rmt: Rmt<'_, esp_hal::Blocking> = {
        let frequency: Rate = Rate::from_mhz(80);
        Rmt::new(peripherals.RMT, frequency)
    }
    .expect("Failed to initialize RMT");

    let rmt_channel = rmt.channel0;
    let mut rmt_buffer = smart_led_buffer!(1);

    let mut smart_led = SmartLedsAdapter::new(rmt_channel, peripherals.GPIO2, &mut rmt_buffer);

    let delay = Delay::new();

    let mut color = Hsv {
        hue: 0,   // Цветовой тон
        sat: 255, // Насыщенность
        val: 255, // Значение
    };
    let mut data: RGB8;
    let level = 10;

    let mut btn_pressed_cnt = 0;

    info!("Main thread has started...");

    loop {
        // Iterate over the rainbow!
        for hue in 0..=255 {
            color.hue = hue;
            // Convert from the HSV color space (where we can easily transition from one
            // color to the other) to the RGB color space that we can then send to the LED
            data = hsv2rgb(color);
            // When sending to the LED, we do a gamma correction first (see smart_leds docs
            // for details <https://docs.rs/smart-leds/latest/smart_leds/struct.Gamma.html>)
            // and then limit the brightness level to 10 out of 255 so that the output
            // is not too bright.
            smart_led
                .write(brightness(gamma([data].into_iter()), level))
                .unwrap();
            delay.delay_millis(20);
        }
        critical_section::with(|cs| {
            if BTN_PRESSED.borrow(cs).get() {
                BTN_PRESSED.borrow(cs).set(false);
                btn_pressed_cnt += 1;
                led.toggle();
                warn!("The button has been pressed {} times", btn_pressed_cnt);
            }
        });
    }
}
