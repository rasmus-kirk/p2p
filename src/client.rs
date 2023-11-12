use std::{net::{SocketAddr, Ipv4Addr, IpAddr}, collections::HashSet, io};

use bincode::{serialize, deserialize};
use rand::Rng;
use tokio::{net::{TcpListener, tcp::{OwnedWriteHalf, OwnedReadHalf}, TcpStream}, sync::mpsc::{Sender, channel, Receiver}};
use crate::types::*;

//mod grpc_server;
use crate::grpc_server::*;

use crate::macros::*;

struct ClientManager {}

impl ClientManager {
    fn init() -> Sender<ManReq> {
        // Create a mpsc channel for managing writes.
        let (tx, rx) = channel::<ManReq>(100);
        let tx_clone = tx.clone();

        // Spawn the manager loop
        tokio::spawn(async move {
            Self::start_loop(rx, tx).await;
        });

        // Return sender handle
        tx_clone
    }

    async fn start_loop(mut rx: Receiver<ManReq>, tx: Sender<ManReq>) {
        let mut peers = Vec::<OwnedWriteHalf>::new();
        let mut history = HashSet::<AccountTransaction>::new();
        let mut ledger = Ledger::new();

        loop {
            match rx.recv().await {
                // A broadcast request was received.
                Some(ManReq::Broadcast(trx)) => {
                    if !history.contains(&trx) {
                        println!("{:}", trx);

                        skip_fail!(ledger.update(&trx));

                        for conn in peers.iter() {
                            skip_fail!(Self::send(conn, &trx).await)
                        }
                    }

                    history.insert(trx);
                }
                // A connection request was received.
                Some(ManReq::Connect(stream)) => {
                    let tx = tx.clone();
                    let (read_stream, write_stream) = stream.into_split();

                    peers.push(write_stream);

                    info!(
                        "Established connection with: {:#}",
                        skip_fail!(read_stream.peer_addr())
                    );

                    // TODO: Add Peer to ledger. How do I get the ID from peer?

                    tokio::spawn(async move {
                        Self::handle_peer_stream(read_stream, tx).await;
                    });
                }
                // A list peers request was received.
                Some(ManReq::ListPeers) => {
                    let peers: Vec<SocketAddr> =
                        peers.iter().filter_map(|x| x.peer_addr().ok()).collect();
                    println!("{:?}", peers);
                }
                // A list peers request was received.
                Some(ManReq::ListBalances) => {
                    println!("{}", ledger);
                }
                None => break,
            }
        }
    }

    async fn send(write_stream: &OwnedWriteHalf, trx: &AccountTransaction) -> anyhow::Result<()> {
        let bytes = serialize(trx)?;
        write_stream.writable().await?;
        let bytes_written = write_stream.try_write(&bytes)?;
        let to = write_stream.peer_addr()?;

        trace!("Wrote {:#?}/{:#?} to {:#}", bytes.len(), bytes_written, to);

        Ok(())
    }

    async fn handle_peer_stream(read_stream: OwnedReadHalf, tx: Sender<ManReq>) {
        loop {
            let mut buf = [0; 4096];

            skip_fail!(read_stream.readable().await);

            match read_stream.try_read(&mut buf) {
                Ok(0) => {
                    trace!("0 bytes read, closing connection");
                    break;
                }
                Ok(bytes_read) => {
                    trace!("Read {:?} bytes with the following message:", bytes_read);

                    let trx = skip_fail!(deserialize(&buf));
                    skip_fail!(tx.send(ManReq::Broadcast(trx)).await);
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
}

pub struct Client {
    id: Id,
    socket: SocketAddr,
    tx: Sender<ManReq>,
}

impl Client {
    pub fn new(id: &str) -> anyhow::Result<Self> {
        // Get IP and port
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);

        let tx = ClientManager::init();

        let client = Self {
            id: Id(id.to_owned()),
            socket: SocketAddr::new(ip, port),
            tx,
        };

        Ok(client)
    }

    pub fn get_address(&self) -> SocketAddr {
        self.socket
    }

    pub fn get_id(&self) -> Id {
        self.id.clone()
    }

    pub async fn listen(&mut self) {
        let listener = TcpListener::bind(self.socket).await.unwrap();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            loop {
                let (stream, addr) = listener.accept().await.unwrap();

                info!("listen: accepted tcp stream from {:?}, handling:", addr);

                tx.send(ManReq::Connect(stream)).await.unwrap();
            }
        });
    }

    pub async fn connect(&self, peer: Peer) -> anyhow::Result<()> {
        let conn = TcpStream::connect(peer.socket).await?;
        self.tx.send(ManReq::Connect(conn)).await?;

        Ok(())
    }

    pub async fn list_peers(&self) -> anyhow::Result<()> {
        self.tx.send(ManReq::ListPeers).await?;

        Ok(())
    }

    pub async fn list_balances(&self) -> anyhow::Result<()> {
        self.tx.send(ManReq::ListBalances).await?;

        Ok(())
    }

    pub async fn send(&self, to: Id, amount: Amount) -> anyhow::Result<()> {
        let trx = AccountTransaction {
            from: self.id.clone(),
            to,
            amount,
        };
        self.tx.send(ManReq::Broadcast(trx)).await?;

        Ok(())
    }
}
