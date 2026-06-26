# Weather Station

> [!IMPORTANT]
> This is a toy project for me to learn Rust and embedded development and it's still under heavy development.

Firmware code for a weather station based on an ESP32-C3 microcontroller and BME280 sensor.

## Getting Started

### Connection diagram

```mermaid
flowchart LR

subgraph ESP32[Seeed Studio ESP32-C3]
  esp_sda[I2C_SDA]
  esp_scl[I2C_SCL]
  esp_3v3[3V3]
  esp_gnd[GND]
end

subgraph GY-BME280
  bme_vcc[VCC]
  bme_gnd[GND]
  bme_scl[SCL]
  bme_sda[SDA]
  bme_vcc --- CSB
  bme_gnd --- SDO
end

esp_sda --- bme_sda
esp_scl --- bme_scl
esp_3v3 --- bme_vcc
esp_gnd --- bme_gnd
```

### Run the code

```bash
$ cargo run --release
initializing I2C...
initializing BME280...
starting loop...
trying to read from BME280...
T: 32.658386 °C
H: 47.545048 %
P: 101351.7 Pa

trying to read from BME280...
T: 32.65711 °C
H: 47.45209 %
P: 101351.7 Pa
```
