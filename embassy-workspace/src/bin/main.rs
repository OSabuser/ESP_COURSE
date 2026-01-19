#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::primitives::Rectangle;
use esp_hal::clock::CpuClock;

use esp_hal::gpio::Output;
use esp_hal::gpio::OutputConfig;
use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

// This crate's framebuffer and async display interface
use lcd_async::{
    Builder, interface,
    models::ST7789,
    options::{ColorInversion, Orientation, Rotation},
    raw_framebuf::RawFrameBuf,
};

use esp_hal::spi::master::Spi;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};
use static_cell::StaticCell;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

// Display parameters
const WIDTH: u16 = 240;
const HEIGHT: u16 = 135;
const PIXEL_SIZE: usize = 2; // RGB565 = 2 bytes per pixel
const FRAME_SIZE: usize = (WIDTH as usize) * (HEIGHT as usize) * PIXEL_SIZE;

static FRAME_BUFFER: StaticCell<[u8; FRAME_SIZE]> = StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    // Create DMA buffers for SPI
    #[allow(clippy::manual_div_ceil)]
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(4, 32_000);
    let dma_rx_buf = esp_hal::dma::DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = esp_hal::dma::DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let sclk = peripherals.GPIO36; // SCK
    let mosi = peripherals.GPIO35; // MOSI
    let res = peripherals.GPIO33; // RES (Reset)
    let dc = peripherals.GPIO34; // DC (Data/Command)
    let cs = peripherals.GPIO37; // CS (Chip Select)

    // Initialize SPI
    // Create SPI with DMA
    let spi = Spi::new(
        peripherals.SPI2,
        esp_hal::spi::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(60))
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let mut display_bl = Output::new(
        peripherals.GPIO38,
        esp_hal::gpio::Level::Low,
        OutputConfig::default(),
    );

    // Create control pins
    let res = Output::new(res, esp_hal::gpio::Level::Low, Default::default());
    let dc = Output::new(dc, esp_hal::gpio::Level::Low, Default::default());
    let cs = Output::new(cs, esp_hal::gpio::Level::High, Default::default());

    // Create shared SPI bus
    static SPI_BUS: StaticCell<
        embassy_sync::mutex::Mutex<
            embassy_sync::blocking_mutex::raw::NoopRawMutex,
            esp_hal::spi::master::SpiDmaBus<'static, esp_hal::Async>,
        >,
    > = StaticCell::new();
    let spi_bus = embassy_sync::mutex::Mutex::new(spi);
    let spi_bus = SPI_BUS.init(spi_bus);
    let spi_device = SpiDevice::new(spi_bus, cs);

    // Create display interface
    let di = interface::SpiInterface::new(spi_device, dc);
    let mut delay = embassy_time::Delay;

    // Initialize the display
    let mut display = Builder::new(ST7789, di)
        .reset_pin(res)
        .display_size(HEIGHT, WIDTH)
        .orientation(Orientation {
            rotation: Rotation::Deg90,
            mirrored: false,
        })
        .display_offset(52, 40)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .await
        .unwrap();

    // Initialize frame buffer
    let frame_buffer = FRAME_BUFFER.init_with(|| [0; FRAME_SIZE]);

    // Create a framebuffer for drawing
    let mut raw_fb =
        RawFrameBuf::<Rgb565, _>::new(frame_buffer.as_mut_slice(), WIDTH.into(), HEIGHT.into());

    // Clear the framebuffer to black
    raw_fb.clear(Rgb565::BLACK).unwrap();
    // Create a new character style
    let style = embedded_graphics::mono_font::MonoTextStyle::new(
        &embedded_graphics::mono_font::ascii::FONT_10X20,
        Rgb565::WHITE,
    );

    embedded_graphics::text::Text::new("Hello Rust!", Point::new(100, 50), style)
        .draw(&mut raw_fb)
        .unwrap();

    Rectangle::new(Point::new(0, 0), Size::new(75, 100))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::GREEN))
        .draw(&mut raw_fb)
        .unwrap();

    // Send the framebuffer data to the display
    display
        .show_raw_data(0, 0, WIDTH, HEIGHT, frame_buffer)
        .await
        .unwrap();

    display_bl.set_high();

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
