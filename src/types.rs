use std::{fmt, net::SocketAddr, str::FromStr, collections::HashMap};
use serde::{Serialize, Deserialize};
use tokio::net::TcpStream;

#[derive(Eq, PartialEq, Hash, Clone, Serialize, Deserialize, Debug)]
pub struct Id(pub String);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub struct Peer {
    pub socket: SocketAddr,
}

impl Peer {
    pub fn new(socket_string: &str) -> anyhow::Result<Self> {
        Ok(Self {
            socket: SocketAddr::from_str(socket_string.trim())?,
        })
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

//pub type Ledger = HashMap<Id, Amount>;
pub struct Ledger(HashMap<Id, Amount>);

impl Ledger {
    pub fn new() -> Ledger {
        Ledger(HashMap::<Id, Amount>::new())
    }

    pub fn update(&mut self, trx: &AccountTransaction) -> anyhow::Result<()> {
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
        for (k, v) in self.0.iter() {
            write!(f, "{}, {}, \n", k, v)?
        }

        Ok(())
    }
}
