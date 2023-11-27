use std::{net::SocketAddr, hash::{Hasher, Hash}, io, fmt};

use bincode::{decode_from_slice, encode_into_slice, Encode};
use tokio::{sync::mpsc::{Sender, channel, Receiver}, net::{TcpStream, tcp::{OwnedWriteHalf, OwnedReadHalf}}, io::AsyncWriteExt};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{types::*, macros::*};

#[derive(Clone)]
pub struct Peer {
    address: SocketAddr,
    peer: Sender<Packet>,
    node: Sender<NodeRequest>,
}

impl Peer {
    pub async fn new(tx_node: Sender<NodeRequest>, stream: TcpStream) -> anyhow::Result<Self> {
        // Create a mpsc channel for managing writes.
        // TODO: Don't use magic numbers, use magic consts
        let (tx_peer, rx_peer) = channel::<Packet>(1000);
        stream.set_nodelay(true)?;
        let (read_stream, write_stream) = stream.into_split();

        let conn = Self {
            address: read_stream.peer_addr()?,
            node: tx_node.clone(),
            peer: tx_peer.clone(),
        };

        // Spawn the manager loop
        tokio::spawn({
            let conn = conn.clone();
            async move {
                conn.request_handler(write_stream, rx_peer).await;
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

    pub async fn send(&self, packet: Packet) {
        log_fail!(self.peer.send(packet).await)
    }

    pub fn get_address(&self) -> SocketAddr {
        self.address
    }

    async fn request_handler(self, mut stream: OwnedWriteHalf, mut rx: Receiver<Packet>) {
        loop {
            match rx.recv().await {
                Some(req) => {
                    log_fail!(Self::send_internal(&mut stream, &req).await)
                }
                None => break,
            }
        }
    }

    async fn send_internal(write_stream: &mut OwnedWriteHalf, packet: &Packet) -> anyhow::Result<()> {
        let bytes = bincode::encode_to_vec(packet, bincode::config::standard())?;
        //write_stream.writable().await?;
        write_stream.write_all(&bytes).await?;
        let to = write_stream.peer_addr()?;

        warn!("Sent: {:?} - {:#?} to {:#}", packet, bytes.len(), to);

        Ok(())
    }

    async fn listen(self, mut read_stream: OwnedReadHalf) {
        loop {
            let mut buf = [0; 4096];

            match read_stream.read(&mut buf).await {
                Ok(0) => {
                    trace!("0 bytes read, closing connection");
                    break;
                }
                // TODO: Remove unwraps and handle buf more elegantly
                Ok(bytes_read) => {
                    tokio::spawn({
                        let peer = self.clone();
                        let bytes_read = bytes_read.clone();
                        let mut buf: Vec<u8> = buf.clone().into_iter().take(bytes_read).collect();
                        async move {
                            loop {
                                let (packet, bytes_decoded): (Packet, _) = bincode::decode_from_slice(
                                    &buf, 
                                    bincode::config::standard()
                                ).unwrap();

                                trace!("Read {:?} bytes as: {:?}", bytes_read, packet);
                                peer.node.send((packet, peer.clone())).await.unwrap();

                                if bytes_read != bytes_decoded && buf.len() - bytes_decoded != 0 {
                                    buf = buf.into_iter().skip(bytes_decoded).collect();
                                } else {
                                    break;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.get_address() == other.get_address()
    }
}

impl Eq for Peer {}

impl Hash for Peer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

impl fmt::Debug for Peer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.address)?;

        Ok(())
    }
}

