use std::collections::BTreeMap;

use crypto_orderbook::DepthUpdate;
use crypto_orderbook::Market;
use crypto_orderbook::orderbook::sync::{Live, live};
use rust_decimal::{Decimal, dec};
use tracing::{Level as LogLevel, instrument, trace};

#[derive(Debug, Copy, Clone)]
pub struct Level {
    pub price: Decimal,
    pub qty: Decimal,
}

#[derive(Debug, Copy, Clone)]
pub enum Side {
    Bid,
    Ask,
}

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

    #[instrument(level = LogLevel::TRACE, skip_all)]
    pub fn upsert(&mut self, level: Level, side: Side) {
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

    /// Applies a Binance spot diff-depth event only if it continues from
    /// `last_update_id`. Events with `u <= lastUpdateId` are ignored; an event
    /// that skips ids clears the book, which then needs a fresh snapshot.
    pub fn apply_update(&mut self, update: &DepthUpdate) -> Live {
        let verdict = live(Market::Spot, self.last_update_id, update);
        match verdict {
            Live::Apply => {
                for l in &update.bids {
                    self.upsert(
                        Level {
                            price: l.price,
                            qty: l.quantity,
                        },
                        Side::Bid,
                    );
                }
                for l in &update.asks {
                    self.upsert(
                        Level {
                            price: l.price,
                            qty: l.quantity,
                        },
                        Side::Ask,
                    );
                }
                self.last_update_id = update.last_update_id;
            }
            Live::Gap { .. } => {
                self.bids.clear();
                self.asks.clear();
            }
            Live::Ignore => {}
        }
        verdict
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

    #[test]
    fn apply_update_follows_binance_sequencing() {
        let event = |first: u64, last: u64, price: &str, qty: &str| {
            crypto_orderbook::parse_depth_update(&format!(
                r#"{{"e":"depthUpdate","E":1,"s":"BTCUSDT","U":{first},"u":{last},"b":[["{price}","{qty}"]],"a":[]}}"#
            ))
            .unwrap()
        };
        let mut bids = BTreeMap::new();
        bids.insert(dec!(100), dec!(1));
        let mut book = L2OrderBook::new(160, bids, BTreeMap::new());

        // Already in the snapshot: discarded.
        assert_eq!(
            book.apply_update(&event(150, 160, "100", "0")),
            Live::Ignore
        );
        assert_eq!(book.bids.get(&dec!(100)), Some(&dec!(1)));

        // Bridges lastUpdateId: applied.
        assert_eq!(book.apply_update(&event(158, 162, "100", "2")), Live::Apply);
        assert_eq!(book.bids.get(&dec!(100)), Some(&dec!(2)));
        assert_eq!(book.last_update_id, 162);

        // 163..=164 was dropped: the book is cleared instead of served.
        assert_eq!(
            book.apply_update(&event(165, 165, "101", "1")),
            Live::Gap {
                expected: 163,
                got: 165
            }
        );
        assert!(book.bids.is_empty());
    }
}
