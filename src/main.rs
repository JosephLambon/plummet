pub mod book;
mod engine;
use std::{process, thread};

use tokio::runtime::Runtime;

use book::*;

use tracing::{Level, info};

use chrono::Local;
use core::time::Duration;
use rust_decimal::dec;

use engine::*;

fn main() {
    tracing_subscriber::fmt::fmt()
        .with_max_level(Level::TRACE)
        .init();

    let rt = Runtime::new().unwrap();

    info!("Welcome. This is Joe's Order Book.");
    info!("==================================\n\n");

    let mut engine = Engine::new();
    engine.add_instrument(InstrumentKey::Btc);
    engine.add_instrument(InstrumentKey::Eth);

    let bid1 = LimitOrder {
        id: 1,
        time_placed: Local::now(),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let bid2 = LimitOrder {
        id: 2,
        time_placed: Local::now(),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let bid3 = LimitOrder {
        id: 3,
        time_placed: Local::now(),
        limit_price: dec!(1234.5320),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let ask1 = LimitOrder {
        id: 4,
        time_placed: Local::now(),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
    };
    let ask2 = LimitOrder {
        id: 5,
        time_placed: Local::now(),
        limit_price: dec!(1123.5696),
        quantity: dec!(50),
        side: Side::Sell,
    };
    let ask3 = LimitOrder {
        id: 6,
        time_placed: Local::now(),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
    };

    let tx_btc = engine
        .senders
        .get(&InstrumentKey::Btc)
        .unwrap_or_else(|| process::exit(1))
        .clone();

    let tx_eth = engine
        .senders
        .get(&InstrumentKey::Eth)
        .unwrap_or_else(|| process::exit(1))
        .clone();

    let bids_btc = [bid1, bid2, bid3];
    let asks_btc = [ask1, ask2, ask3];

    let bids_eth = bids_btc.clone();
    let asks_eth = asks_btc.clone();

    let btc = async move {
        for bid in bids_btc {
            let _ = tx_btc.send(bid.clone());
        }

        for ask in asks_btc {
            let _ = tx_btc.send(ask.clone());
        }
    };

    let eth = async move {
        for bid in bids_eth {
            let _ = tx_eth.send(bid.clone());
        }

        for ask in asks_eth {
            let _ = tx_eth.send(ask.clone());
        }
    };

    info!("\n\nSPAWNING THREADS!\n");
    rt.spawn(btc);
    rt.spawn(eth);

    thread::sleep(Duration::from_secs(10));
    info!("\n\nShutting down\n");
}
