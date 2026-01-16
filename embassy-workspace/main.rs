#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;

use embassy_time::{Duration, Timer};
use embedded_graphics::primitives::Rectangle;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::delay;
use esp_hal::delay::Delay;
use esp_hal::gpio::Output;
use esp_hal::gpio::OutputConfig;
use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

use esp_hal::spi::master::Spi;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};

// Provides the Display builder
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ST7789,
    options::{ColorInversion, Orientation},
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

/// Display width
const DISPLAY_SIZE_WIDTH: u16 = 240;
/// Display height
const DISPLAY_SIZE_HEIGHT: u16 = 135;

const DISPLAY_BUFFER_SIZE: usize = DISPLAY_SIZE_WIDTH as usize * DISPLAY_SIZE_HEIGHT as usize * 2; // sizeof(Rgb565)*W*H

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    // Initialize SPI
    let spi_bus = Spi::new(
        peripherals.SPI2,
        esp_hal::spi::master::Config::default().with_frequency(esp_hal::time::Rate::from_mhz(26)),
    )
    .unwrap()
    .with_sck(peripherals.GPIO36)
    .with_mosi(peripherals.GPIO35);

    let display_cs = Output::new(
        peripherals.GPIO37,
        esp_hal::gpio::Level::Low,
        OutputConfig::default(),
    );
    //

    let spi_dev = ExclusiveDevice::new(spi_bus, display_cs, Delay::new()).unwrap();

    let display_rs = Output::new(
        peripherals.GPIO34,
        esp_hal::gpio::Level::Low,
        OutputConfig::default(),
    );
    let display_rst = Output::new(
        peripherals.GPIO33,
        esp_hal::gpio::Level::High,
        OutputConfig::default(),
    );
    let mut display_bl = Output::new(
        peripherals.GPIO38,
        esp_hal::gpio::Level::Low,
        OutputConfig::default(),
    );

    let mut framebuffer = [0_u8; DISPLAY_BUFFER_SIZE];

    // Define the display interface with no chip select
    let di = SpiInterface::new(spi_dev, display_rs, &mut framebuffer);

    let mut display = Builder::new(ST7789, di)
        .invert_colors(ColorInversion::Inverted)
        .display_size(DISPLAY_SIZE_WIDTH, DISPLAY_SIZE_HEIGHT)
        .display_offset(0, 100)
        .reset_pin(display_rst)
        .init(&mut Delay::new())
        .expect("Unable to initialize display!");

    display
        .set_orientation(Orientation::new().rotate(mipidsi::options::Rotation::Deg0))
        .expect("Unable to set orientation!");

    display
        .set_vertical_scroll_offset(0)
        .expect("Unable to set vertical offset!");

    let delay = Delay::new();
    // Make the display all black
    display.clear(Rgb565::BLACK).unwrap();

    delay.delay_millis(1500);
    display_bl.set_high();
    delay.delay_millis(1500);
    display.fill_solid(
        &Rectangle::new(
            Point::new(52, 40), // Центрируйте в 240x320
            Size::new(135, 240),
        ),
        Rgb565::WHITE,
    );

    delay.delay_millis(1500);

    spawner.spawn(heartbeat_task()).unwrap();

    loop {
        info!("Message sent from main");
        Timer::after(Duration::from_millis(1500)).await;
    }
}

#[embassy_executor::task]
async fn heartbeat_task() {
    loop {
        info!("We are alive");
        Timer::after(Duration::from_millis(1250)).await;
    }
}
