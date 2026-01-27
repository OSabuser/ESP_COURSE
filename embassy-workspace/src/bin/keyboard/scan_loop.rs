//! Keyboard module - Task manager module
use super::core::Keyboard;
use crate::{
    keyboard::{
        self,
        keys::{KeyboardState, SpecialKey},
    },
    manager::SYSTEM_READY_PUBSUB_CHANNEL,
};
use embassy_futures::select::select;
use embassy_time::{Duration, Timer};

use heapless::String;
use log::{error, info};

#[embassy_executor::task]
pub async fn start_keyboard_scan(mut keyboard: super::core::Keyboard<'static>) {
    info!("Running Keyboard scan async task ...");
    let mut keymap = KeyboardState::new();
    let mut text_buffer: String<256> = String::new();

    //TODO: (1) connect with manager
    loop {
        keyboard.scan();
        let key_set = keyboard.pressed_keys();

        keymap.update_modifiers(key_set);

        match keymap.handle_special_key(key_set) {
            Some(SpecialKey::Enter) => {
                // Вывод текста, очистка строки
                error!("Text: {text_buffer}");
                text_buffer.clear();
            }
            Some(SpecialKey::Tab) => {
                // Добавление табуляции
                text_buffer.push('\t').ok();
            }
            Some(SpecialKey::Backspace) => {
                // Удаление символа из строки
                text_buffer.pop();
            }
            None => {
                // Добавление символа в строку
                if let Some(ch) = keymap.key_to_char(key_set) {
                    text_buffer.push(ch).ok();
                }
            }
        }

        //TODO: (2) assemble word and send to display task
        keyboard.clear_pressed_keys();
        Timer::after(Duration::from_millis(10)).await;
    }
}
