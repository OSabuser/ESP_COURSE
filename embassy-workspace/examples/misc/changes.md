# Чтение внешнего конфиг файла

- `build.rs` (функции для чтения внешнего конфига/ создание конфига по умолчанию)
- `src/bin/main.rs` (`include!` сгенерированного файла)
- `config.json` (конфигурационный файл)
- `cargo.toml` (serde_json, serde & anyhow)
  
```json
// config.json
{
  "device_name": "ESP32-MyDevice",
  "update_interval_ms": "1000",
  "max_retries": "3",
  "wifi_ssid": "MyNetwork",
  "wifi_password": "password123",
  "led_pin": "2",
  "log_level": "DEBUG"
}
```
