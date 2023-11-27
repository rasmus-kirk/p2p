#[cfg(test)]
mod tests {
    use std::time::Duration;
    use tokio::time::sleep;

    use crate::client::*;
    use log::info;

    fn log_init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    const SHORT: Duration = Duration::from_millis(100);
    const _MID:   Duration = Duration::from_millis(1000);
    const _LONG:  Duration = Duration::from_millis(2000);

    //#[tokio::test(flavor = "multi_thread")]
    #[tokio::test]
    async fn it_works() -> anyhow::Result<()> {
        log_init();
        info!("Starting test");

        let node_a = Node::new("NodeA")?;
        info!("Accepting connections for {:?} on: {:#}", node_a.id, node_a.socket);
        let node_b = Node::new("NodeB")?;
        info!("Accepting connections for {:?} on: {:#}", node_b.id, node_b.socket);
        sleep(SHORT).await;

        node_a.connect(node_b.get_address()).await?;
        sleep(SHORT).await;
        //node_b.connect(node_c.get_address()).await?;
        sleep(SHORT).await;

        node_a.send(node_b.get_id(), 10).await;
        node_a.send(node_b.get_id(), 20).await;
        sleep(_LONG).await;

        println!("Peers:");
        println!("NodeA: {:?}", node_a.get_peers());
        println!("NodeB: {:?}\n", node_b.get_peers());

        println!("Histories:");
        println!("NodeA: {:?}", node_a.get_history());
        println!("NodeB: {:?}\n", node_b.get_history());

        println!("Ledgers:");
        println!("NodeA: {:?}", node_a.get_ledger());
        println!("NodeB: {:?}\n", node_b.get_ledger());

        assert!(node_a.get_peers().len() != 0);
        assert_eq!(node_a.get_peers(), node_b.get_peers());
        
        assert!(node_a.get_ledger().len() != 0);
        assert_eq!(node_a.get_ledger(), node_b.get_ledger());

        Ok(())
    }
    
}
