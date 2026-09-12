use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use kubeweft_model::DeviceId;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};

use crate::{ClusterStore, ClusterSummary, TransportError, wire::PROTOCOL_VERSION};

pub const DEFAULT_DISCOVERY_PORT: u16 = 37_845;
const MULTICAST_ADDRESS: Ipv4Addr = Ipv4Addr::new(239, 255, 87, 77);
const DISCOVERY_MAGIC: &str = "kubeweft-discovery-v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Untrusted reachability hint; callers must not infer identity or membership.
pub struct DiscoveryAnnouncement {
    pub device_id: DeviceId,
    pub device_name: String,
    pub endpoint: SocketAddr,
    pub cluster: Option<ClusterSummary>,
}

#[derive(Serialize, Deserialize)]
struct DiscoveryPacket {
    magic: String,
    version: u16,
    announcement: DiscoveryAnnouncement,
}

/// Collects untrusted LAN announcements without establishing trust.
pub fn discover(
    discovery_port: u16,
    timeout: Duration,
) -> Result<Vec<DiscoveryAnnouncement>, TransportError> {
    let socket = discovery_listener_socket(discovery_port)?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;

    let deadline = Instant::now() + timeout;
    let mut found = BTreeMap::new();
    let mut buffer = [0_u8; 65_507];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((length, _)) => {
                let Ok(packet) = serde_json::from_slice::<DiscoveryPacket>(&buffer[..length])
                else {
                    continue;
                };
                if packet.magic == DISCOVERY_MAGIC && packet.version == PROTOCOL_VERSION {
                    found.insert(packet.announcement.device_id.clone(), packet.announcement);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(found.into_values().collect())
}

pub(crate) fn run_discovery(
    socket: UdpSocket,
    store: ClusterStore,
    endpoint: SocketAddr,
    discovery_port: u16,
    shutdown: Arc<AtomicBool>,
) -> Result<(), TransportError> {
    while !shutdown.load(Ordering::Relaxed) {
        send_announcement(&socket, &store, endpoint, discovery_port)?;
        for _ in 0..10 {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(())
}

fn send_announcement(
    socket: &UdpSocket,
    store: &ClusterStore,
    endpoint: SocketAddr,
    discovery_port: u16,
) -> Result<(), TransportError> {
    let state = store.load()?;
    let packet = DiscoveryPacket {
        magic: DISCOVERY_MAGIC.to_owned(),
        version: PROTOCOL_VERSION,
        announcement: DiscoveryAnnouncement {
            device_id: state.device_id().clone(),
            device_name: state.device_name().to_owned(),
            endpoint,
            cluster: state.status().cluster,
        },
    };
    let bytes = serde_json::to_vec(&packet)?;
    let multicast = socket.send_to(&bytes, SocketAddrV4::new(MULTICAST_ADDRESS, discovery_port));
    let broadcast = socket.send_to(
        &bytes,
        SocketAddrV4::new(Ipv4Addr::BROADCAST, discovery_port),
    );
    // Linux does not route the limited broadcast back through loopback, so a
    // loopback broadcast keeps multiple local development agents discoverable.
    let loopback = socket.send_to(
        &bytes,
        SocketAddrV4::new(Ipv4Addr::new(127, 255, 255, 255), discovery_port),
    );

    match (multicast, broadcast, loopback) {
        (Err(multicast), Err(_), Err(_)) => Err(multicast.into()),
        _ => Ok(()),
    }
}

pub(crate) fn discovery_sender_socket() -> Result<UdpSocket, TransportError> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    socket.set_multicast_loop_v4(true)?;
    Ok(socket)
}

fn discovery_listener_socket(port: u16) -> Result<UdpSocket, TransportError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
    socket.join_multicast_v4(&MULTICAST_ADDRESS, &Ipv4Addr::UNSPECIFIED)?;
    let socket: UdpSocket = socket.into();
    socket.set_broadcast(true)?;
    socket.set_multicast_loop_v4(true)?;
    Ok(socket)
}
