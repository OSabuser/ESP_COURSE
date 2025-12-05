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
use esp_hal::timer::PeriodicTimer;
use esp_hal::timer::timg::{MwdtStage, TimerGroup};
use esp_hal::uart::{Config, DataBits, RxConfig, Uart, UartInterrupt};
use esp_hal::{Blocking, handler, main};
use log::{error, info, warn};

struct MessageBuffer {
    message: [u8; 64],
    length: usize,
}
impl MessageBuffer {
    fn new() -> Self {
        Self {
            message: [0; 64],
            length: 0,
        }
    }
}
static BUTTON: Mutex<RefCell<Option<Input>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<PeriodicTimer<'_, Blocking>>>> = Mutex::new(RefCell::new(None));
static UART: Mutex<RefCell<Option<Uart<'_, Blocking>>>> = Mutex::new(RefCell::new(None));

static MSG_RECEIVED: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));
static MSG_BUFFER: Mutex<RefCell<Option<MessageBuffer>>> = Mutex::new(RefCell::new(None));
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
fn gpt_irq_handler() {
    critical_section::with(|cs| {
        // Очистка флага прерывания
        TIMER.borrow_ref_mut(cs).as_mut().unwrap().clear_interrupt();

        // Установка флага
        PERIOD_ELAPSED.borrow(cs).set(true);
    });
}

#[handler]
fn uart_rx_irq_handler() {
    critical_section::with(|cs| {
        // Очистка флага прерывания
        let mut serial = UART.borrow_ref_mut(cs);

        if let Some(serial) = serial.as_mut() {
            let mut buf = [0u8; 64];
            if let Ok(cnt) = serial.read_buffered(&mut buf) {
                let mut buffer = MSG_BUFFER.borrow_ref_mut(cs);
                buffer.as_mut().unwrap().length = cnt;
                for i in 0..cnt {
                    buffer.as_mut().unwrap().message[i] = buf[i];
                }
                // Установка флага
                MSG_RECEIVED.borrow(cs).set(true);
            }

            serial.clear_interrupts(UartInterrupt::RxTimeout | UartInterrupt::RxFifoFull);
        }
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
    let mut periodic_timer = PeriodicTimer::new(timg0.timer0);

    let mut wdt_timer = timg0.wdt;
    wdt_timer.set_timeout(MwdtStage::Stage0, Duration::from_millis(5_000));
    wdt_timer.enable();

    periodic_timer.set_interrupt_handler(gpt_irq_handler);
    periodic_timer.listen();

    // Запуск таймера
    if let Err(e) = periodic_timer.start(esp_hal::time::Duration::from_millis(1000)) {
        error!("Failed to start timer: {:?}", e);
    }
    critical_section::with(|cs| TIMER.borrow_ref_mut(cs).replace(periodic_timer));

    // Инициализация UART1 [UART0 занят ESP-IDF]
    // При превышении
    let config = Config::default().with_rx(
        RxConfig::default()
            .with_fifo_full_threshold(64)
            .with_timeout(10),
    );
    let mut serial_instance = Uart::new(
        peripherals.UART1,
        config.with_baudrate(9600).with_data_bits(DataBits::_8),
    )
    .unwrap()
    .with_rx(peripherals.GPIO20)
    .with_tx(peripherals.GPIO21);

    serial_instance.set_interrupt_handler(uart_rx_irq_handler);

    let msg_instance = MessageBuffer::new();

    critical_section::with(|cs| {
        serial_instance.listen(UartInterrupt::RxTimeout | UartInterrupt::RxFifoFull);
        UART.borrow_ref_mut(cs).replace(serial_instance);
        MSG_BUFFER.borrow_ref_mut(cs).replace(msg_instance);
    });

    let mut count = 0;

    info!("Main thread has started...");
    loop {
        critical_section::with(|cs| {
            if MSG_RECEIVED.borrow(cs).get() {
                MSG_RECEIVED.borrow(cs).set(false);
                let buffer = MSG_BUFFER.borrow_ref_mut(cs);
                info!(
                    "Received message: {:?}",
                    core::str::from_utf8(
                        &buffer.as_ref().unwrap().message[0..buffer.as_ref().unwrap().length]
                    )
                );
            }
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
