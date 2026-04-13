// Declare book module

use std::{process, thread};

use limit_order_book::{
    book::{LimitOrder, OrderState, Side},
    engine::{Engine, EngineCommand, EngineEvent, InstrumentKey},
};

use tokio::runtime::Runtime;

use tracing::{Level, info};

use chrono::Local;
use core::time::Duration;
use rust_decimal::dec;

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
        instrument: InstrumentKey::Btc,
        id: 1,
        state: OrderState::New,
        placed_at: Local::now(),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
        quantity_traded: dec!(0),
        quantity_remaining: dec!(50),
    };
    let bid2 = LimitOrder {
        instrument: InstrumentKey::Btc,
        id: 2,
        state: OrderState::New,
        placed_at: Local::now(),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
        quantity_traded: dec!(0),
        quantity_remaining: dec!(50),
    };
    let bid3 = LimitOrder {
        instrument: InstrumentKey::Btc,
        id: 3,
        state: OrderState::New,
        placed_at: Local::now(),
        limit_price: dec!(1234.5320),
        quantity: dec!(50),
        side: Side::Buy,
        quantity_traded: dec!(0),
        quantity_remaining: dec!(50),
    };
    let ask1 = LimitOrder {
        instrument: InstrumentKey::Btc,
        id: 4,
        state: OrderState::New,
        placed_at: Local::now(),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
        quantity_traded: dec!(0),
        quantity_remaining: dec!(50),
    };
    let ask2 = LimitOrder {
        instrument: InstrumentKey::Btc,
        id: 5,
        state: OrderState::New,
        placed_at: Local::now(),
        limit_price: dec!(1123.5696),
        quantity: dec!(50),
        side: Side::Sell,
        quantity_traded: dec!(0),
        quantity_remaining: dec!(50),
    };
    let ask3 = LimitOrder {
        instrument: InstrumentKey::Btc,
        id: 6,
        state: OrderState::New,
        placed_at: Local::now(),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
        quantity_traded: dec!(0),
        quantity_remaining: dec!(50),
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
            let _ = tx_btc.send(EngineCommand::PlaceOrder(bid.clone()));
        }

        for ask in asks_btc {
            let _ = tx_btc.send(EngineCommand::PlaceOrder(ask.clone()));
        }

        thread::sleep(Duration::from_secs(2));
        info!("\n\nShutting down: BTC\n");
        let _ = tx_btc.send(EngineCommand::Shutdown);
    };

    let eth = async move {
        for mut bid in bids_eth {
            bid.instrument = InstrumentKey::Eth;
            let _ = tx_eth.send(EngineCommand::PlaceOrder(bid));
        }

        for mut ask in asks_eth {
            ask.instrument = InstrumentKey::Eth;
            let _ = tx_eth.send(EngineCommand::PlaceOrder(ask.clone()));
        }
        thread::sleep(Duration::from_secs(2));
        info!("\n\nShutting down: ETH\n");
        let _ = tx_eth.send(EngineCommand::Shutdown);
    };

    info!("\n\nSPAWNING THREADS!\n");
    rt.spawn(btc);
    rt.spawn(eth);

    thread::sleep(Duration::from_secs(5));

    let mut audit_log: Vec<EngineEvent> = vec![];

    while let Ok(event) = engine.events.recv() {
        audit_log.push(event.clone());

        if let EngineEvent::Shutdown = event {
            println!("\n\n ENGINE SHUTTING DOWN... \n\n");
            println!("\n\n Final Audit log: {:#?} \n\n", audit_log);
            break;
        }
    }

    info!("SIMULATION COMPLETE.\n");
    info!("===================\n\n");
}
