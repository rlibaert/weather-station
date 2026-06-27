#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use bme280::i2c::BME280;
use embedded_graphics::Drawable;
use esp_hal::time::Rate;
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup,
};
use esp_hal::{i2c, spi};
use esp_println::println;
use log::{debug, error, info};

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
#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    // generator version: 1.3.0
    // generator parameters: --chip esp32c3 -o unstable-hal -o alloc -o ci -o zed

    esp_println::logger::init_logger(log::LevelFilter::Debug);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let _peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let timg0 = TimerGroup::new(_peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(_peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let i2c = {
        info!("initializing I2C...");
        let config = i2c::master::Config::default();
        i2c::master::I2c::new(_peripherals.I2C0, config)
            .unwrap()
            .with_sda(_peripherals.GPIO6)
            .with_scl(_peripherals.GPIO7)
    };

    let spi = {
        info!("initializing SPI...");
        let config = spi::master::Config::default()
            .with_frequency(Rate::from_khz(100))
            .with_mode(spi::Mode::_0);
        spi::master::Spi::new(_peripherals.SPI2, config)
            .inspect_err(|e| error!("Error while creating SPI: {e}"))
            .unwrap()
            .with_sck(_peripherals.GPIO8)
            .with_mosi(_peripherals.GPIO10)
    };

    let mut driver = {
        use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

        let busy = Input::new(
            _peripherals.GPIO2,
            InputConfig::default().with_pull(Pull::Up),
        );
        let res = Output::new(_peripherals.GPIO3, Level::High, OutputConfig::default());
        let dc = Output::new(_peripherals.GPIO4, Level::Low, OutputConfig::default());
        let cs = Output::new(_peripherals.GPIO5, Level::High, OutputConfig::default());

        let device = embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs, embassy_time::Delay)
            .inspect_err(|e| error!("Error creating exclusive spi device: {e}"))
            .unwrap();
        let iface = display_interface_spi::SPIInterface::new(device, dc);

        weact_studio_epd::WeActStudio213BlackWhiteDriver::new(iface, busy, res, embassy_time::Delay)
    };
    driver.init().unwrap();

    let mut display = weact_studio_epd::graphics::Display213BlackWhite::new();
    display.set_rotation(weact_studio_epd::graphics::DisplayRotation::Rotate90);

    let style = embedded_graphics::mono_font::MonoTextStyle::new(
        &profont::PROFONT_24_POINT,
        weact_studio_epd::Color::Black,
    );
    let _ = embedded_graphics::text::Text::with_text_style(
        "Hello World!",
        embedded_graphics::geometry::Point::new(8, 68),
        style,
        embedded_graphics::text::TextStyle::default(),
    )
    .draw(&mut display);

    driver.full_update(&display).unwrap();

    spawner.spawn(task_bme280(i2c).unwrap());
    spawner.spawn(task_dummy().unwrap());
}

#[embassy_executor::task]
async fn task_bme280(i2c: i2c::master::I2c<'static, esp_hal::Blocking>) {
    let mut delay = embassy_time::Delay;
    let mut bme280 = {
        info!("initializing BME280...");
        const ADDRESS: u8 = 0x76; // SDO connected to GND
        let mut bme280 = BME280::new(i2c, ADDRESS);
        bme280.init(&mut delay).unwrap();
        bme280
    };

    info!("starting loop...");
    loop {
        debug!("reading sensor...");

        let measurements = bme280.measure(&mut delay).unwrap();
        println!(
            "[{:.1} °C] [{:.1} %] [{:.1} hPa]",
            measurements.temperature,
            measurements.humidity,
            measurements.pressure / 100.0
        );

        embassy_time::Timer::after_millis(5_000).await;
    }
}

#[embassy_executor::task]
async fn task_dummy() {
    loop {
        info!("Hello world from embassy!");
        embassy_time::Timer::after_millis(10_000).await;
    }
}
