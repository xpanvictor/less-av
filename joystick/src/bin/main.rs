#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::error;
use embassy_executor::Spawner;
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use joystick::{calibration, heartbeat, input, net};

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
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
async fn main(spawner: Spawner) -> ! {
    let hal_config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(hal_config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 98768);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    // ADC setup. GPIO34/35 == config::PIN_AXIS_X/Y.
    let mut adc_config = AdcConfig::new();
    let mut axis_x = adc_config.enable_pin(peripherals.GPIO34, Attenuation::_11dB);
    let mut axis_y = adc_config.enable_pin(peripherals.GPIO35, Attenuation::_11dB);
    let mut adc = Adc::new(peripherals.ADC1, adc_config);

    // Calibrate at boot: the operator must not touch the joystick for the
    // next 500ms -- see README's boot sequence.
    let cal = calibration::calibrate_at_boot(&mut adc, &mut axis_x, &mut axis_y).await;

    // GPIO33 == config::PIN_LED_HEARTBEAT, active LOW -- initialise HIGH (off).
    let led = Output::new(peripherals.GPIO33, Level::High, OutputConfig::default());

    let (wifi_controller, interfaces) = esp_radio::wifi::new(peripherals.WIFI, Default::default())
        .expect("Failed to initialize Wi-Fi controller");

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let (stack, runner) = net::wifi::new_stack(interfaces.station, seed);

    spawner.spawn(net::wifi::net_runner_task(runner).expect("net_runner_task spawns exactly once"));
    spawner.spawn(
        net::wifi::connection_task(wifi_controller, stack)
            .expect("connection_task spawns exactly once"),
    );
    spawner
        .spawn(net::transport::transport_task(stack).expect("transport_task spawns exactly once"));
    spawner.spawn(
        input::input_task(adc, axis_x, axis_y, cal).expect("input_task spawns exactly once"),
    );
    spawner.spawn(heartbeat::heartbeat_task(led).expect("heartbeat_task spawns exactly once"));

    core::future::pending::<()>().await;
    unreachable!("pending() never resolves");
}
