//! WiFi station connection: join, DHCP, auto-reconnect without a reboot.

use alloc::string::ToString;
use core::sync::atomic::{AtomicBool, Ordering};

use defmt::{info, warn};
use embassy_net::{Config as NetConfig, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{Config as WifiConfig, Interface, WifiController, sta::StationConfig};
use static_cell::StaticCell;

use crate::config;

/// Concurrent sockets the network stack must support: one MQTT/TCP
/// connection, plus DHCP and DNS.
const SOCKET_COUNT: usize = 4;

/// Backoff between reconnect attempts. Long enough not to hot-loop against a
/// down AP or broker, short enough to still feel instant on a real outage.
const RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

static NETWORK_UP: AtomicBool = AtomicBool::new(false);

/// Whether the device currently has a WiFi link *and* a DHCP-assigned
/// address. Readable without blocking by any task that cares.
pub fn is_up() -> bool {
    NETWORK_UP.load(Ordering::Relaxed)
}

static RESOURCES: StaticCell<StackResources<SOCKET_COUNT>> = StaticCell::new();

/// Builds the embassy-net stack over the WiFi station interface. Call once at
/// startup; the returned `Runner` must be handed to [`net_runner_task`].
pub fn new_stack(
    station: Interface<'static>,
    seed: u64,
) -> (Stack<'static>, Runner<'static, Interface<'static>>) {
    let resources = RESOURCES.init(StackResources::new());
    embassy_net::new(
        station,
        NetConfig::dhcpv4(Default::default()),
        resources,
        seed,
    )
}

/// Drives all network stack I/O. Must run forever in its own task; nothing
/// else may block this executor slot.
#[embassy_executor::task]
pub async fn net_runner_task(mut runner: Runner<'static, Interface<'static>>) -> ! {
    runner.run().await
}

/// Connects to WiFi on boot, waits for link-up *and* a DHCP lease before
/// signalling ready, and reconnects automatically (with backoff) on
/// disconnection — without a reboot.
#[embassy_executor::task]
pub async fn connection_task(mut controller: WifiController<'static>, stack: Stack<'static>) -> ! {
    loop {
        if !controller.is_connected() {
            let sta_config = StationConfig::default()
                .with_ssid(config::WIFI_SSID)
                .with_password(config::WIFI_PASSWORD.to_string());

            if let Err(e) = controller.set_config(&WifiConfig::Station(sta_config)) {
                warn!("wifi: set_config failed: {}", e);
                NETWORK_UP.store(false, Ordering::Relaxed);
                Timer::after(RECONNECT_BACKOFF).await;
                continue;
            }

            if let Err(e) = controller.connect_async().await {
                warn!("wifi: connect failed: {}", e);
                NETWORK_UP.store(false, Ordering::Relaxed);
                Timer::after(RECONNECT_BACKOFF).await;
                continue;
            }
        }

        stack.wait_config_up().await;
        if let Some(ip_config) = stack.config_v4() {
            info!("wifi: DHCP address {}", ip_config.address);
        }
        NETWORK_UP.store(true, Ordering::Relaxed);

        // Blocks until the station disconnects, then falls through to retry.
        let _ = controller.wait_for_disconnect_async().await;
        NETWORK_UP.store(false, Ordering::Relaxed);
        warn!(
            "wifi: disconnected, retrying in {}s",
            RECONNECT_BACKOFF.as_secs()
        );
        Timer::after(RECONNECT_BACKOFF).await;
    }
}
