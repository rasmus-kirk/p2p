use std::net::{SocketAddr, Ipv4Addr, IpAddr};

use rand::Rng;
use tokio::{net::TcpListener, sync::mpsc::{Sender, channel, Receiver}};
use rand::rngs::OsRng;
use ed25519_dalek::SigningKey;

use crate::types::*;
use crate::macros::*;


#[derive(Clone)]
pub struct Node {
    pub name: NodeName,
    pub socket: SocketAddr,
    node_tx: Sender<NodeRequest>,
    pub state: State
}

impl Node {
    pub async fn new(name: &str) -> anyhow::Result<Self> {
        // Get IP and port
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);
        let socket = SocketAddr::new(ip, port);
        let name = NodeName(name.to_owned());
        let (node_tx, node_rx) = channel::<NodeRequest>(1000);

        let node = Self {
            state: State::new(socket, name.clone()),
            name,
            socket,
            node_tx,
        };

        log_fail!(node.listen().await);
        
        tokio::spawn({
            let node = node.clone();
            async move { node.peer_receiver(node_rx).await; }
        });

        Ok(node)
    }

    pub fn get_address(&self) -> SocketAddr {
        self.socket
    }

    pub fn get_node_name(&self) -> NodeName {
        self.name.clone()
    }

    pub fn get_history(&self) -> History {
        self.state.history.clone()
    }

    pub fn get_peers(&self) -> Peers {
        self.state.peers.clone()
    }

    pub fn get_ledger(&self) -> Ledger {
        self.state.ledger.clone()
    }

    pub fn get_balance(&self, id: &Id) -> Amount {
        self.state.ledger.get(id).unwrap()
    }

    pub async fn listen(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.socket).await?;

        tokio::spawn({
            let node = self.clone();
            async move {
                loop {
                    trace!("{:?}-listen: Waiting for connections", node.name);
                    let (stream, addr) = skip_fail!(listener.accept().await);

                    info!("󰟅 Listener accepted tcp stream from {:?}, handling:", addr);
                    skip_fail!(node.state.peers.new_stream(node.node_tx.clone(), stream).await);
                }
            }
        });

        Ok(())
    }

    pub async fn peer_receiver(&self, mut rx: Receiver<NodeRequest>) {
        let node = self.clone();
        loop {
            if let Some((packet, peer)) = rx.recv().await {
                debug!("{:?}: Received {:?} from {:?}", node.name, packet, peer);
                match packet {
                    Packet::GetPeers => {
                        let peers = node.state.peers.to_vec();
                        let packet = Packet::ResponseGetPeers(peers);

                        peer.send(packet).await
                    }
                    Packet::Broadcast(trx) => {
                        node.broadcast(trx);
                    }
                    Packet::AddPeer(socket) => {
                        skip_fail!(self.state.peers.add_peer(socket, peer))
                    } 
                    Packet::ResponseGetPeers(peers) => {
                        skip_fail!(node.state.peers.new_conns(node.node_tx.clone(), peers).await);
                    }
                }
            } else {
                info!("peer_reveiver received empty request, shutting down");
                break
            }
        }
    }

    pub async fn connect(&self, addr: SocketAddr) -> anyhow::Result<()> {
        self.state.peers.new_conn(self.node_tx.clone(), addr).await?;

        Ok(())
    }

    pub fn gen_keys(&self) -> KeyPair {
        let mut csprng = OsRng;
        let sk = SigningKey::generate(&mut csprng);
        let pk = sk.verifying_key();
        KeyPair {
            private: sk.into(),
            public: pk.into(),
        }
    }

    pub async fn send(&self, trx: SignedAccountTransaction) {
        self.broadcast(trx.into())
    }

    fn broadcast(&self, trx: SignedAccountTransaction) {
        let conn = self.clone();

        let is_trx_valid = trx.verify();
        let is_trx_new = conn.state.history.insert(trx.trx.clone());
        
        if is_trx_new && is_trx_valid {
            info!(" {:?}: {:?}", self.name, trx);

            log_fail!(conn.state.ledger.update(&trx.trx));

            for peer in conn.state.peers.clone_iter() {
                let trx = trx.clone();
                tokio::spawn(async move {
                    let packet = Packet::Broadcast(trx);
                    peer.1.send(packet).await;
                });
            }
        }
    }
}
