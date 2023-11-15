use std::{fmt, net::SocketAddr, str::FromStr, collections::HashMap, sync::Arc};
use serde::{Serialize, Deserialize};
use tokio::{net::TcpStream, sync::mpsc::Sender};
use dashmap::{DashMap, DashSet};

use crate::*;

#[derive(Eq, PartialEq, Hash, Clone, Serialize, Deserialize, Debug)]
pub struct Id(pub String);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Eq, PartialEq, Hash, Clone, Serialize, Deserialize, Debug)]
pub struct Amount(pub u64);

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Sub for Amount {
    type Output = Amount;

    fn sub(self, rhs: Amount) -> Self::Output {
        Amount(self.0 - rhs.0)
    }
}

impl std::ops::Sub for &Amount {
    type Output = Amount;

    fn sub(self, rhs: &Amount) -> Self::Output {
        Amount(self.0 - rhs.0)
    }
}

impl std::ops::Add for Amount {
    type Output = Amount;

    fn add(self, rhs: Amount) -> Self::Output {
        Amount(self.0 + rhs.0)
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Serialize, Deserialize, Debug)]
pub struct AccountTransaction {
    pub to: Id,
    pub from: Id,
    pub amount: Amount,
}

impl fmt::Display for AccountTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} -> {}: {} DKK", self.from, self.to, self.amount)
    }
}

pub enum ManReq {
    Broadcast(AccountTransaction),
    Connect(TcpStream),
    ListPeers,
    ListBalances,
}

#[derive(Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum Packet {
    Transaction(AccountTransaction),
    Response(PeerResponse),
    Request(PeerReq)
}

#[derive(Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum PeerReq {
    GetPeers,
    Broadcast(AccountTransaction)
}


#[derive(Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum PeerResponse {
    GetPeers(Vec<SocketAddr>)
}

#[derive(Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum RpcCall {
    GetPeers,
}

#[derive(Clone)]
pub struct Ledger(Arc<DashMap<Id, Amount>>);

impl Ledger {
    pub fn new() -> Ledger {
        Ledger(Arc::new(DashMap::new()))
    }

    pub fn update(&self, trx: &AccountTransaction) -> anyhow::Result<()> {
        let from_amount = self.0
            .get(&trx.from)
            .ok_or(anyhow::anyhow!(
                "From account does not exist. Invalid transaction"
            ))?
            .clone();
        let to_amount = self.0
            .get(&trx.to)
            .ok_or(anyhow::anyhow!(
                "To account does not exist. Invalid transaction"
            ))?
            .clone();

        self.0.insert(trx.from.clone(), from_amount - trx.amount);
        self.0.insert(trx.to.clone(), to_amount - trx.amount);

        Ok(())
    }
}

impl fmt::Display for Ledger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{}, {}, \n", i.key(), i.value())?
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct Peers(Arc<DashSet<Peer>>);

impl Peers {
    pub fn new() -> Peers {
        Peers(Arc::new(DashSet::new()))
    }

    pub fn insert(&self, peer: Peer) -> bool {
        self.0.insert(peer)
    }

    pub fn into_iter(&self) -> dashmap::iter_set::OwningIter<Peer, std::collections::hash_map::RandomState> {
        self.0.into_iter()
    }

    pub fn iter(&self) -> dashmap::iter_set::Iter<'_, Peer, std::collections::hash_map::RandomState, DashMap<Peer, ()>> {
        self.0.iter()
    }

    pub fn to_vec(&self) -> Vec<SocketAddr> {
        self.iter().map(|x| x.address).collect()
    }
}

impl fmt::Display for Peers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{}, \n", i.key())?
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct State {
    pub history: Arc<DashSet<AccountTransaction>>,
    pub ledger: Ledger,
    pub peers: Peers
}

impl State {
    pub fn new() -> State {
        State {
            history: Arc::new(DashSet::new()),
            ledger: Ledger::new(),
            peers: Peers::new()
        }
    }

    pub async fn connect_to_peer(&self, address: SocketAddr) -> anyhow::Result<()> {
        let stream = TcpStream::connect(address).await?;
        let peer = Peer::new(self.clone(), stream).await?;
        self.peers.insert(peer);

        Ok(())
    }
}


