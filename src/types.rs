use std::{fmt, net::SocketAddr, sync::Arc, time::{UNIX_EPOCH, SystemTime}};
use bincode::{Encode, Decode};
use ed25519_dalek::{VerifyingKey, SigningKey, Signer};
use rand::seq::SliceRandom;
use tokio::{net::TcpStream, sync::mpsc::Sender};
use dashmap::{DashMap, DashSet};
use anyhow::anyhow;

use base64ct::{Base64, Encoding};

use crate::{*, macros::log_fail};

#[derive(Eq, PartialEq, Hash, Clone, Decode, Encode)]
pub struct NodeName(pub String);

impl fmt::Debug for NodeName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Copy, Decode, Encode, Debug)]
pub struct Signature([u8; 64]);

impl From<ed25519_dalek::Signature> for Signature {
    fn from(signature: ed25519_dalek::Signature) -> Signature {
        Signature(signature.to_bytes())
    }
}

impl Into<ed25519_dalek::Signature> for Signature {
    fn into(self) -> ed25519_dalek::Signature {
        ed25519_dalek::Signature::from_bytes(&self.0)
    }
}

#[derive(Eq, PartialEq, Clone)]
pub struct PrivateKey(SigningKey);

impl From<SigningKey> for PrivateKey {
    fn from(key: SigningKey) -> PrivateKey {
        PrivateKey(key)
    }
}

impl PrivateKey {
    pub fn sign(&self, trx: AccountTransaction) -> anyhow::Result<SignedAccountTransaction> {
        let bytes = bincode::encode_to_vec(&trx, bincode::config::standard())?;
        let signature = self.0.sign(&bytes);
        Ok(SignedAccountTransaction {
            signature: signature.into(),
            trx
        })
    }

    pub fn get_pk(&self) -> PublicKey {
        self.0.verifying_key().into()
    }
}

pub type PublicKey = Id;

#[derive(Eq, PartialEq, Clone)]
pub struct KeyPair {
    pub public: PublicKey,
    pub private: PrivateKey,
}

#[derive(Eq, PartialEq, Hash, Clone, Copy, Decode, Encode)]
pub struct Id([u8; 32]);

impl Id {
    fn verify(&self, msg: &[u8], s: &Signature) -> bool {
        let signature: ed25519_dalek::Signature = s.clone().into();
        let res = VerifyingKey::from_bytes(&self.0).unwrap().verify_strict(msg, &signature);
        match res {
            Ok(_) => true,
            Err(_) => false
        }
    }
}

impl From<VerifyingKey> for Id {
    fn from(key: VerifyingKey) -> Id {
        Id(VerifyingKey::to_bytes(&key))
    }
}

impl FromStr for Id {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        Base64::decode(s, &mut bytes).unwrap();
        let key = VerifyingKey::from_bytes(&bytes)?;

        Ok(key.into())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = Base64::encode_string(&self.0);
        write!(f, "{:#?}", str)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = Base64::encode_string(&self.0);
        write!(f, "{:#?}", str)
    }
}

#[derive(Copy, Eq, PartialEq, Hash, Clone, Encode, Decode)]
pub struct Amount(pub i64);

impl fmt::Debug for Amount {
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

#[derive(Eq, PartialEq, Hash, Clone, Encode, Decode)]
pub struct Timestamp(u64);

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", Duration::from_millis(self.0).as_secs())?;

        Ok(())
    }
}

impl Timestamp {
    pub fn since_unix() -> anyhow::Result<Timestamp> {
        Ok(Timestamp(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64))
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Decode, Encode)]
pub struct SignedAccountTransaction {
    pub signature: Signature,
    pub trx: AccountTransaction
}

impl fmt::Debug for SignedAccountTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "󰱒 {:?}", self.trx)
    }
}

impl SignedAccountTransaction {
    pub fn verify(&self) -> bool {
        let bytes_res = bincode::encode_to_vec(&self.trx, bincode::config::standard());
        let bytes = match bytes_res {
            Ok(bs) => bs,
            Err(_) => return false,
        };
        self.trx.from.verify(&bytes, &self.signature)
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Decode, Encode)]
pub struct AccountTransaction {
    pub to: Id,
    pub from: Id,
    pub amount: Amount,
    pub timestamp: Timestamp
}

#[derive(Eq, PartialEq, Hash, Clone, Decode, Encode, Debug)]
pub struct AccountCreation {
    pub id: Id,
}

impl fmt::Display for AccountTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} -> {:?}: {:?} DKK", self.from, self.to, self.amount)
    }
}

impl fmt::Debug for AccountTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} -> {:?}: {:?} DKK", self.from, self.to, self.amount)
    }
}

pub type NodeRequest = (Packet, Peer);

#[derive(Eq, PartialEq, Clone, Hash, Decode, Encode, Debug)]
pub enum Packet {
    GetPeers,
    AddPeer(SocketAddr),
    Broadcast(SignedAccountTransaction),
    ResponseGetPeers(Vec<SocketAddr>),
}

#[derive(Eq, PartialEq, Clone, Hash, Encode, Decode)]
pub enum RpcCall {
    GetPeers,
}

#[derive(Clone)]
pub struct Ledger(Arc<DashMap<Id, Amount>>);

impl Ledger {
    pub fn new() -> Ledger {
        Ledger(Arc::new(DashMap::new()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn update(&self, trx: &AccountTransaction) -> anyhow::Result<()> {
        let from_amount = self.0
            .get(&trx.from)
            .map(|x| x.value().clone())
            .unwrap_or(Amount(0));
        let to_amount = self.0
            .get(&trx.to)
            .map(|x| x.value().clone())
            .unwrap_or(Amount(0));

        self.0.insert(trx.from.clone(), from_amount - trx.amount);
        self.0.insert(trx.to.clone(), to_amount + trx.amount);

        Ok(())
    }

    pub fn get(&self, id: &Id) -> Option<Amount> {
        self.0.get(id).map(|x| x.clone())
    }
}

impl fmt::Debug for Ledger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ ")?;
        for i in self.0.iter() {
            write!(f, "({:?}, {:?}) ", i.key(), i.value())?
        }
        write!(f, "]")?;

        Ok(())
    }
}

impl PartialEq for Ledger {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() &&
        self.0.iter().all(|p| other.0.get(p.key()).is_some_and(|q| q.value() == p.value()))
    }
}

impl Eq for Ledger {}

#[derive(Clone)]
pub struct Peers {
    node_name: NodeName,
    active: Arc<DashMap<SocketAddr, Peer>>,
    inactive: Arc<DashSet<SocketAddr>>,
    self_address: SocketAddr
}

impl Peers {
    pub fn new(self_address: SocketAddr, node_name: NodeName) -> Peers {
        Peers {
            node_name,
            active: Arc::new(DashMap::new()),
            inactive: Arc::new(DashSet::new()),
            self_address
        }
    }

    pub fn len(&self) -> usize {
        self.active.len() + self.inactive.len() + 1
    }

    pub fn clone_iter(&self) -> dashmap::iter::OwningIter<SocketAddr, Peer> {
        (*self.active).clone().into_iter()
    }

    pub fn iter(&self) -> dashmap::iter::Iter<'_, SocketAddr, Peer> {
        self.active.iter()
    }

    pub fn contains(&self, key: &SocketAddr) -> bool {
        self.active.contains_key(key) || self.inactive.contains(key) || self.self_address == *key
    }

    pub fn to_vec(&self) -> Vec<SocketAddr> {
        // include self if there is less than 10 peers
        let mut self_address = vec![];
        if self.len() < 10 {
            self_address.push(self.self_address)
        }
        let mut peers: Vec<SocketAddr> = self.active
            .iter()
            .map(|x| x.key().clone())
            .chain(self.inactive.iter().map(|x| x.key().clone()))
            .chain(self_address.into_iter())
            .collect();

        // Randomize peers
        peers.shuffle(&mut rand::thread_rng());

        // Return 10
        peers.into_iter().take(10).collect()
    }

    pub fn add_peer(&self, socket: SocketAddr, peer: Peer) -> anyhow::Result<()> {
        self.active.insert(socket, peer).ok_or_else(|| anyhow!("Address already bound to a peer."))?;

        Ok(())
    }

    pub fn _add_inactive(&self, socket: SocketAddr) -> bool {
        let res = self.inactive.insert(socket);
        trace!("added inactive: {:?}, {:?}", self, res);
        res
    }

    pub async fn new_conn(&self, node_tx: Sender<NodeRequest>, address: SocketAddr) -> anyhow::Result<()> {
        if self.self_address == address {
            return Ok(())
        }

        if !(self.contains(&address)) {
            let stream = TcpStream::connect(address).await?;
            let peer = Peer::new(node_tx, stream, self.node_name.clone()).await?;
            log_fail!(self.add_peer(address, peer.clone()));
            peer.send(Packet::GetPeers).await;
        }

        Ok(())
    }

    pub async fn new_stream(&self, node_tx: Sender<NodeRequest>, stream: TcpStream) -> anyhow::Result<()> {
        let peer = Peer::new(node_tx, stream, self.node_name.clone()).await?;
        peer.send(Packet::GetPeers).await;

        Ok(())
    }

    pub async fn new_conns(&self, node_tx: Sender<NodeRequest>, addrs: Vec<SocketAddr>) -> anyhow::Result<()> {
        for addr in addrs {
            self.new_conn(node_tx.clone(), addr).await?;
        }

        Ok(())
    }
}

impl PartialEq for Peers {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false
        }
        self.iter().all(|p| other.contains(p.key()))
    }
}

impl Eq for Peers {}

impl fmt::Debug for Peers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{ active: [ ")?;
        for i in self.active.iter() {
            write!(f, "{} ", i.key())?
        }
        write!(f, "], ")?;

        write!(f, "inactive: [ ")?;
        for i in self.inactive.iter() {
            write!(f, "{} ", i.key())?
        }
        write!(f, "], ")?;

        write!(f, "self: [ {} ] }}", self.self_address)?;

        Ok(())
    }
}

pub type History = Arc<DashSet<AccountTransaction>>;

#[derive(Clone, Debug)]
pub struct State {
    pub history: History,
    pub ledger: Ledger,
    pub peers: Peers
}

impl State {
    pub fn new(self_socket: SocketAddr, node_name: NodeName) -> State {
        State {
            history: Arc::new(DashSet::new()),
            ledger: Ledger::new(),
            peers: Peers::new(self_socket, node_name)
        }
    }
}
