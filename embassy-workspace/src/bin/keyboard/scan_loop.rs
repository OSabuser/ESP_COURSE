//! Keyboard module - Task manager module
use super::core::Keyboard;
use crate::{keyboard, manager::SYSTEM_READY_PUBSUB_CHANNEL};
use embassy_futures::select::select;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use jiff::civil::Time;
use log::info;

#[embassy_executor::task]
pub async fn start_keyboard_scan(mut keyboard: super::core::Keyboard<'static>) {
    info!("Running Keyboard scan async task ...");
    //TODO: (1) connect with manager
    loop {
        keyboard.scan();
        let key_set = keyboard.pressed_keys();
        if !key_set.is_none() {
            for &(key_name, key) in super::core::Key::flags() {
                if key_set.contains(key) {
                    info!("Key {key_name} is pressed");
                }
            }
        }
        //TODO: (2) assemble word and send to display task
        keyboard.clear_pressed_keys();
        Timer::after(Duration::from_millis(10)).await;
    }
}
