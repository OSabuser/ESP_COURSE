#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};

use embedded_sdmmc::{TimeSource, Timestamp};
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

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

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
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(4096, 4096);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let sclk = peripherals.GPIO40; // SCK
    let mosi = peripherals.GPIO14; // MOSI
    let miso = peripherals.GPIO39; // MISO

    // Initialize SPI
    // Create SPI with DMA
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(esp_hal::time::Rate::from_khz(400))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_miso(miso)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default()); // CS

    let spi_dev =
        embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs, embassy_time::Delay).unwrap();

    let sdcard = embedded_sdmmc::SdCard::new(spi_dev, embassy_time::Delay);

    info!("Init SD card controller and retrieve card size...");
    let sd_size = sdcard.num_bytes().unwrap();
    info!("card size is {} bytes\r\n", sd_size);

    // Now let's look for volumes (also known as partitions) on our block device.
    // To do this we need a Volume Manager. It will take ownership of the block device.
    let volume_mgr = embedded_sdmmc::VolumeManager::new(sdcard, DummyTimesource::default());

    let volume0 = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();

    let root_dir = volume0.open_root_dir().unwrap();

    let mut my_file = root_dir
        .open_file_in_dir("TEST.TXT", embedded_sdmmc::Mode::ReadOnly)
        .unwrap();

    while !my_file.is_eof() {
        let mut buffer = [0u8; 32];

        if let Ok(n) = my_file.read(&mut buffer) {
            for b in &buffer[..n] {
                error!("{}", *b as char);
            }
        }
    }

    let user_btn = UserButton {
        input: Input::new(
            peripherals.GPIO0,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        ),
        state: ButtonState::Released,
    };

    spawner.spawn(button_read_task(user_btn)).unwrap();
    spawner.spawn(heartbeat_task()).unwrap();

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

#[embassy_executor::task]
async fn heartbeat_task() {
    loop {
        info!("We are alive");
        Timer::after(Duration::from_millis(2000)).await;
    }
}

#[derive(Default)]
pub struct DummyTimesource();

impl TimeSource for DummyTimesource {
    // In theory you could use the RTC of the rp2040 here, if you had
    // any external time synchronizing device.
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}
