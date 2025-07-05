use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub::{self, IdentTopic as Topic, MessageAuthenticity, ValidationMode, GossipsubConfigBuilder, GossipsubMessage},
    identity,
    mdns::{Mdns, MdnsEvent, Config as MdnsConfig},
    noise,
    swarm::{SwarmBuilder, SwarmEvent, NetworkBehaviourEventProcess},
    tcp::{Config as TcpConfig, TokioTcpTransport},
    yamux, Multiaddr, PeerId, Transport, NetworkBehaviour, Swarm
};
use std::time::Duration;
use tokio::sync::mpsc;
use consensus_node_lib::data_structures::{P2PMessage, TxData}; // Import shared structures
use crate::SimulatorConfig; // Import simulator specific config
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Define the simulator's network behaviour
#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub struct SimulatorBehaviour {
    pub gossipsub: gossipsub::Gossipsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub local_peer_id: PeerId,
    // Potentially channels for receiving messages if the simulator needs to react to network events
    // For now, primarily focused on sending.
}

impl NetworkBehaviourEventProcess<MdnsEvent> for SimulatorBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        if let MdnsEvent::Discovered(list) = event {
            for (peer_id, _multiaddr) in list {
                println!("[Simulator] mDNS discovered a new peer: {}", peer_id);
                self.gossipsub.add_explicit_peer(&peer_id);
            }
        } else if let MdnsEvent::Expired(list) = event {
            for (peer_id, _multiaddr) in list {
                println!("[Simulator] mDNS peer expired: {}", peer_id);
                self.gossipsub.remove_explicit_peer(&peer_id);
            }
        }
    }
}

// Event processing for Gossipsub - Simulator might listen for leader announcements or other info
impl NetworkBehaviourEventProcess<gossipsub::GossipsubEvent> for SimulatorBehaviour {
    fn inject_event(&mut self, event: gossipsub::GossipsubEvent) {
        if let gossipsub::GossipsubEvent::Message { message, .. } = event {
            // The simulator could parse messages to find leaders, etc.
            // For now, just log receipt.
            if let Ok(p2p_message) = serde_json::from_slice::<P2PMessage>(&message.data) {
                println!("[Simulator] Received P2PMessage: {:?}", p2p_message);
                // TODO: Potentially identify leaders from NewLeaderList messages
            } else {
                // println!("[Simulator] Received undecipherable message on gossipsub");
            }
        }
    }
}


pub async fn start_simulator_swarm(
    config: &SimulatorConfig,
    // TODO: Potentially pass an MPSC sender here if the main loop needs to send commands to the swarm task
) -> Result<Swarm<SimulatorBehaviour>, Box<dyn std::error::Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("[Simulator] Local Peer ID: {}", local_peer_id);

    let transport = TokioTcpTransport::new(TcpConfig::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseAuthenticated::xx(&local_key)?)
        .multiplex(yamux::YamuxConfig::default())
        .timeout(std::time::Duration::from_secs(20))
        .boxed();

    // Content-addressable message IDs
    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
    };

    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict) // Or Anonymous if messages are not always signed by known peers
        .message_id_fn(message_id_fn)
        .build()?;

    // We use the same keypair for signing gossipsub messages as for app-level identity.
    // If simulator doesn't sign its own app-level messages in a way consensus_node expects,
    // it might need MessageAuthenticity::Anonymous or a specific key for gossipsub.
    // For sending ClientSubmitRawTransaction, the TxData is signed by a *SimulatedUser*'s key,
    // not the simulator's libp2p node key. So, gossipsub messages from simulator can be anonymous
    // or signed by its own libp2p key if that's desired for network participation.
    // Let's make it signed by its own key for now.
    let mut gossipsub = gossipsub::Gossipsub::new(MessageAuthenticity::Signed(local_key.clone()), gossipsub_config)?;

    let topic = Topic::new("consensus-messages");
    gossipsub.subscribe(&topic)?;

    let mdns_config = MdnsConfig {
        ttl: Duration::from_secs(60), // How long to keep discovered peers.
        query_interval: Duration::from_secs(10), // Interval for sending mDNS queries.
        ..Default::default()
    };
    let mdns = Mdns::new(mdns_config).await?;

    let behaviour = SimulatorBehaviour {
        gossipsub,
        mdns,
        local_peer_id,
    };

    let mut swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(Box::new(|fut| { tokio::spawn(fut); }))
        .build();

    if let Some(addr_str) = &config.target_multiaddr {
        match addr_str.parse::<Multiaddr>() {
            Ok(addr) => {
                swarm.dial(addr)?;
                println!("[Simulator] Dialing target peer: {}", addr_str);
            }
            Err(e) => eprintln!("[Simulator] Failed to parse target_multiaddr '{}': {}", addr_str, e),
        }
    }

    let listen_addr_str = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port);
    swarm.listen_on(listen_addr_str.parse()?)?;

    Ok(swarm)
}
```
