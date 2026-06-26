#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use bme280::i2c::BME280;
use esp_hal::{
    clock::CpuClock,
    i2c, main,
    time::{Duration, Instant},
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32c3 -o unstable-hal -o alloc -o ci -o zed

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let _peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    esp_println::println!("initializing I2C...");

    let i2c = i2c::master::I2c::new(_peripherals.I2C0, i2c::master::Config::default())
        .unwrap()
        .with_sda(_peripherals.GPIO6)
        .with_scl(_peripherals.GPIO7);

    const BME280_I2C_ADDR: u8 = 0x76; // SDO connected to GND
    let mut bme280 = BME280::new(i2c, BME280_I2C_ADDR);
    let mut delay = esp_hal::delay::Delay::new();

    esp_println::println!("initializing BME280...");
    bme280.init(&mut delay).unwrap();

    esp_println::println!("starting loop...");
    loop {
        esp_println::println!("trying to read from BME280...");

        let measurements = bme280.measure(&mut delay).unwrap();
        esp_println::println!("T: {} °C", measurements.temperature);
        esp_println::println!("H: {} %", measurements.humidity);
        esp_println::println!("P: {} Pa", measurements.pressure);
        esp_println::println!("");

        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(2000) {}
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}
