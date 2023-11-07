#![feature(let_chains)]

use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use serde::Deserialize;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use rand::Rng;
use bincode::{serialize, deserialize};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::tcp::OwnedWriteHalf;
use std::fmt;

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
        Ok(Self { socket: SocketAddr::from_str(socket_string.trim())? })
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
    peers: Peers,
    conns: Arc<Mutex<Vec<OwnedWriteHalf>>>,
    history: History
}

type History = Arc<Mutex<HashMap<Message, bool>>>;
type Peers = Arc<Mutex<HashMap<Peer, OwnedWriteHalf>>>;

impl Client {
    fn new(id: &str) -> anyhow::Result<Self> {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);

        let client = Self {
            id: Id(id.to_owned()),
            socket: SocketAddr::new(ip, port),
            peers: Arc::new(Mutex::new(HashMap::<Peer, OwnedWriteHalf>::new())),
            conns: Arc::new(Mutex::new(Vec::<OwnedWriteHalf>::new())),
            history: Arc::new(Mutex::new(HashMap::<Message, bool>::new()))
        };

        Ok(client)
    }

    async fn listen(&mut self) {
        let socket = self.socket;
        let listener = TcpListener::bind(socket).await.unwrap();
        let history = self.history.clone();
        let peers = self.peers.clone();
        let conns = self.conns.clone();

        tokio::spawn(async move {
            loop {
                let mut history = history.clone();
                let peers = peers.clone();
                let conns = conns.clone();

                let (stream, addr) = listener.accept().await.unwrap();

                info!("listen: accepted tcp stream from {:?}, handling:", addr);

                let peer = Peer { socket: addr };
                let (read_stream, write_stream) = stream.into_split();
                match peers.lock() {
                    Ok(mut o) => { o.insert(peer, write_stream); },
                    Err(_) => continue,
                };
                let p = peers.clone();

                tokio::spawn(async move {
                    Self::handle_peer_stream(read_stream, &mut conns, &mut history).await;
                });
            }
        });
    }

    async fn handle_peer_stream(stream: OwnedReadHalf, peers: &Arc<Mutex<Vec<OwnedWriteHalf>>>, history: &mut History) {
        loop {
            let mut buf = [0; 4096];

            match stream.try_read(&mut buf) {
                Ok(0) => {
                    info!("0 bytes read, closing connection");
                    break;
                }
                Ok(bytes_read) => {
                    info!("Read {:?} bytes with the following message:", bytes_read);

                    let msg: Message = deserialize(&buf).unwrap();

                    println!("{:}", msg);

                    Self::broadcast(msg, peers, history).await;
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

    async fn update_history(msg: Message, history: &mut History) {
        match history.lock() {
            Ok(mut o) => { o.insert(msg, false); },
            Err(e) => error!("{:?}", e)
        }
    }

    async fn broadcast(msg: Message, peers: Arc<Mutex<Vec<OwnedWriteHalf>>>, history: &mut History) {
        Self::update_history(msg.clone(), history).await;

        let peers = match peers.lock() {
            Ok(o) => o,
            Err(e) => { error!("{:?}", e); return () }
        };
        let p = peers.clone();
        for conn in peers.into_iter() {
            let bytes = match serialize(&msg) {
                Ok(o) => o,
                Err(e) => { error!("{e}"); break; }
            };

            match conn.writable().await {
                Ok(o) => o,
                Err(e) => { error!("{e}"); break; }
            };

            let bytes_written = match conn.try_write(&bytes) {
                Ok(o) => o,
                Err(e) => { error!("{:#?}", e); break; }
            };

            info!("Wrote {:#?}/{:#?} to {:#}", bytes.len(), bytes_written, conn.peer_addr().unwrap());
        }
    }

    async fn connect(&self, to: &Peer) -> anyhow::Result<TcpStream> {
        let conn = TcpStream::connect(to.socket).await?;
        Ok(conn)
    }
}

fn prompt(name: &str) -> String {
    let mut line = String::new();

    loop {
        print!("{}", name);
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut line).expect("Error: Could not read a line");

        if line.trim() == "" {
            continue;
        } else {
            return line.trim().to_string()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()>{
    env_logger::init();

    println!("Please type in a user name:");
    let username = prompt("> ");
    
    let mut client = Client::new(&username)?;
    client.listen().await;
    let mut conns = Vec::<TcpStream>::new();

    println!("Accepting connections on: {:#}", client.socket.to_string());
    println!("Available commands are: ':connect <ip:port>, :exit'");

    loop {
        info!("{:?}", client.history);
        let input = prompt("> ");
        
        let command = input.split_whitespace().next();
        let args: String = input.split_whitespace().skip(1).collect();

        match command {
            Some(":connect") => {
                let peer = match Peer::new(&args) {
                    Ok(o) => o,
                    Err(e) => { 
                        error!("{:#}: {:#}", e, args);
                        continue;
                    }
                };

                conns.push(client.connect(&peer).await?);

                println!("Established connection with: {:#}", peer.socket);

                //client.peers.insert(peer);
            }
            Some(":exit") => {
                break;
            }
            Some(_) => {
                for conn in &conns {
                    let msg = Message {
                        from: client.id.clone(),
                        content: input.clone()
                    };

                    let bytes = serialize(&msg)?;

                    conn.writable().await?;
                    let bytes_written = conn.try_write(&bytes)?;

                    info!("Wrote {:#?}/{:#?} to {:#}", bytes.len(), bytes_written, conn.peer_addr()?);
                }
            }
            _ => ()
        }
    }

    Ok(())
}

