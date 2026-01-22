#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use embedded_graphics::primitives::Circle;
use esp_hal::clock::CpuClock;

use esp_hal::Async;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::{Input, Level, Output, OutputConfig};
use esp_hal::spi::{
    Mode,
    master::{Config, Spi, SpiDmaBus},
};
use esp_hal::timer::timg::TimerGroup;
use log::{error, info, warn};

// This crate's framebuffer and async display interface
use lcd_async::{
    Builder, interface,
    models::ST7789,
    options::{ColorInversion, Orientation, Rotation},
    raw_framebuf::RawFrameBuf,
};

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::mutex::Mutex;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Primitive, PrimitiveStyle},
};

use static_cell::StaticCell;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

enum ButtonState {
    Pressed,
    Released,
}

struct UserButton {
    input: Input<'static>,
    state: ButtonState,
}

static BUTTON_PRESSED: Signal<CriticalSectionRawMutex, ButtonState> = Signal::new();

esp_bootloader_esp_idf::esp_app_desc!();

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
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let sclk = peripherals.GPIO36; // SCK
    let mosi = peripherals.GPIO35; // MOSI

    // Initialize SPI
    // Create SPI with DMA
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(60))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    // Create control pins
    let res = Output::new(peripherals.GPIO33, Level::Low, Default::default());
    let dc = Output::new(peripherals.GPIO34, Level::Low, Default::default());
    let cs = Output::new(peripherals.GPIO37, Level::High, Default::default());
    let display_bl = Output::new(peripherals.GPIO38, Level::Low, OutputConfig::default());

    // Create shared SPI bus
    static SPI_BUS: StaticCell<Mutex<NoopRawMutex, SpiDmaBus<'static, Async>>> = StaticCell::new();
    let spi_bus = Mutex::new(spi);
    let spi_bus = SPI_BUS.init(spi_bus);

    let spi_device: SpiDevice<'static, NoopRawMutex, SpiDmaBus<'static, Async>, Output<'static>> =
        SpiDevice::new(spi_bus, cs);

    let user_btn = UserButton {
        input: Input::new(
            peripherals.GPIO0,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        ),
        state: ButtonState::Released,
    };

    spawner.spawn(button_read_task(user_btn)).unwrap();
    spawner.spawn(heartbeat_task()).unwrap();
    spawner
        .spawn(drawing_task(spi_device, dc, res, display_bl))
        .unwrap();

    loop {
        let command = BUTTON_PRESSED.wait().await;

        match command {
            ButtonState::Pressed => {
                error!("Button has been pressed!");
            }
            ButtonState::Released => {
                error!("Button has been released!");
            }
        }
    }
}

#[embassy_executor::task]
async fn button_read_task(mut button: UserButton) {
    loop {
        button.input.wait_for_falling_edge().await;
        button.state = ButtonState::Pressed;
        BUTTON_PRESSED.signal(button.state);
        button.input.wait_for_rising_edge().await;
        button.state = ButtonState::Released;
        BUTTON_PRESSED.signal(button.state);
        Timer::after(Duration::from_millis(250)).await;
    }
}

static HEARTBEAT_OCCURED: Signal<CriticalSectionRawMutex, HeartbeatState> = Signal::new();

#[derive(Copy, Clone)]
enum HeartbeatState {
    On,
    Off,
}
#[embassy_executor::task]
async fn heartbeat_task() {
    let mut state = HeartbeatState::Off;
    loop {
        info!("We are alive");
        HEARTBEAT_OCCURED.signal(state.clone());
        state = match state {
            HeartbeatState::On => HeartbeatState::Off,
            HeartbeatState::Off => HeartbeatState::On,
        };
        Timer::after(Duration::from_millis(2000)).await;
    }
}

#[embassy_executor::task]
async fn drawing_task(
    spi_device: SpiDevice<'static, NoopRawMutex, SpiDmaBus<'static, Async>, Output<'static>>,
    data_pin: Output<'static>,
    reset_pin: Output<'static>,
    mut backlight_pin: Output<'static>,
) {
    // Display parameters
    const WIDTH: u16 = 240;
    const HEIGHT: u16 = 135;
    const PIXEL_SIZE: usize = 2; // RGB565 = 2 bytes per pixel
    const FRAME_SIZE: usize = (WIDTH as usize) * (HEIGHT as usize) * PIXEL_SIZE;

    static FRAME_BUFFER: StaticCell<[u8; FRAME_SIZE]> = StaticCell::new();

    // Create display interface
    let di = interface::SpiInterface::new(spi_device, data_pin);
    let mut delay = embassy_time::Delay;

    // Initialize the display
    let mut oled_display: lcd_async::Display<
        interface::SpiInterface<
            SpiDevice<'_, NoopRawMutex, SpiDmaBus<'_, Async>, Output<'_>>,
            Output<'_>,
        >,
        ST7789,
        Output<'_>,
    > = Builder::new(ST7789, di)
        .reset_pin(reset_pin)
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

    // Initialize framebuffer
    let frame_buffer = FRAME_BUFFER.init_with(|| [0; FRAME_SIZE]);

    // Create a framebuffer for drawing
    let mut raw_fb =
        RawFrameBuf::<Rgb565, _>::new(frame_buffer.as_mut_slice(), WIDTH.into(), HEIGHT.into());

    // Clear the framebuffer to black
    raw_fb.clear(Rgb565::BLACK).unwrap();

    // Turn on the backlight
    backlight_pin.set_high();

    // Create a new character style
    let style = embedded_graphics::mono_font::MonoTextStyle::new(
        &embedded_graphics::mono_font::ascii::FONT_10X20,
        Rgb565::WHITE,
    );

    embedded_graphics::text::Text::new("In an Async World!", Point::new(0, 125), style)
        .draw(&mut raw_fb)
        .unwrap();

    loop {
        let state = HEARTBEAT_OCCURED.wait().await;
        // Create a framebuffer for drawing
        let mut raw_fb =
            RawFrameBuf::<Rgb565, _>::new(frame_buffer.as_mut_slice(), WIDTH.into(), HEIGHT.into());

        // Toggle heartbeat
        toggle_heartbeat(&mut raw_fb, state);

        // Send the framebuffer data to the display
        if let Err(e) = oled_display
            .show_raw_data(0, 0, WIDTH, HEIGHT, frame_buffer)
            .await
        {
            error!("Ошибка обновления фреймбуфера: {:?}", e);
        }

        Timer::after(Duration::from_millis(1000)).await;
    }
}

fn toggle_heartbeat(fb: &mut RawFrameBuf<Rgb565, &mut [u8]>, state: HeartbeatState) {
    let color = match state {
        HeartbeatState::On => Rgb565::GREEN,
        HeartbeatState::Off => Rgb565::BLACK,
    };

    if let Err(e) = Circle::new(Point::new(210, 10), 20)
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(fb)
    {
        error!("Ошибка отрисовки: {:?}", e);
    }
}
