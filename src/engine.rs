use std::{
    collections::{HashMap, hash_map::Entry},
    sync::mpsc,
    thread,
};

use tracing::info;

use crate::book::{LimitOrder, OrderBook};

#[derive(Eq, PartialEq, Hash)]
pub enum InstrumentKey {
    Btc,
    Eth,
}

pub struct Engine {
    pub senders: HashMap<InstrumentKey, mpsc::Sender<LimitOrder>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            senders: HashMap::new(),
        }
    }

    pub fn add_instrument(&mut self, ticker_symbol: InstrumentKey) {
        if let Entry::Vacant(entry) = self.senders.entry(ticker_symbol) {
            let (tx, rx) = mpsc::channel::<LimitOrder>();

            entry.insert(tx);

            // Listen for orders until channel closes
            thread::spawn(move || {
                let mut order_book = OrderBook::new();

                while let Ok(order) = rx.recv() {
                    order_book.insert(order);
                    if order_book.check_match() {
                        info!("MATCH FOUND");
                    };
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::TryRecvError;

    use super::*;

    #[test]
    fn add_instrument_inserts_to_senders() {
        let mut engine = Engine::new();

        engine.add_instrument(InstrumentKey::Btc);

        assert_eq!(engine.senders.len(), 1);
    }

    #[test]
    fn add_instrument_does_not_overwrite_existing() {
        let mut engine = Engine::new();

        let (tx, rx) = mpsc::channel();

        engine.senders.insert(InstrumentKey::Btc, tx);

        engine.add_instrument(InstrumentKey::Btc);

        assert_ne!(rx.try_recv().err().unwrap(), TryRecvError::Disconnected);
    }

    #[test]
    fn add_instrument_inserts_separate_thread_per_instrument() {
        let mut engine = Engine::new();

        engine.add_instrument(InstrumentKey::Btc);
        engine.add_instrument(InstrumentKey::Eth);

        assert_eq!(engine.senders.len(), 2);
    }
}
