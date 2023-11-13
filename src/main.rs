use std::io::Write;

mod types;
mod client;

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

    let mut client = Client::new(&username)?;
    client.listen().await;

    println!("Accepting connections on: {:#}", client.get_address().to_string());
    println!("Available commands are: ':connect <ip:port>, :peers, :balances, :exit, :send <to> <amount>'");

    loop {
        let input = prompt("");

        let input: Vec<&str> = input.split_whitespace().collect();

        match input.get(0) {
            Some(&":connect") => {
                verify_len!(":connect", input.len(), 2);

                let peer = skip_fail!(Peer::new(input[1]));
                skip_fail!(client.connect(peer).await);
            }
            Some(&":peers") => {
                verify_len!(":peers", input.len(), 1);

                skip_fail!(client.list_peers().await);
            }
            Some(&":balances") => {
                verify_len!(":balances", input.len(), 1);

                skip_fail!(client.list_balances().await);
            }
            Some(&":exit") => {
                verify_len!(":exit", input.len(), 1);

                break;
            }
            Some(&":send") => {
                verify_len!(":send", input.len(), 3);

                let to = Id(input[1].to_string());
                let amount = Amount(skip_fail!(input[2].parse()));
                skip_fail!(client.send(to, amount).await);
            }
            Some(_) => {
                println!("Available commands are: ':connect <ip:port>, :peers, :balances, :exit, :send <to> <amount>'");
            }
            _ => (),
        }
    }

    Ok(())
}
