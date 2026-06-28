#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use bme280::i2c::BME280;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embedded_graphics::Drawable;
use esp_hal::time::Rate;
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup,
};
use esp_hal::{i2c, spi};
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
    info!("Starting");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let i2c = {
        debug!("Initializing I2C...");
        let config = i2c::master::Config::default();
        i2c::master::I2c::new(peripherals.I2C0, config)
            .unwrap()
            .with_sda(peripherals.GPIO6)
            .with_scl(peripherals.GPIO7)
    };

    let bme280 = {
        debug!("Initializing BME280...");
        const ADDRESS: u8 = 0x76; // SDO connected to GND
        let mut bme280 = BME280::new(i2c, ADDRESS);
        bme280
            .init(&mut embassy_time::Delay)
            .inspect_err(|_| error!("Failed to initialize BME280"))
            .unwrap();
        bme280
    };

    let spi = {
        debug!("Initializing SPI...");
        let config = spi::master::Config::default()
            .with_frequency(Rate::from_mhz(1))
            .with_mode(spi::Mode::_0);
        spi::master::Spi::new(peripherals.SPI2, config)
            .inspect_err(|e| error!("Error while creating SPI: {e}"))
            .unwrap()
            .with_sck(peripherals.GPIO8)
            .with_mosi(peripherals.GPIO10)
    };

    let mut driver = {
        debug!("Initializing EPD...");
        use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

        let busy = Input::new(
            peripherals.GPIO2,
            InputConfig::default().with_pull(Pull::Up),
        );
        let res = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
        let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
        let cs = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());

        let device = embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs, embassy_time::Delay)
            .inspect_err(|e| error!("Error creating exclusive spi device: {e}"))
            .unwrap();
        let iface = display_interface_spi::SPIInterface::new(device, dc);

        weact_studio_epd::WeActStudio213BlackWhiteDriver::new(iface, busy, res, embassy_time::Delay)
    };
    driver.init().unwrap();

    let style = embedded_graphics::mono_font::MonoTextStyle::new(
        &profont::PROFONT_24_POINT,
        weact_studio_epd::Color::Black,
    );

    static UPDATES_CHANNEL: Channel<CriticalSectionRawMutex, UpdateType, 8> = Channel::new();
    spawner.spawn(task_bme280(bme280, UPDATES_CHANNEL.sender()).unwrap());

    loop {
        match UPDATES_CHANNEL.receive().await {
            UpdateType::BME(t, h, p) => {
                info!("BME: t={t} °C, h={h} %, p={p} Pa");
                let mut display = weact_studio_epd::graphics::Display213BlackWhite::new();
                display.set_rotation(weact_studio_epd::graphics::DisplayRotation::Rotate270);

                _ = embedded_graphics::text::Text::new(
                    alloc::format!("[{:.1} °C]", t).as_str(),
                    embedded_graphics::geometry::Point::new(8, 32),
                    style,
                )
                .draw(&mut display);

                _ = embedded_graphics::text::Text::new(
                    alloc::format!("[{:.1} %]", h).as_str(),
                    embedded_graphics::geometry::Point::new(8, 64),
                    style,
                )
                .draw(&mut display);

                _ = embedded_graphics::text::Text::new(
                    alloc::format!("[{:.1} hPa]", p / 100.0).as_str(),
                    embedded_graphics::geometry::Point::new(8, 96),
                    style,
                )
                .draw(&mut display);

                driver.wake_up().unwrap();
                driver.fast_update(&display).unwrap();
                driver.sleep().unwrap();
            }
        }
    }
}

enum UpdateType {
    BME(f32, f32, f32),
}

#[embassy_executor::task]
async fn task_bme280(
    mut bme280: BME280<i2c::master::I2c<'static, esp_hal::Blocking>>,
    channel: Sender<'static, CriticalSectionRawMutex, UpdateType, 8>,
) {
    loop {
        let m = bme280.measure(&mut embassy_time::Delay).unwrap();
        channel
            .send(UpdateType::BME(m.temperature, m.humidity, m.pressure))
            .await;
        embassy_time::Timer::after_secs(60).await;
    }
}
