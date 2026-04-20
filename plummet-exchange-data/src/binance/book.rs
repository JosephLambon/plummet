use std::collections::BTreeMap;

use rust_decimal::{Decimal, dec};
use tracing::{Level as LogLevel, instrument, trace};

use crate::{Level, OrderBookUpsert, Side};

#[derive(Default)]
pub struct L2OrderBook {
    pub last_update_id: u64,
    pub asks: BTreeMap<Decimal, Decimal>,
    pub bids: BTreeMap<Decimal, Decimal>,
}

impl L2OrderBook {
    pub fn new(
        last_update_id: u64,
        bids: BTreeMap<Decimal, Decimal>,
        asks: BTreeMap<Decimal, Decimal>,
    ) -> Self {
        Self {
            last_update_id,
            asks,
            bids,
        }
    }
}

impl OrderBookUpsert for L2OrderBook {
    #[instrument(level = LogLevel::TRACE, skip_all)]
    fn upsert(&mut self, level: Level, side: Side) {
        trace!(
            side = ?side,
            level = %level.price,
            qty = %level.qty,
            "Pushing order to price level"
        );

        let side = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        let entry = side
            .entry(level.price)
            .and_modify(|q| *q = level.qty)
            .or_insert(level.qty);

        if *entry <= dec!(0) {
            side.remove(&level.price);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_bid_inserts_new_level() {
        let bid = Level {
            price: dec!(0.920041),
            qty: dec!(24.123),
        };

        let mut book = L2OrderBook::default();

        book.upsert(bid, Side::Bid);

        assert!(book.bids.len() == 1);
        assert!(book.asks.len() == 0);
    }
    #[test]
    fn upsert_bid_overwrites_existing_level() {
        let mut bid = Level {
            price: dec!(0.920041),
            qty: dec!(24.123),
        };

        let mut book = L2OrderBook::default();

        book.upsert(bid, Side::Bid);

        assert!(book.bids.len() == 1);
        assert_eq!(*book.bids.get(&dec!(0.920041)).unwrap(), dec!(24.123));
        assert!(book.asks.len() == 0);

        bid.qty = dec!(5.00);
        book.upsert(bid, Side::Bid);

        assert!(book.bids.len() == 1);
        assert_eq!(*book.bids.get(&dec!(0.920041)).unwrap(), dec!(5.00));
        assert!(book.asks.len() == 0);
    }
    #[test]
    fn upsert_bid_removes_level_when_emptied() {
        let mut bid = Level {
            price: dec!(0.920041),
            qty: dec!(24.123),
        };

        let mut book = L2OrderBook::default();

        book.upsert(bid, Side::Bid);

        assert!(book.bids.len() == 1);
        assert_eq!(*book.bids.get(&dec!(0.920041)).unwrap(), dec!(24.123));
        assert!(book.asks.len() == 0);

        bid.qty = dec!(0.00);
        book.upsert(bid, Side::Bid);

        assert!(book.asks.len() == 0);
        assert!(book.bids.len() == 0);
    }
    #[test]
    fn upsert_ask_inserts_new_level() {
        let ask = Level {
            price: dec!(0.920041),
            qty: dec!(24.123),
        };

        let mut book = L2OrderBook::default();

        book.upsert(ask, Side::Ask);

        assert!(book.bids.len() == 0);
        assert!(book.asks.len() == 1);
    }
    #[test]
    fn upsert_ask_overwrites_existing_level() {
        let mut ask = Level {
            price: dec!(0.920041),
            qty: dec!(24.123),
        };

        let mut book = L2OrderBook::default();

        book.upsert(ask, Side::Ask);

        assert!(book.asks.len() == 1);
        assert_eq!(*book.asks.get(&dec!(0.920041)).unwrap(), dec!(24.123));
        assert!(book.bids.len() == 0);

        ask.qty = dec!(5.00);
        book.upsert(ask, Side::Ask);

        assert!(book.asks.len() == 1);
        assert_eq!(*book.asks.get(&dec!(0.920041)).unwrap(), dec!(5.00));
        assert!(book.bids.len() == 0);
    }
    #[test]
    fn upsert_ask_removes_level_when_emptied() {
        let mut ask = Level {
            price: dec!(0.920041),
            qty: dec!(24.123),
        };

        let mut book = L2OrderBook::default();

        book.upsert(ask, Side::Ask);

        assert!(book.asks.len() == 1);
        assert_eq!(*book.asks.get(&dec!(0.920041)).unwrap(), dec!(24.123));
        assert!(book.bids.len() == 0);

        ask.qty = dec!(0.00);
        book.upsert(ask, Side::Ask);

        assert!(book.asks.len() == 0);
        assert!(book.bids.len() == 0);
    }
}
