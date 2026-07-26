//! Broker connection: connects with a Last Will and Testament, announces
//! `VcuStatus::Online`, subscribes to `cmd/manual`, then serves both
//! directions -- inbound commands and outbound telemetry -- until the
//! connection drops.
//!
//! The broker's LWT mechanism -- not this task -- is what announces
//! `VcuStatus::Offline` on an unclean disconnect (dead socket, power loss).
//! That is the entire point of registering it at connect time.
//!
//! S2 has no safety logic: a decoded, clamped command is applied directly.
//! There is no deadman timeout yet (S3), so if the broker/network vanishes
//! without the socket noticing, the last applied command holds indefinitely.

use core::num::NonZero;

use defmt::{info, warn};
use embassy_futures::select::{Either, select};
use embassy_net::{Ipv4Address, Stack, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use rust_mqtt::{
    Bytes,
    buffer::AllocBuffer,
    client::{
        Client,
        event::Event,
        options::{
            ConnectOptions, PublicationOptions, SubscriptionOptions, TopicReference, WillOptions,
        },
    },
    config::KeepAlive,
    types::{MqttBinary, MqttString, TopicFilter, TopicName},
};
use shared::{DriveCommand, VcuState};

use crate::{config, control::RAW_CMD, net::wifi};

/// Backoff between broker (re)connect attempts.
const RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

/// ~5s keep-alive gives ~7.5s worst-case broker-side dead-VCU detection,
/// which is the S1 T6 acceptance gate.
const KEEP_ALIVE_SECS: u16 = 5;

const TCP_BUFFER_LEN: usize = 2048;

/// Latest telemetry sample, handed off from [`crate::telemetry`] to whichever
/// task currently owns the broker connection. Overwrite (not queue)
/// semantics: only the newest sample matters, so a disconnected or slow
/// transport never backs up or delays the telemetry ticker.
pub static TELEMETRY: Signal<CriticalSectionRawMutex, VcuState> = Signal::new();

fn encode_status<'b>(status: shared::VcuStatus, buf: &'b mut [u8]) -> &'b [u8] {
    let len = shared::encode(&status, buf).expect("VcuStatus always fits MAX_PAYLOAD_STATE");
    &buf[..len]
}

fn topic(name: &str) -> TopicName<'_> {
    TopicName::new(MqttString::try_from(name).expect("topic constants are valid MQTT strings"))
        .expect("topic constants are valid MQTT topic names")
}

fn topic_filter(name: &str) -> TopicFilter<'_> {
    TopicFilter::new(MqttString::try_from(name).expect("topic constants are valid MQTT strings"))
        .expect("topic constants are valid MQTT topic filters")
}

/// Decodes a `cmd/manual` payload, clamps it, and hands it to
/// [`crate::control::actuators::actuator_task`]. Must never panic on a
/// malformed payload -- a bad publish from any client on the LAN is an
/// expected, not exceptional, occurrence.
fn dispatch_command(payload: &[u8]) {
    match shared::decode::<DriveCommand>(payload) {
        Ok(cmd) => RAW_CMD.signal(cmd.clamped()),
        Err(_) => warn!("cmd/manual: malformed payload, ignoring"),
    }
}

/// Owns the broker connection for the VCU's lifetime: connects (registering
/// the LWT), announces Online, subscribes to `cmd/manual`, then serves
/// inbound commands and outbound telemetry on the same socket until the
/// connection drops, at which point it retries with backoff and re-announces
/// Online on the next successful connect.
#[embassy_executor::task]
pub async fn transport_task(stack: Stack<'static>) -> ! {
    let broker_addr: Ipv4Address = config::MQTT_BROKER_HOST
        .parse()
        .expect("MQTT_BROKER_HOST must be a valid IPv4 address");

    loop {
        if !wifi::is_up() {
            Timer::after(RECONNECT_BACKOFF).await;
            continue;
        }

        let mut rx_buffer = [0u8; TCP_BUFFER_LEN];
        let mut tx_buffer = [0u8; TCP_BUFFER_LEN];
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        if let Err(e) = socket
            .connect((broker_addr, config::MQTT_BROKER_PORT))
            .await
        {
            warn!("mqtt: tcp connect failed: {}", e);
            Timer::after(RECONNECT_BACKOFF).await;
            continue;
        }

        let mut offline_payload = [0u8; shared::MAX_PAYLOAD_STATE];
        let offline_bytes = encode_status(shared::VcuStatus::Offline, &mut offline_payload);
        let will = WillOptions::new(
            topic(shared::TOPIC_VCU_STATUS),
            MqttBinary::try_from(offline_bytes).expect("offline payload fits MqttBinary"),
        )
        .at_least_once()
        .retain();

        let connect_options = ConnectOptions::new()
            .clean_start()
            .keep_alive(KeepAlive::Seconds(
                NonZero::new(KEEP_ALIVE_SECS).expect("KEEP_ALIVE_SECS is nonzero"),
            ))
            .will(will);

        let client_id = MqttString::try_from(config::MQTT_CLIENT_ID)
            .expect("MQTT_CLIENT_ID is a valid MQTT string");

        let mut buffer = AllocBuffer;
        // 1 in-flight SUBSCRIBE (cmd/manual, sent once below), 1 in-flight
        // QoS>=1 PUBLISH (the online announcement), no subscription
        // identifiers.
        let mut client = Client::<'_, _, _, 1, 1, 1, 0>::new(&mut buffer);

        if let Err(e) = client
            .connect(socket, &connect_options, Some(client_id))
            .await
        {
            warn!("mqtt: connect failed: {}", e);
            Timer::after(RECONNECT_BACKOFF).await;
            continue;
        }
        info!("mqtt: connected to broker as {}", config::MQTT_CLIENT_ID);

        let mut online_payload = [0u8; shared::MAX_PAYLOAD_STATE];
        let online_bytes = encode_status(shared::VcuStatus::Online, &mut online_payload);
        let announce =
            PublicationOptions::new(TopicReference::Name(topic(shared::TOPIC_VCU_STATUS)))
                .at_least_once()
                .retain();

        if let Err(e) = client.publish(&announce, Bytes::from(online_bytes)).await {
            warn!("mqtt: online announcement failed: {}", e);
            Timer::after(RECONNECT_BACKOFF).await;
            continue;
        }
        info!("mqtt: announced online");

        if let Err(e) = client
            .subscribe(
                topic_filter(shared::TOPIC_CMD_MANUAL),
                SubscriptionOptions::new(),
            )
            .await
        {
            warn!("mqtt: subscribe to cmd/manual failed: {}", e);
            Timer::after(RECONNECT_BACKOFF).await;
            continue;
        }
        info!("mqtt: subscribed to cmd/manual");

        loop {
            if !wifi::is_up() {
                warn!("mqtt: wifi down, reconnecting");
                break;
            }

            match select(client.poll(), TELEMETRY.wait()).await {
                Either::First(Ok(Event::Publish(publish))) => {
                    dispatch_command(publish.message.as_bytes());
                }
                Either::First(Ok(_other_event)) => {
                    // SUBACK, PINGRESP, etc: nothing to do for S2.
                }
                Either::First(Err(e)) => {
                    warn!("mqtt: poll failed, reconnecting: {}", e);
                    break;
                }
                Either::Second(state) => {
                    let mut payload = [0u8; shared::MAX_PAYLOAD_STATE];
                    let Ok(len) = shared::encode(&state, &mut payload) else {
                        continue; // VcuState always fits MAX_PAYLOAD_STATE; unreachable.
                    };

                    let options = PublicationOptions::new(TopicReference::Name(topic(
                        shared::TOPIC_VCU_STATE,
                    )));

                    if let Err(e) = client.publish(&options, Bytes::from(&payload[..len])).await {
                        warn!("mqtt: telemetry publish failed, reconnecting: {}", e);
                        break;
                    }
                }
            }
        }

        Timer::after(RECONNECT_BACKOFF).await;
    }
}
