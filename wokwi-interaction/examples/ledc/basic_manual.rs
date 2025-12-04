#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::cell::{Cell, RefCell};
use core::error;
use esp_backtrace as _;

use critical_section::Mutex;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{DriveMode, Event, Input, InputConfig, Io, Level, Output, OutputConfig};

use esp_hal::ledc::channel::{ChannelHW, ChannelIFace};
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed, channel, timer};
use esp_hal::time::Rate;
use esp_hal::{handler, main};
use log::{error, info, warn};

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

    // Инициализация LEDC
    let led = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    // Привязка к одному из 4 доступных таймеров
    let mut lstimer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);

    if let Err(e) = lstimer0.configure(timer::config::Config {
        duty: timer::config::Duty::Duty12Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_khz(1),
    }) {
        error!("Failed to configure timer: {:?}", e);
    };

    // Создание канала и привязка к конкретному пину GPIO
    let mut channel0 = ledc.channel(channel::Number::Channel0, led);

    if let Err(e) = channel0.configure(channel::config::Config {
        timer: &lstimer0,
        duty_pct: 25,
        drive_mode: DriveMode::PushPull,
    }) {
        error!("Failed to configure channel: {:?}", e);
    }

    // F = 1kHz, T = 1ms, Tp = 0.5ms (duty = 50%)
    // F = 1kHz, T = 1ms, Tp = 0.25ms (duty = 25%)
    // F = 4kHz, T = 125us, Tp = 250us (duty = 50%)
    let mut button = Input::new(
        peripherals.GPIO9,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    // Запуск обработки прерываний, передача handle button в глобальный контекст BUTTON для работы в другом потоке выполнения
    button.listen(Event::FallingEdge);
    critical_section::with(|cs| BUTTON.borrow_ref_mut(cs).replace(button));

    let mut btn_pressed_cnt = 0;

    let delay = esp_hal::delay::Delay::new();

    info!("Main thread has started...");

    loop {
        //frequency * duration / ((1<<bit_count) * abs(start-end)) < 1024
        // for duty_value in 0..=100 {
        //     channel0.set_duty(duty_value).unwrap();
        //     delay.delay_millis(10u32);
        // }

        // for duty_value in (0..=100).rev() {
        //     channel0.set_duty(duty_value).unwrap();
        //     delay.delay_millis(10u32);
        // }

        critical_section::with(|cs| {
            if BTN_PRESSED.borrow(cs).get() {
                BTN_PRESSED.borrow(cs).set(false);
                btn_pressed_cnt += 1;
                warn!("The button has been pressed {} times", btn_pressed_cnt);
            }
        });
    }
}
