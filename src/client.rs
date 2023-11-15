use std::{net::{SocketAddr, Ipv4Addr, IpAddr}, io, sync::Arc, hash::{Hash, Hasher}, fmt};

use bincode::{serialize, deserialize};
use rand::Rng;
use tokio::{net::{TcpListener, tcp::{OwnedWriteHalf, OwnedReadHalf}, TcpStream}, sync::mpsc::{Sender, channel, Receiver}};
use anyhow::anyhow;
use dashmap::*;

use crate::{types::*, peer::Peer};
use crate::macros::*;

struct ClientManager {}

pub struct Client {
    id: Id,
    socket: SocketAddr,
    state: State
}

impl Client {
    pub fn new(id: &str) -> anyhow::Result<Self> {
        // Get IP and port
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);

        let client = Self {
            id: Id(id.to_owned()),
            socket: SocketAddr::new(ip, port),
            state: State::new()
        };

        Ok(client)
    }

    pub fn get_address(&self) -> SocketAddr {
        self.socket
    }

    pub fn get_id(&self) -> Id {
        self.id.clone()
    }

    pub fn get_peers(&self) -> Peers {
        self.state.peers.clone()
    }

    pub fn get_ledger(&self) -> Ledger {
        self.state.ledger.clone()
    }

    pub async fn listen(&mut self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.socket).await?;
        let state = self.state.clone();

        tokio::spawn(async move {
            loop {
                let (stream, addr) = skip_fail!(listener.accept().await);

                info!("listen: accepted tcp stream from {:?}, handling:", addr);
                let peer = skip_fail!(Peer::new(state.clone(), stream).await);
                state.peers.insert(peer);
            }
        });

        Ok(())
    }

    pub async fn connect(&self, addr: SocketAddr) -> anyhow::Result<()> {
        let stream = TcpStream::connect(addr).await?;
        let peer = Peer::new(self.state.clone(), stream).await?;
        self.state.peers.insert(peer);

        Ok(())
    }

    pub async fn send(&self, to: Id, amount: Amount) -> anyhow::Result<()> {
        for peer in self.state.peers.iter() {
            let peer = peer.key();
            let trx = AccountTransaction {
                to: to.clone(),
                from: self.id.clone(),
                amount,
            };
            peer.broadcast(trx).await?
        }

        Ok(())
    }
}
