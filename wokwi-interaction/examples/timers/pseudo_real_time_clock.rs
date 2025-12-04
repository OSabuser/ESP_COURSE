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

struct CurrentTime {
    hours: u64,
    minutes: u64,
    seconds: u64,
}

impl Default for CurrentTime {
    fn default() -> Self {
        Self {
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

impl CurrentTime {
    fn set_time(&mut self, duration: Duration) {
        self.hours = duration.as_hours() % 24;
        self.minutes = duration.as_minutes() % 60;
        self.seconds = duration.as_secs() % 60;
    }
}

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

    timer0.load_value(Duration::from_secs(1)).unwrap();
    timer0.set_interrupt_handler(timer0_irq_handler);
    timer0.enable_interrupt(true);
    timer0.enable_auto_reload(true);
    timer0.start();
    let initial_time_stamp = timer0.now();
    critical_section::with(|cs| TIMER.borrow_ref_mut(cs).replace(timer0));

    let mut btn_pressed_cnt = 0;
    let mut system_time = CurrentTime::default();

    info!("Main thread has started...");

    loop {
        critical_section::with(|cs| {
            if BTN_PRESSED.borrow(cs).get() {
                BTN_PRESSED.borrow(cs).set(false);
                btn_pressed_cnt += 1;
                warn!("The button has been pressed {} times", btn_pressed_cnt);
            }

            if PERIOD_ELAPSED.borrow(cs).get() {
                PERIOD_ELAPSED.borrow(cs).set(false);

                let duration = initial_time_stamp.elapsed();

                system_time.set_time(duration);

                info!(
                    "Current time: {:0>2}:{:0>2}:{:0>2}",
                    system_time.hours, system_time.minutes, system_time.seconds
                );

                led.toggle();
            }
        });
    }
}
