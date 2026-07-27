//! Broker connection: publish-only, no subscriptions, no LWT. The joystick
//! disconnecting is not a safety event -- the VCU's own deadman timer
//! handles command-link loss independently of whether the joystick is even
//! reachable. This is deliberately simpler than the VCU's transport.

use core::num::NonZero;

use defmt::{info, warn};
use embassy_net::{Ipv4Address, Stack, tcp::TcpSocket};
use embassy_time::{Duration, Timer};
use rust_mqtt::{
    Bytes,
    buffer::AllocBuffer,
    client::{
        Client,
        options::{ConnectOptions, PublicationOptions, TopicReference},
    },
    config::KeepAlive,
    types::{MqttString, TopicName},
};

use crate::config;
use crate::input::JOYSTICK_CMD;
use crate::net::wifi;

/// Backoff between broker (re)connect attempts.
const RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

/// A generous keep-alive: the joystick has nothing safety-critical riding on
/// fast dead-link detection (unlike the VCU's 5s) -- that's the deadman
/// timer's job, driven by whether commands arrive, not whether this specific
/// client's TCP connection is technically still open.
const KEEP_ALIVE_SECS: u16 = 10;

const TCP_BUFFER_LEN: usize = 1024;

fn topic(name: &str) -> TopicName<'_> {
    TopicName::new(MqttString::try_from(name).expect("topic constants are valid MQTT strings"))
        .expect("topic constants are valid MQTT topic names")
}

/// Owns the broker connection for the joystick's lifetime: connects (no
/// LWT, clean session), then republishes every `JOYSTICK_CMD` sample as it
/// arrives until the connection drops, at which point it retries with
/// backoff.
#[embassy_executor::task]
pub async fn transport_task(stack: Stack<'static>) -> ! {
    let broker_addr: Ipv4Address = config::MQTT_BROKER_HOST
        .parse()
        .expect("MQTT_BROKER_HOST must be a valid IPv4 address");

    loop {
        if !wifi::WIFI_UP.load(core::sync::atomic::Ordering::Relaxed) {
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

        let connect_options = ConnectOptions::new()
            .clean_start()
            .keep_alive(KeepAlive::Seconds(
                NonZero::new(KEEP_ALIVE_SECS).expect("KEEP_ALIVE_SECS is nonzero"),
            ));

        let client_id = MqttString::try_from(config::MQTT_CLIENT_ID)
            .expect("MQTT_CLIENT_ID is a valid MQTT string");

        let mut buffer = AllocBuffer;
        // Publish-only, QoS0 only: no subscriptions, no in-flight QoS>=1
        // publishes, no subscription identifiers. RECEIVE_MAXIMUM must be
        // >=1 even though we never expect an incoming QoS>=1 publish.
        let mut client = Client::<'_, _, _, 0, 1, 1, 0>::new(&mut buffer);

        if let Err(e) = client
            .connect(socket, &connect_options, Some(client_id))
            .await
        {
            warn!("mqtt: connect failed: {}", e);
            Timer::after(RECONNECT_BACKOFF).await;
            continue;
        }
        info!("mqtt: joystick connected");

        loop {
            if !wifi::WIFI_UP.load(core::sync::atomic::Ordering::Relaxed) {
                warn!("mqtt: wifi down, reconnecting");
                break;
            }

            let cmd = JOYSTICK_CMD.wait().await;

            let mut payload = [0u8; shared::MAX_PAYLOAD_CMD];
            let Ok(len) = shared::encode(&cmd, &mut payload) else {
                continue; // DriveCommand always fits MAX_PAYLOAD_CMD; unreachable.
            };

            // QoS 0, not retained: each command is a full state snapshot, not
            // a delta, so a lost packet is irrelevant -- the next one is 20ms
            // away. QoS 1 would add ACK round-trips for no benefit here.
            let options =
                PublicationOptions::new(TopicReference::Name(topic(shared::TOPIC_CMD_MANUAL)));

            if let Err(e) = client.publish(&options, Bytes::from(&payload[..len])).await {
                warn!("mqtt: publish failed, reconnecting: {}", e);
                break;
            }
        }

        Timer::after(RECONNECT_BACKOFF).await;
    }
}
