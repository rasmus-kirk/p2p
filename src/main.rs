#![feature(let_chains)]

use bincode::{deserialize, serialize};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
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
    tx: mpsc::Sender<RecReq>,
}

enum RecReq {
    Msg(Message),
    Conn(TcpStream),
}

impl Client {
    fn new(id: &str) -> anyhow::Result<Self> {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = rand::thread_rng().gen_range(20000..60000);
        let (tx, rx) = mpsc::channel::<RecReq>(100);
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            Self::start_reciever(rx, tx_clone).await;
        });

        let client = Self {
            id: Id(id.to_owned()),
            socket: SocketAddr::new(ip, port),
            tx,
        };

        Ok(client)
    }

    async fn start_reciever(mut rx: mpsc::Receiver<RecReq>, tx: mpsc::Sender<RecReq>) {
        let mut history = HashMap::<Message, bool>::new();
        let mut peers = Vec::<OwnedWriteHalf>::new();

        loop {
            match rx.recv().await {
                Some(RecReq::Msg(msg)) => {
                    if history.get(&msg) != Some(&true) {
                        for conn in peers.iter() {
                            let bytes = match serialize(&msg) {
                                Ok(o) => o,
                                Err(e) => {
                                    error!("{e}");
                                    break;
                                }
                            };

                            match conn.writable().await {
                                Ok(o) => o,
                                Err(e) => {
                                    error!("{e}");
                                    break;
                                }
                            };

                            let bytes_written = match conn.try_write(&bytes) {
                                Ok(o) => o,
                                Err(e) => {
                                    error!("{e}");
                                    break;
                                }
                            };

                            trace!(
                                "Wrote {:#?}/{:#?} to {:#}",
                                bytes.len(),
                                bytes_written,
                                conn.peer_addr().unwrap()
                            );
                        }
                    }

                    history.insert(msg, true);
                }
                Some(RecReq::Conn(stream)) => {
                    let tx = tx.clone();
                    let (read_stream, write_stream) = stream.into_split();

                    peers.push(write_stream);

                    info!("Established connection with: {:#}", read_stream.peer_addr().unwrap());

                    tokio::spawn(async move {
                        Self::handle_peer_stream(read_stream, tx).await;
                    });
                }
                None => break,
            }
        }
    }

    async fn listen(&mut self) {
        let listener = TcpListener::bind(self.socket).await.unwrap();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            loop {
                let (stream, addr) = listener.accept().await.unwrap();

                info!("listen: accepted tcp stream from {:?}, handling:", addr);

                tx.send(RecReq::Conn(stream)).await.unwrap();
            }
        });
    }

    async fn handle_peer_stream(read_stream: OwnedReadHalf, tx: mpsc::Sender<RecReq>) {
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

                    println!("{:}", msg);

                    tx.send(RecReq::Msg(msg)).await.unwrap();
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
                let peer = match Peer::new(&args) {
                    Ok(o) => o,
                    Err(e) => {
                        error!("{:#}: {:#}", e, args);
                        continue;
                    }
                };

                let conn = TcpStream::connect(peer.socket).await?;
                tx.send(RecReq::Conn(conn)).await.unwrap();
            }
            Some(":exit") => {
                break;
            }
            Some(_) => {
                let msg = Message {
                    from: client.id.clone(),
                    content: input.clone(),
                };
                tx.send(RecReq::Msg(msg)).await.unwrap();
            }
            _ => (),
        }
    }

    Ok(())
}
