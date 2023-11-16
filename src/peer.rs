use std::{net::SocketAddr, hash::{Hasher, Hash}, io, fmt};

use bincode::{serialize, deserialize};
use tokio::{sync::mpsc::{Sender, channel, Receiver}, net::{TcpStream, tcp::{OwnedWriteHalf, OwnedReadHalf}}};

use crate::{types::*, macros::*};

#[derive(Clone)]
pub struct Peer {
    pub address: SocketAddr,
    peer_conn: PeerConn,
    state: State
}

impl Peer {
    pub async fn new(state: State, stream: TcpStream) -> anyhow::Result<Peer> {
        let peer = Peer {
            address: stream.peer_addr()?,
            peer_conn: PeerConn::new(state.clone(), stream).await?,
            state,
        };

        Ok(peer)
    } 

    pub async fn broadcast(&self, trx: AccountTransaction) -> anyhow::Result<()>{
        let request = PeerReq::Broadcast(trx);
        self.peer_conn.sender.send(request).await?;

        Ok(())
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl Eq for Peer {}

impl Hash for Peer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}, \n", self.address)?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct PeerConn {
    sender: Sender<PeerReq>,
    state: State,
}

impl PeerConn {
    async fn new(state: State, stream: TcpStream) -> anyhow::Result<PeerConn> {
        // Create a mpsc channel for managing writes.
        let (tx, rx) = channel::<PeerReq>(100);
        let (read_stream, write_stream) = stream.into_split();

        let conn = PeerConn {
            sender: tx.clone(),
            state: state.clone()
        };

        // Spawn the manager loop
        tokio::spawn({
            let conn = conn.clone();
            async move {
                conn.request_handler(write_stream, rx).await;
            }
        });

        // Spawn the listener loop
        tokio::spawn({
            let conn = conn.clone();
            async move {
                conn.listen(read_stream).await;
            }
        });

        Ok(conn)
    }

    fn broadcast(&self, trx: AccountTransaction) {
        let conn = self.clone();
        tokio::spawn(async move {
            if !conn.state.history.contains(&trx) {
                println!("{:}", trx);

                log_fail!(conn.state.ledger.update(&trx));

                for peer in conn.state.peers.clone_iter() {
                    let trx = trx.clone();
                    tokio::spawn(async move {
                        log_fail!(peer.broadcast(trx).await)
                    });
                }
            }

            conn.state.history.insert(trx);
        });
    }

    async fn request_handler(self, stream: OwnedWriteHalf, mut rx: Receiver<PeerReq>) {
        loop {
            match rx.recv().await {
                // A broadcast request was received.
                Some(PeerReq::Broadcast(trx)) => {
                    self.broadcast(trx)
                }
                // A list peers request was received.
                Some(PeerReq::GetPeers) => {
                    let peers = self.state.peers.to_vec();
                    let res = Packet::Response(PeerResponse::GetPeers(peers));
                    skip_fail!(Self::send(&stream, &res).await);
                }
                None => break,
            }
        }
    }

    async fn send(write_stream: &OwnedWriteHalf, packet: &Packet) -> anyhow::Result<()> {
        let bytes = serialize(packet)?;
        write_stream.writable().await?;
        let bytes_written = write_stream.try_write(&bytes)?;
        let to = write_stream.peer_addr()?;

        trace!("Wrote {:#?}/{:#?} to {:#}", bytes.len(), bytes_written, to);

        Ok(())
    }

    async fn listen(self: PeerConn, read_stream: OwnedReadHalf) {
        loop {
            let mut buf = [0; 4096];

            skip_fail!(read_stream.readable().await);

            match read_stream.try_read(&mut buf) {
                Ok(0) => {
                    trace!("0 bytes read, closing connection");
                    break;
                }
                Ok(bytes_read) => {
                    trace!("Read {:?} bytes.", bytes_read);

                    tokio::spawn({
                        let peer = self.clone();
                        async move {
                            log_fail!(peer.handle_packet(&buf).await);
                        }
                    });
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    debug!("Entered would_block");
                    continue;
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }
    }

    async fn handle_packet(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let packet: Packet = deserialize(bytes)?;

        match packet {
            Packet::Transaction(trx) => self.broadcast(trx),
            Packet::Response(PeerResponse::GetPeers(peers)) => {
                for peer in peers {
                    //self.state.connect_to_peer(peer).await?
                }
            }
            Packet::Request(req) => self.sender.send(req).await?
        }

        Ok(())

    }
}
