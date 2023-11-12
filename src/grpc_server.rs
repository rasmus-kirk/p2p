use std::collections::HashMap;
use std::net::SocketAddr;

use tonic::{transport::Server, Request, Response, Status};

use grpc::proto_server::{Proto, ProtoServer};
use grpc::*;

pub mod grpc {
    tonic::include_proto!("p2p");
}

#[derive(Debug, Default)]
pub struct Grpc {}

impl Grpc {
    pub async fn new(addr: SocketAddr) -> anyhow::Result<()> {
        let grpc = Grpc::default();

        Server::builder()
            .add_service(ProtoServer::new(grpc))
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl Proto for Grpc {
    async fn get_peers(
        &self,
        request: Request<Unit>,
    ) -> Result<Response<Peers>, Status> {
        println!("Got a request: {:?}", request);

        let peers = Peers {
            peers: Vec::<Peer>::new(),
        };

        Ok(Response::new(peers))
    }

    async fn send(
        &self,
        request: Request<Transaction>,
    ) -> Result<Response<Unit>, Status> {
        println!("Got a request: {:?}", request);

        Ok(Response::new(Unit {}))
    }

    async fn get_ledger(
        &self,
        request: Request<Unit>,
    ) -> Result<Response<Ledger>, Status> {
        println!("Got a request: {:?}", request);

        let ledger = Ledger {
            ledger: HashMap::new()
        };

        Ok(Response::new(ledger))
    }
}
