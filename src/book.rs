pub mod order;
pub use order::{LimitOrder, Side};

use std::collections::{BTreeMap, VecDeque};

use tracing::{Level, debug, instrument, trace};

use rust_decimal::Decimal;

#[derive(Debug)]
pub struct OrderBook {
    pub asks: BTreeMap<Decimal, VecDeque<LimitOrder>>,
    pub bids: BTreeMap<Decimal, VecDeque<LimitOrder>>,
}

#[derive(Debug, PartialEq)]
pub struct MatchResult {
    bid_id: u64,
    ask_id: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
        }
    }

    #[instrument(level = Level::DEBUG, skip_all)]
    pub fn match_sides(&self) -> Option<MatchResult> {
        debug!("Checking for match");
        let lowest_ask = self.asks.first_key_value()?;
        let highest_bid = self.bids.last_key_value()?;

        debug!(
            bid = %highest_bid.0,
            ask = %lowest_ask.0,
        );

        // Get orders from front of price level's queues
        let bid = highest_bid.1.front()?;
        let ask = lowest_ask.1.front()?;

        if bid.limit_price >= ask.limit_price && bid.is_open() && ask.is_open() {
            Some(MatchResult {
                bid_id: bid.id,
                ask_id: ask.id,
            })
        } else {
            None
        }
    }

    #[instrument(level = Level::DEBUG, skip_all)]
    pub fn insert(&mut self, limit_order: LimitOrder) {
        debug!(
            price = %limit_order.limit_price,
            quantity = %limit_order.quantity,
            side = ?limit_order.side,
            "Inserting order ID to order book"
        );
        match limit_order.side {
            Side::Buy => {
                OrderBook::push_back_or_create_price_level(&mut self.bids, limit_order);
            }
            Side::Sell => {
                OrderBook::push_back_or_create_price_level(&mut self.asks, limit_order);
            }
        }
    }

    #[instrument(level = Level::TRACE, skip_all)]
    fn push_back_or_create_price_level(
        order_book_side: &mut BTreeMap<Decimal, VecDeque<LimitOrder>>,
        limit_order: LimitOrder,
    ) {
        trace!(
            price_level = %limit_order.limit_price,
            order_id = %limit_order.id,
            "Pushing order to price level"
        );
        order_book_side
            .entry(limit_order.limit_price)
            .or_default()
            .push_back(limit_order);
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use chrono::prelude::Local;
    use rust_decimal::dec;

    use super::*;

    #[test]
    fn new_initialises() {
        let order_book = OrderBook::new();

        assert_eq!(BTreeMap::new(), order_book.asks);
        assert_eq!(BTreeMap::new(), order_book.bids);
    }

    #[test]
    fn insert_new_bids() {
        let bid1 = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let bid2 = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2136),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(bid1);
        order_book.insert(bid2);

        assert!(order_book.asks.is_empty());
        assert_eq!(order_book.bids.len(), 2);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert!(order_book.bids.contains_key(&dec!(1200.2136)));
    }

    #[test]
    fn insert_existing_bid_level() {
        let bid1 = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let bid2 = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(bid1.clone());
        order_book.insert(bid2.clone());

        let expected = VecDeque::from([bid1, bid2]);

        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert_eq!(order_book.bids.get(&dec!(1200.2134)).unwrap(), &expected);
    }
    #[test]
    fn insert_new_asks() {
        let ask1 = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask2 = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2136),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(ask1);
        order_book.insert(ask2);

        assert!(order_book.bids.is_empty());
        assert_eq!(order_book.asks.len(), 2);
        assert!(order_book.asks.contains_key(&dec!(1200.2134)));
        assert!(order_book.asks.contains_key(&dec!(1200.2136)));
    }

    #[test]
    fn insert_existing_ask_level() {
        let ask1 = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask2 = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(ask1.clone());
        order_book.insert(ask2.clone());

        let expected = VecDeque::from([ask1, ask2]);

        assert_eq!(order_book.asks.len(), 1);
        assert!(order_book.asks.contains_key(&dec!(1200.2134)));
        assert_eq!(order_book.asks.get(&dec!(1200.2134)).unwrap(), &expected);
    }

    #[test]
    fn insert_routes_to_correct_side() {
        let bid = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2136),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(bid.clone());
        order_book.insert(ask.clone());

        let expected_bids = VecDeque::from([bid]);
        let expected_asks = VecDeque::from([ask]);

        assert_eq!(order_book.asks.len(), 1);
        assert!(order_book.asks.contains_key(&dec!(1200.2136)));
        assert_eq!(
            order_book.asks.get(&dec!(1200.2136)).unwrap(),
            &expected_asks
        );

        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert_eq!(
            order_book.bids.get(&dec!(1200.2134)).unwrap(),
            &expected_bids
        );
    }

    #[test]
    fn insert_fifo_order_within_price_level() {
        let bid1: LimitOrder = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let bid2 = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let bid3 = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(bid1.clone());
        order_book.insert(bid2.clone());
        order_book.insert(bid3.clone());

        let expected = VecDeque::from([bid1.clone(), bid2.clone(), bid3.clone()]);

        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert_eq!(
            order_book
                .bids
                .get(&dec!(1200.2134))
                .unwrap()
                .front()
                .unwrap(),
            &bid1
        );
        assert_eq!(
            order_book
                .bids
                .get(&dec!(1200.2134))
                .unwrap()
                .back()
                .unwrap(),
            &bid3
        );
        assert_eq!(order_book.bids.get(&dec!(1200.2134)).unwrap(), &expected);
    }

    #[test]
    fn match_sides_bid_above_ask() {
        let bid: LimitOrder = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        let expected = MatchResult {
            ask_id: ask.id,
            bid_id: bid.id,
        };

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides().unwrap(), expected);
    }

    #[test]
    fn match_sides_bid_matching_ask() {
        let bid: LimitOrder = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        let expected = MatchResult {
            ask_id: ask.id,
            bid_id: bid.id,
        };

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides().unwrap(), expected);
    }

    #[test]
    fn match_sides_bid_below_ask() {
        let bid: LimitOrder = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2132),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_no_bids() {
        let ask = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_no_asks() {
        let ask = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_fifo_order() {
        let bid1: LimitOrder = LimitOrder {
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let bid2: LimitOrder = LimitOrder {
            id: 2,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask1 = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };
        let ask2 = LimitOrder {
            id: 4,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            matched_quantity: dec!(0),
            remaining_quantity: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        let expected = MatchResult {
            ask_id: ask1.id,
            bid_id: bid1.id,
        };

        order_book.insert(bid1);
        order_book.insert(bid2);
        order_book.insert(ask1);
        order_book.insert(ask2);

        assert_eq!(order_book.match_sides().unwrap(), expected);
    }

    #[test]
    fn match_sides_empty_order_book() {
        let order_book = OrderBook::new();

        assert_eq!(order_book.match_sides(), None);
    }
}
