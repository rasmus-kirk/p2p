use std::net::{SocketAddr, Ipv4Addr, IpAddr};

use rand::Rng;
use tokio::{net::TcpListener, sync::mpsc::{Sender, channel, Receiver}};

use crate::types::*;
use crate::macros::*;


#[derive(Clone)]
pub struct Node {
    pub id: Id,
    pub socket: SocketAddr,
    node_tx: Sender<NodeRequest>,
    pub state: State
}

impl Node {
    pub fn new(id: &str) -> anyhow::Result<Self> {
        // Get IP and port
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);
        let socket = SocketAddr::new(ip, port);
        let (node_tx, node_rx) = channel::<NodeRequest>(1000);

        let node = Self {
            id: Id(id.to_owned()),
            socket,
            node_tx,
            state: State::new(socket)
        };

        tokio::spawn({
            let node = node.clone();
            async move { log_fail!(node.listen().await); }
        });
        
        tokio::spawn({
            let node = node.clone();
            async move { node.peer_receiver(node_rx).await; }
        });

        Ok(node)
    }

    pub fn get_address(&self) -> SocketAddr {
        self.socket
    }

    pub fn get_id(&self) -> Id {
        self.id.clone()
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

    pub async fn listen(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.socket).await?;

        tokio::spawn({
            let node = self.clone();
            async move {
                loop {
                    trace!("{:?}-listen: Waiting for connections", node.id);
                    let (stream, addr) = skip_fail!(listener.accept().await);

                    info!("listen: accepted tcp stream from {:?}, handling:", addr);
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
                debug!("{:?}: Received {:?} from {:?}", node.id, packet, peer);
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

    pub async fn send(&self, to: Id, amount: i64) {
        let trx = AccountTransaction {
            to: to.clone(),
            from: self.id.clone(),
            amount: Amount(amount),
            timestamp: Timestamp::since_unix().unwrap(),
        };
        self.broadcast(trx)
        
    }

    fn broadcast(&self, trx: AccountTransaction) {
        let conn = self.clone();
        let is_trx_new = conn.state.history.insert(trx.clone());
        
        if is_trx_new {
            info!("{:?}: {:}", self.id, trx);

            log_fail!(conn.state.ledger.update(&trx));

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
