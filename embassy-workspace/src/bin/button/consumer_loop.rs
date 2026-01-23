//! Button module - Task manager module
use super::core::UserButton;
use super::messaging::BUTTON_READY_SIGNAL;
use super::utility;
use crate::manager::SYSTEM_READY_PUBSUB_CHANNEL;
use embassy_futures::select::select;
use esp_hal::gpio::Input;
use log::info;

/// Async task - Button
/// Continuously monitor and report button press/release events
///
/// * `button_info` - Button information vector
#[embassy_executor::task]
pub async fn start_button_monitor(button_info: [(u8, Input<'static>); 1]) {
    let mut button = UserButton::new(button_info).expect("Failed to init UserButton!");
    info!("Running Button monitor async task ...");

    // Signal to system that button is ready to be used
    BUTTON_READY_SIGNAL.signal(true);

    // Wait idle until the system manager sends a ready signal
    let mut system_ready_message = SYSTEM_READY_PUBSUB_CHANNEL
        .subscriber()
        .expect("Button: Failed to subscribe to channel!");
    select(
        system_ready_message.next_message_pure(),
        utility::do_nothing_idle(),
    )
    .await;

    button.monitor_press(0).await;
}
