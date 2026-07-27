#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use alloc::boxed::Box;

use defmt::error;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig, Pin};
use esp_hal::ledc::timer::Timer as LedcTimer;
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use static_cell::StaticCell;
use vcu::steering::Steering;
use vcu::{config, control, drivers, heartbeat_led, net, steering, telemetry};

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
    spawner.spawn(telemetry::telemetry_task().expect("telemetry_task spawns exactly once"));
    spawner.spawn(control::arbiter::arbiter_task().expect("arbiter_task spawns exactly once"));

    static MOTOR_TIMER: StaticCell<LedcTimer<'static, LowSpeed>> = StaticCell::new();

    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let motor_timer = MOTOR_TIMER.init(drivers::motor::configure_timer(&ledc));

    // Rear motors -- L298N #1 (drive: equal throttle both sides, always).
    // GPIO19/21/22 == config::PIN_REAR_L_EN/IN1/IN2.
    let rear_l = drivers::motor::MotorChannel::new(
        motor_timer,
        drivers::motor::channel_from_index(config::LEDC_CH_REAR_L),
        peripherals.GPIO19.degrade(),
        peripherals.GPIO21.degrade(),
        peripherals.GPIO22.degrade(),
    );
    // GPIO23/25/26 == config::PIN_REAR_R_EN/IN1/IN2.
    let rear_r = drivers::motor::MotorChannel::new(
        motor_timer,
        drivers::motor::channel_from_index(config::LEDC_CH_REAR_R),
        peripherals.GPIO23.degrade(),
        peripherals.GPIO25.degrade(),
        peripherals.GPIO26.degrade(),
    );
    let rear = drivers::motor::Motors::new(rear_l, rear_r);

    // Front motors -- L298N #2 (differential steering).
    // GPIO4/32/33 == config::PIN_FRONT_L_EN/IN1/IN2. (Not GPIO16/17: not
    // exposed on this board's header -- see config.rs.)
    let front_l = drivers::motor::MotorChannel::new(
        motor_timer,
        drivers::motor::channel_from_index(config::LEDC_CH_FRONT_L),
        peripherals.GPIO4.degrade(),
        peripherals.GPIO32.degrade(),
        peripherals.GPIO33.degrade(),
    );
    // GPIO13/14/15 == config::PIN_FRONT_R_EN/IN1/IN2.
    let front_r = drivers::motor::MotorChannel::new(
        motor_timer,
        drivers::motor::channel_from_index(config::LEDC_CH_FRONT_R),
        peripherals.GPIO13.degrade(),
        peripherals.GPIO14.degrade(),
        peripherals.GPIO15.degrade(),
    );

    // S2.5 -- differential drive (active now)
    let active_steering: Box<dyn Steering> =
        Box::new(steering::DifferentialSteering::new(front_l, front_r));
    // Future: servo Ackermann (uncomment when BEC wired, comment above)
    // let active_steering: Box<dyn Steering> = Box::new(steering::ServoSteering::new());

    spawner.spawn(
        control::actuators::actuator_task(rear, active_steering)
            .expect("actuator_task spawns exactly once"),
    );

    loop {
        Timer::after(Duration::from_secs(3600)).await;
    }
}
