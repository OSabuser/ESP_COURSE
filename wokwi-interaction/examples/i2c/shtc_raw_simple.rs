#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

/// Работа с датчиком температуры и влажности SHT3C
/// I2C: Standard (100kHz) - Fast-mode Plus (400kHz)
/// I2C address: 0x70
/// ESP SCL:IO8
/// ESP SDA:IO10
use core::cell::{Cell, RefCell};
use esp_backtrace as _;

use critical_section::Mutex;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Event, Input, InputConfig, Io, Level, Output, OutputConfig};

use esp_hal::i2c::master::I2c;
use esp_hal::riscv::asm::delay;
use esp_hal::time::Duration;
use esp_hal::timer::PeriodicTimer;
use esp_hal::timer::timg::{MwdtStage, TimerGroup};
use esp_hal::{Blocking, delay, handler, main};
use esp_hal::{i2c::master::Config, time::Rate};
use log::{error, info, warn};

static BUTTON: Mutex<RefCell<Option<Input>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<PeriodicTimer<'_, Blocking>>>> = Mutex::new(RefCell::new(None));

static PERIOD_ELAPSED: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));
static BTN_PRESSED: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));

const SHTC_ADDR: u8 = 0x70;
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

#[handler]
fn gpt_irq_handler() {
    critical_section::with(|cs| {
        // Очистка флага прерывания
        TIMER.borrow_ref_mut(cs).as_mut().unwrap().clear_interrupt();

        // Установка флага
        PERIOD_ELAPSED.borrow(cs).set(true);
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

    // Инициализация I2C
    let config = Config::default().with_frequency(Rate::from_khz(400));

    let mut i2c = I2c::new(peripherals.I2C0, config)
        .unwrap()
        .with_sda(peripherals.GPIO10)
        .with_scl(peripherals.GPIO8);

    let mut read_buffer = [0u8; 2];

    let delay = Delay::new();

    info!("[1] Putting the SHTC3 to sleep");
    i2c.write(SHTC_ADDR, &[0xB0, 0x98]).unwrap();

    info!("[2] Waking up the SHTC3");
    i2c.write(SHTC_ADDR, &[0x35, 0x17]).unwrap();

    delay.delay_millis(100);

    info!("[3] Reading the SHTC3 ID");
    i2c.write(SHTC_ADDR, &[0xEF, 0xC8]).unwrap();
    i2c.read(SHTC_ADDR, &mut read_buffer).unwrap();

    let id =
        (((((read_buffer[0] >> 3) & 0x01) as u16) << 8) >> 2) | ((read_buffer[1] & 0x3F) as u16);

    error!("[4] SHTC3 ID: {:#04x}", id);

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

    // Инициализация таймера
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let mut periodic_timer = PeriodicTimer::new(timg0.timer0);
    periodic_timer.set_interrupt_handler(gpt_irq_handler);
    periodic_timer.listen();

    // Запуск таймера
    if let Err(e) = periodic_timer.start(esp_hal::time::Duration::from_millis(1000)) {
        error!("Failed to start timer: {:?}", e);
    }
    critical_section::with(|cs| TIMER.borrow_ref_mut(cs).replace(periodic_timer));

    // Запуск Watchdog
    let mut wdt_timer = timg0.wdt;
    wdt_timer.set_timeout(MwdtStage::Stage0, Duration::from_millis(5_000));
    wdt_timer.enable();

    info!("Main thread has started...");
    let mut count = 0;
    loop {
        critical_section::with(|cs| {
            if BTN_PRESSED.borrow(cs).get() {
                BTN_PRESSED.borrow(cs).set(false);
                count += 1;
                warn!("The button has been pressed {} times", count);
            }

            if PERIOD_ELAPSED.borrow(cs).get() {
                PERIOD_ELAPSED.borrow(cs).set(false);
                warn!("Periodic Timer period elapsed!");
                led.toggle();
                wdt_timer.feed();
                info!("Watchdog feeded");
            }
        });
    }
}
