use crate::p2p::network::P2PNetwork;
use crate::p2p::types::{PeerId, PeerInfo};
use crate::types::{ChainId, Hash, Timestamp};

pub fn make_peer_id(seed: u8) -> PeerId {
    PeerId::new(
        Hash::new([seed; 32]),
        format!("/ip4/127.0.0.{seed}/tcp/30333"),
    )
}

pub fn make_peer(seed: u8, chain_id: ChainId, now: Timestamp) -> PeerInfo {
    PeerInfo::new(make_peer_id(seed), "1.0.0", chain_id, now)
}

pub fn default_network() -> P2PNetwork {
    let local = make_peer_id(0);
    P2PNetwork::new(local, 1, 50, 3)
}
