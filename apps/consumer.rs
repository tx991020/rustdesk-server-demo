use anyhow::{Context, Result};
use async_channel::{bounded, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::option::Option::Some;
use std::sync::Arc;
use tokio::signal::ctrl_c;

use tokio::time;

use hbb_common::anyhow::{Context, Result};
use hbb_common::tokio;
use hbb_common::tokio::sync::Mutex;
use hbb_common::tokio::time::Duration;

type Tx = Sender<String>;

/// Shorthand for the receive half of the message channel.
type Rx = Receiver<String>;

struct Shared {
    receivers_18: HashMap<String, Rx>,
    receivers_19: HashMap<String, Rx>,
    kv: HashMap<String, String>,
    vk: HashMap<String, String>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            receivers_18: HashMap::new(),
            receivers_19: HashMap::new(),

            kv: HashMap::new(),
            vk: HashMap::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let state = Arc::new(Mutex::new(Shared::new()));

    tokio::spawn(fx1(state.clone()));
    tokio::spawn(fx2(state.clone()));
    tokio::spawn(fx5(state.clone()));

    ctrl_c().await?;

    Ok(())
}

async fn fx1(state: Arc<Mutex<Shared>>) {
    let (tx, mut rx) = unbounded::<String>();

    {
        let mut s = state.lock().await;
        s.receivers_19.insert("A".to_string(), rx);
    }
    println!("{}", "xxxx");

    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        tx.send("111".to_string()).await;
        tx.send("222".to_string()).await;
        tx.send("3333".to_string()).await;
        interval.tick().await;
    }
}

async fn fx2(state: Arc<Mutex<Shared>>) {
    let (tx, mut rx) = unbounded::<String>();
    {
        let mut s = state.lock().await;
        s.receivers_18.insert("B".to_string(), rx);
    }

    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        tx.send("444".to_string()).await;
        tx.send("555".to_string()).await;
        tx.send("666".to_string()).await;
        interval.tick().await;
    }
}

async fn defer_mutex(state: Arc<Mutex<Shared>>) -> Result<(Rx, Rx)> {
    let mut s = state.lock().await;
    let chan1 = s.receivers_18.get("B").context("none1")?;

    let chan2 = s.receivers_19.get("A").context("none2")?;
    return Ok((chan1.clone(), chan2.clone()));
}

async fn fx5(state: Arc<Mutex<Shared>>) -> Result<()> {
    println!("{}", "jjjjjjj");

    let (ch3, ch4) = defer_mutex(state).await?;

    loop {
        tokio::select! {
            Ok(t)= ch3.recv() => {
                println!("BBB {}",t)
            }
            Ok(t)= ch4.recv() => {
                 println!("AAAA{}",t)
            }
             else => {
                println!("{}","error");

            },

        }
    }
    Ok(())
}
