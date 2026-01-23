use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::PubSubChannel;
use log::info;

use crate::button::BUTTON_READY_SIGNAL;
/// System ready pub-sub topic channel
/// 1 total capacity/messages, 4 subscribers, and 1 publisher
pub static SYSTEM_READY_PUBSUB_CHANNEL: PubSubChannel<CriticalSectionRawMutex, (), 1, 4, 1> =
    PubSubChannel::new();

/// Async task - Waiting for all subsystems to report back as ready
#[embassy_executor::task]
pub async fn wait_for_system_ready() {
    info!("Running System ready async task ...");
    info!("Waiting for system to be ready ...");

    // Waiting for each system component to report back as ready
    BUTTON_READY_SIGNAL.wait().await;
    // ... Add other ready signal here ...

    // Signal out that the system is ready
    SYSTEM_READY_PUBSUB_CHANNEL
        .publisher()
        .expect("Manager: Failed to publish to channel!")
        .publish(())
        .await;
    info!(">>>>>>>>>  ALL SYSTEMS GO! BIG BUTTON IS READY!  <<<<<<<<<");
}
