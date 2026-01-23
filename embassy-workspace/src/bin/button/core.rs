//! Button module

use crate::AppConfig;
use crate::button::messaging::{BUTTON_PUBSUB_CHANNEL, ButtonMessage, PressType};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{Error, Publisher};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::gpio::Input;
use log::{debug, error, info, warn};

// Custom type aliases
type ItemId = u8;
type ItemHandle = Input<'static>;
type ItemInfo = [(ItemId, ItemHandle); 1];

/// Handle for individual Buttons - Button has all provisioned buttons defined
pub struct UserButton<'a> {
    /// Button definitions - `[(<BUTTON ID>, <`[Input]` HANDLE>), ...]`
    pub item_info: ItemInfo,
    /// Button pub-sub publisher for message publishing
    pub button_pubsub_publisher: Publisher<'a, CriticalSectionRawMutex, ButtonMessage, 2, 3, 1>,
}

impl UserButton<'_> {
    /// Button constructor
    pub fn new(item_info: ItemInfo) -> Result<Self, Error> {
        let publisher = BUTTON_PUBSUB_CHANNEL.publisher()?;

        Ok(Self {
            item_info,
            button_pubsub_publisher: publisher,
        })
    }

    /// Get all IDs
    pub fn get_ids(&self) -> [u8; 1] {
        let mut ids = [0; 1];
        for (index, (id, _)) in self.item_info.iter().enumerate() {
            if let Some(number) = ids.get_mut(index) {
                *number = *id;
            }
        }
        ids
    }

    /// Check if specified IDs are valid and within the pre-defined item info
    ///
    /// * `ids` - Item IDs to check. Empty ids array will return `true`
    pub fn check_ids(&self, ids: &[u8]) -> bool {
        if ids.is_empty() {
            return true;
        }
        for id in ids {
            if !self.get_ids().contains(id) {
                return false;
            }
        }
        true
    }

    /// Debouncing button press - GPIO Level HIGH to LOW
    /// Debouncing is the process of removing noise from a button press signal.
    /// Returns when the button press signal is stable.
    ///
    /// * `id` - Button ID number
    async fn debounce_high_to_low(&mut self, id: u8) {
        let item = self
            .item_info
            .iter_mut()
            .find(|(button_id, _)| *button_id == id);

        if let Some((_, handle)) = item {
            loop {
                let pin_level_1 = handle.level();
                handle.wait_for_low().await;
                Timer::after_millis(20).await;
                let pin_level_2 = handle.level();
                if pin_level_1 != pin_level_2 && handle.is_low() {
                    break;
                }
            }
        } else {
            warn!("Button with ID {id} not found");
        }
    }

    /// Debouncing button press - GPIO Level LOW to HIGH
    /// Debouncing is the process of removing noise from a button press signal.
    /// Returns when the button press signal is stable.
    ///
    /// * `id` - Button ID number
    async fn debounce_low_to_high(&mut self, id: u8) {
        let item = self
            .item_info
            .iter_mut()
            .find(|(button_id, _)| *button_id == id);

        if let Some((_, handle)) = item {
            loop {
                let pin_level_1 = handle.level();
                handle.wait_for_high().await;
                Timer::after_millis(20).await;
                let pin_level_2 = handle.level();
                if pin_level_1 != pin_level_2 && handle.is_high() {
                    break;
                }
            }
        } else {
            warn!("Button with ID {id} not found");
        }
    }

    /// Continuously watch specified button edge state and report button press.
    ///
    /// Button press patterns can be the following:
    ///
    ///   - Short Press - Released before long press threshold
    ///   - Long Press - Released after long press threshold
    ///   - Long Hold - Held more than long hold threshold
    ///
    /// * `id` - Button ID number
    pub async fn monitor_press(&mut self, id: u8) -> () {
        if !self.check_ids(&[id]) {
            error!("Failed to find specified Button ID in the pre-defined Button info");
            return;
        }

        let button_long_press_release_threshold =
            Duration::from_millis(AppConfig::BTN_LONG_PRESS_THRESHOLD_MS.into());
        let button_long_press_hold_threshold =
            Duration::from_millis(AppConfig::BTN_LONG_HOLD_THRESHOLD_MS.into());

        let mut button_down_press_timestamp: Instant;
        let mut button_up_release_timestamp: Instant;

        warn!("Monitoring Button ID: {id}");

        loop {
            // Wait for button down press
            self.debounce_high_to_low(id).await;

            button_down_press_timestamp = Instant::now();
            warn!(
                "Button ID {} down pressed! - Timestamp: {:?}ms",
                id,
                button_down_press_timestamp.as_millis()
            );
            // Long press hold timer
            let long_press_hold_future = Timer::after(button_long_press_hold_threshold);

            // Wait for either button release OR long press timeout
            match select(self.debounce_low_to_high(id), long_press_hold_future).await {
                Either::First(()) => {
                    // Button released before long hold timeout
                    button_up_release_timestamp = Instant::now();
                    let release_time =
                        button_up_release_timestamp.duration_since(button_down_press_timestamp);
                    warn!(
                        "Button ID {} up released! - Timestamp: {:?}ms -> Time Difference: {:?}ms",
                        id,
                        button_up_release_timestamp.as_millis(),
                        release_time.as_millis(),
                    );

                    if release_time < button_long_press_release_threshold {
                        // PRESS: SHORT PRESS - released before long timeout
                        warn!(
                            "Button ID {} - Press type: SHORT RELEASE (< {}ms)",
                            id,
                            button_long_press_release_threshold.as_millis()
                        );
                        self.button_pubsub_publisher
                            .publish(ButtonMessage {
                                id,
                                timestamp_start: button_down_press_timestamp,
                                timestamp_end: button_up_release_timestamp,
                                press_type: PressType::ShortRelease,
                            })
                            .await;
                    } else {
                        // PRESS: LONG PRESS - released after long press threshold
                        warn!(
                            "Button ID {} - Press type: LONG RELEASE (>= {}ms)",
                            id,
                            button_long_press_release_threshold.as_millis()
                        );
                        self.button_pubsub_publisher
                            .publish(ButtonMessage {
                                id,
                                timestamp_start: button_down_press_timestamp,
                                timestamp_end: button_up_release_timestamp,
                                press_type: PressType::LongRelease,
                            })
                            .await;
                    }
                }
                Either::Second(()) => {
                    // PRESS: LONG HOLD - no release detected before long hold threshold
                    warn!(
                        "Button ID {} - Press type: LONG HOLD (>= {}ms)",
                        id,
                        button_long_press_hold_threshold.as_millis()
                    );
                    self.button_pubsub_publisher
                        .publish(ButtonMessage {
                            id,
                            timestamp_start: button_down_press_timestamp,
                            timestamp_end: Instant::now(),
                            press_type: PressType::LongHold,
                        })
                        .await;
                }
            }
        }
    }
}
