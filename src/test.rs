#[cfg(test)]
mod tests {
    use std::time::Duration;
    use tokio::time::sleep;

    use crate::{client::*, types::{AccountTransaction, Amount, Timestamp}};
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

        let node_a = Node::new("NodeA").await?;
        info!("Accepting connections for {:?} on: {:#}", node_a.get_node_name(), node_a.socket);
        let node_b = Node::new("NodeB").await?;
        info!("Accepting connections for {:?} on: {:#}", node_b.get_node_name(), node_b.socket);
        let node_c = Node::new("NodeC").await?;
        info!("Accepting connections for {:?} on: {:#}", node_b.get_node_name(), node_b.socket);

        let a_keys = node_a.gen_keys();
        let b_keys = node_b.gen_keys();
        let c_keys = node_c.gen_keys();

        node_a.connect(node_b.get_address()).await?;
        node_b.connect(node_c.get_address()).await?;

        let trx_1 = AccountTransaction {
            from: a_keys.public,
            to: b_keys.public,
            amount: Amount(100),
            timestamp: Timestamp::since_unix().unwrap(),
        };
        let strx_1 = a_keys.private.sign(trx_1).unwrap();

        let trx_2 = AccountTransaction {
            from: b_keys.public,
            to: c_keys.public,
            amount: Amount(150),
            timestamp: Timestamp::since_unix().unwrap(),
        };
        let strx_2 = b_keys.private.sign(trx_2).unwrap();

        node_a.send(strx_1).await;
        node_b.send(strx_2).await;
        sleep(_LONG).await;

        println!("Peers:");
        println!("NodeA: {:?}", node_a.get_peers());
        println!("NodeB: {:?}", node_b.get_peers());
        println!("NodeC: {:?}\n", node_c.get_peers());

        println!("Histories:");
        println!("NodeA: {:?}", node_a.get_history());
        println!("NodeB: {:?}", node_b.get_history());
        println!("NodeC: {:?}\n", node_c.get_history());

        println!("Ledgers:");
        println!("NodeA: {:?}", node_a.get_ledger());
        println!("NodeB: {:?}", node_b.get_ledger());
        println!("NodeC: {:?}\n", node_c.get_ledger());

        assert!(node_a.get_peers().len() != 0);
        assert_eq!(node_a.get_peers(), node_b.get_peers());
        
        assert!(node_a.get_ledger().len() != 0);
        assert_eq!(node_a.get_ledger(), node_b.get_ledger());

        println!("Balances:");
        println!("Acc A: {:?}", node_a.get_balance(&a_keys.public));
        println!("Acc B: {:?}", node_a.get_balance(&b_keys.public));
        println!("Acc C: {:?}\n", node_a.get_balance(&c_keys.public));

        assert!(node_a.get_balance(&a_keys.public) == Amount(-100));
        assert!(node_b.get_balance(&a_keys.public) == Amount(-100));
        assert!(node_c.get_balance(&a_keys.public) == Amount(-100));

        assert!(node_a.get_balance(&b_keys.public) == Amount(-50));
        assert!(node_b.get_balance(&b_keys.public) == Amount(-50));
        assert!(node_c.get_balance(&b_keys.public) == Amount(-50));

        assert!(node_a.get_balance(&c_keys.public) == Amount(150));
        assert!(node_b.get_balance(&c_keys.public) == Amount(150));
        assert!(node_c.get_balance(&c_keys.public) == Amount(150));

        //assert!(false);

        Ok(())
    }
    
}
