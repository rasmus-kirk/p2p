#![feature(let_chains)]

use bincode::{deserialize, serialize};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

#[macro_use]
extern crate log;

#[derive(Eq, PartialEq, Hash, Clone, Serialize, Deserialize, Debug)]
struct Id(String);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
struct Peer {
    socket: SocketAddr,
}

impl Peer {
    fn new(socket_string: &str) -> anyhow::Result<Self> {
        Ok(Self {
            socket: SocketAddr::from_str(socket_string.trim())?,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Serialize, Deserialize, Debug)]
struct Message {
    from: Id,
    content: String,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.from, self.content)
    }
}

struct Client {
    id: Id,
    socket: SocketAddr,
    history: History,
    tx: mpsc::Sender<ManReq>,
}

type History = Arc<Mutex<HashSet<Message>>>;

macro_rules! skip_fail {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!("An error: {}; skipped.", e);
                continue;
            }
        }
    };
}

macro_rules! err_unit {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => warn!("An error: {}; skipped.", e)
        }
    };
}

enum ManReq {
    Broadcast(Message),
    Connect(TcpStream),
    ListPeers,
}

struct ClientManager {}

impl ClientManager {
    fn init() -> (Sender<ManReq>, History) {
        // Create a mpsc channel for managing writes.
        let (tx, rx) = mpsc::channel::<ManReq>(100);
        let tx_clone = tx.clone();

        let history = Arc::new(Mutex::new(HashSet::<Message>::new()));
        let history_clone = history.clone();

        // Spawn the manager loop
        tokio::spawn(async move {
            Self::start(history, rx, tx).await;
        });

        (tx_clone, history_clone)
    }

    async fn start(history: History, mut rx: mpsc::Receiver<ManReq>, tx: mpsc::Sender<ManReq>) {
        let mut peers = Vec::<OwnedWriteHalf>::new();
        loop {
            match rx.recv().await {
                // A broadcast request was received.
                Some(ManReq::Broadcast(msg)) => {
                    let mut history = history.lock().await;

                    if !history.contains(&msg) {
                        println!("{:}", msg);

                        for conn in peers.iter() {
                            skip_fail!(Self::send(conn, &msg).await)
                        }
                    }

                    history.insert(msg);
                }
                // A connection request was received.
                Some(ManReq::Connect(stream)) => {
                    let tx = tx.clone();
                    let (read_stream, write_stream) = stream.into_split();

                    peers.push(write_stream);

                    info!("Established connection with: {:#}", read_stream.peer_addr().unwrap());

                    tokio::spawn(async move {
                        Self::handle_peer_stream(read_stream, tx).await;
                    });
                }
                // A list peers request was received.
                Some(ManReq::ListPeers) => {
                    let peers: Vec<SocketAddr> = peers.iter().filter_map(|x| x.peer_addr().ok()).collect();
                    println!("{:?}", peers);
                }
                None => break,
            }
        }
    }

    async fn send(write_stream: &OwnedWriteHalf, msg: &Message) -> anyhow::Result<()> {
        let bytes = serialize(msg)?;
        write_stream.writable().await?;
        let bytes_written = write_stream.try_write(&bytes)?;
        let to = write_stream.peer_addr()?;

        trace!("Wrote {:#?}/{:#?} to {:#}", bytes.len(), bytes_written, to);

        Ok(())
    }

    async fn handle_peer_stream(read_stream: OwnedReadHalf, tx: mpsc::Sender<ManReq>) {
        loop {
            let mut buf = [0; 4096];

            read_stream.readable().await.unwrap();

            match read_stream.try_read(&mut buf) {
                Ok(0) => {
                    trace!("0 bytes read, closing connection");
                    break;
                }
                Ok(bytes_read) => {
                    trace!("Read {:?} bytes with the following message:", bytes_read);

                    let msg: Message = deserialize(&buf).unwrap();

                    tx.send(ManReq::Broadcast(msg)).await.unwrap();
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

impl Client {
    fn new(id: &str) -> anyhow::Result<Self> {
        // Get IP and port
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);

        let (tx, history) = ClientManager::init();

        let client = Self {
            id: Id(id.to_owned()),
            socket: SocketAddr::new(ip, port),
            history,
            tx,
        };

        Ok(client)
    }


    async fn listen(&mut self) {
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
}

fn prompt(name: &str) -> String {
    let mut line = String::new();

    loop {
        print!("{}", name);
        std::io::stdout().flush().unwrap();
        std::io::stdin()
            .read_line(&mut line)
            .expect("Error: Could not read a line");

        if line.trim() == "" {
            continue;
        } else {
            return line.trim().to_string();
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    println!("Please type in a user name:");
    let username = prompt("");

    let mut client = Client::new(&username)?;
    client.listen().await;
    let tx = client.tx;

    println!("Accepting connections on: {:#}", client.socket.to_string());
    println!("Available commands are: ':connect <ip:port>, :exit'");

    loop {
        let input = prompt("");

        let command = input.split_whitespace().next();
        let args: String = input.split_whitespace().skip(1).collect();

        match command {
            Some(":connect") => {
                let peer = skip_fail!(Peer::new(&args));

                let conn = skip_fail!(TcpStream::connect(peer.socket).await);
                skip_fail!(tx.send(ManReq::Connect(conn)).await);
            }
            Some(":peers") => {
                skip_fail!(tx.send(ManReq::ListPeers).await);
            }
            Some(":exit") => {
                break;
            }
            Some(_) => {
                let msg = Message {
                    from: client.id.clone(),
                    content: input.clone(),
                };
                skip_fail!(tx.send(ManReq::Broadcast(msg)).await);
            }
            _ => (),
        }
    }

    Ok(())
}
