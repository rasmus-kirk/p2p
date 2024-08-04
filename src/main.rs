#![feature(is_some_and)]

use std::{io::Write, net::SocketAddr, str::FromStr, thread::sleep, time::Duration};

mod types;
mod client;
mod peer;
mod test;

use peer::*;
use types::*;
use client::*;

#[macro_use]
extern crate log;

#[macro_use]
mod macros;

fn prompt(name: &str) -> String {
    let mut line = String::new();

    loop {
        print!("{}", name);
        skip_fail!(std::io::stdout().flush());
        skip_fail!(std::io::stdin().read_line(&mut line));

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

    let node= Node::new(&username).await?;

    sleep(Duration::from_secs(1));

    println!("Accepting connections on: {:#}", node.get_address().to_string());
    println!("Available commands are: ':connect <ip:port>, :peers, :balances, :exit, :send <to> <amount>'");

    loop {
        let input = prompt("");
        let input: Vec<&str> = input.split_whitespace().collect();

        match input.get(0) {
            Some(&":connect") => {
                verify_len!(":connect", input.len(), 2);

                let peer = skip_fail!(SocketAddr::from_str(input[1].trim()));
                skip_fail!(node.connect(peer).await);
            }
            Some(&":peers") => {
                verify_len!(":peers", input.len(), 1);

                println!("{:?}: {:?}", node.get_peers().len(), node.get_peers());
            }
            Some(&":balances") => {
                verify_len!(":balances", input.len(), 1);

                println!("{:?}", node.get_ledger());
            }
            Some(&":exit") => {
                verify_len!(":exit", input.len(), 1);

                break;
            }
            Some(&":send") => {
                //verify_len!(":send", input.len(), 3);
                //let from: Id.from(input[1].to_string())
                //let to = Id(input[2].to_string());
                //let amount = skip_fail!(input[3].parse());
                //node.send(to, amount).await;
            }
            Some(_) => {
                println!("Available commands are: ':connect <ip:port>, :peers, :balances, :exit, :send <to> <amount>'");
            }
            _ => (),
        }
    }

    Ok(())
}
