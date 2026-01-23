use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::mutex::Mutex;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Primitive, PrimitiveStyle},
};
use esp_hal::Async;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::spi::{
    Mode,
    master::{Config, Spi, SpiDmaBus},
};
use static_cell::StaticCell;

//TODO: (1) basic initialization
