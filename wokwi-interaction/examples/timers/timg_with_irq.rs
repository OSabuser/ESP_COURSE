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
use esp_hal::gpio::{Event, Input, InputConfig, Io, Level, Output, OutputConfig};

use esp_hal::time::Duration;
use esp_hal::timer::Timer;
use esp_hal::timer::timg::Timer as TimgTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{handler, main};
use log::{info, warn};

static BUTTON: Mutex<RefCell<Option<Input>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<TimgTimer>>> = Mutex::new(RefCell::new(None));
static PERIOD_ELAPSED: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));
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

#[handler]
fn timer0_irq_handler() {
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
    let timer0: TimgTimer = timg0.timer0;

    timer0.load_value(Duration::from_millis(1000)).unwrap();
    timer0.set_interrupt_handler(timer0_irq_handler);
    timer0.enable_interrupt(true);
    timer0.enable_auto_reload(false);
    let mut now = timer0.now();
    timer0.start();
    critical_section::with(|cs| TIMER.borrow_ref_mut(cs).replace(timer0));

    let timings = [900, 800, 700, 600, 500, 400];
    let mut timing_cnt = 0;
    let mut count = 0;

    info!("Main thread has started...");

    loop {
        critical_section::with(|cs| {
            if BTN_PRESSED.borrow(cs).get() {
                BTN_PRESSED.borrow(cs).set(false);
                count += 1;
                warn!("The button has been pressed {} times", count);
            }

            if PERIOD_ELAPSED.borrow(cs).get() {
                PERIOD_ELAPSED.borrow(cs).set(false);

                let elapsed = TIMER.borrow_ref_mut(cs).as_mut().unwrap().now() - now;
                warn!("Period {} ms has elapsed", elapsed.as_millis());

                // Загрузка нового значения переполнения
                TIMER
                    .borrow_ref_mut(cs)
                    .as_mut()
                    .unwrap()
                    .load_value(Duration::from_millis(timings[timing_cnt]))
                    .unwrap();
                TIMER.borrow_ref_mut(cs).as_mut().unwrap().start();

                timing_cnt = (timing_cnt + 1) % timings.len();

                now = TIMER.borrow_ref_mut(cs).as_mut().unwrap().now();

                led.toggle();
            }
        });
    }
}
