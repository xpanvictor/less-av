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
use embassy_time::{Duration, Instant, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig, Pin};
use esp_hal::ledc::timer::Timer as LedcTimer;
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use static_cell::StaticCell;
use vcu::{control, drivers, heartbeat_led, net, telemetry};

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

    let boot = Instant::now();

    // GPIO2 == config::PIN_LED_HEARTBEAT.
    let led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    spawner.spawn(
        heartbeat_led::heartbeat_led_task(led).expect("heartbeat_led_task spawns exactly once"),
    );

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
    spawner.spawn(telemetry::telemetry_task(boot).expect("telemetry_task spawns exactly once"));

    static SERVO_TIMER: StaticCell<LedcTimer<'static, LowSpeed>> = StaticCell::new();
    static MOTOR_TIMER: StaticCell<LedcTimer<'static, LowSpeed>> = StaticCell::new();

    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let servo_timer = SERVO_TIMER.init(drivers::servo::configure_timer(&ledc));
    let motor_timer = MOTOR_TIMER.init(drivers::motor::configure_timer(&ledc));

    let servo = drivers::servo::Servo::new(servo_timer, peripherals.GPIO18.degrade());
    let motors = drivers::motor::Motors::new(
        motor_timer,
        peripherals.GPIO19.degrade(),
        peripherals.GPIO21.degrade(),
        peripherals.GPIO22.degrade(),
        peripherals.GPIO23.degrade(),
        peripherals.GPIO25.degrade(),
        peripherals.GPIO26.degrade(),
    );
    spawner.spawn(
        control::actuators::actuator_task(servo, motors)
            .expect("actuator_task spawns exactly once"),
    );

    loop {
        Timer::after(Duration::from_secs(3600)).await;
    }
}
