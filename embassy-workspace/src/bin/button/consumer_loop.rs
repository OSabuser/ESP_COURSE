//! Button module - Task manager module
use esp_hal::gpio::Input;
use log::info;

use crate::button::core::UserButton;

/// Async task - Button
/// Continuously monitor and report button press/release events
///
/// * `button_info` - Button information vector
#[embassy_executor::task]
pub async fn start_button_monitor(button_info: [(u8, Input<'static>); 1]) {
    let mut button = UserButton::new(button_info).expect("Failed to init UserButton!");
    info!("Running Button monitor async task ...");
    button.monitor_press(0).await;
}
