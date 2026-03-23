use std::collections::{BTreeMap, VecDeque};

use rust_decimal::Decimal;

use chrono::prelude::*;

pub struct OrderBook {
    pub asks: BTreeMap<Decimal, VecDeque<LimitOrder>>,
    pub bids: BTreeMap<Decimal, VecDeque<LimitOrder>>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, limit_order: LimitOrder) {
        match limit_order.side {
            Side::Buy => {
                OrderBook::push_back_or_create_price_level(&mut self.bids, limit_order);
            }
            Side::Sell => {
                OrderBook::push_back_or_create_price_level(&mut self.asks, limit_order);
            }
        }
    }

    fn push_back_or_create_price_level(
        order_book_side: &mut BTreeMap<Decimal, VecDeque<LimitOrder>>,
        limit_order: LimitOrder,
    ) {
        order_book_side
            .entry(limit_order.limit_price)
            .or_default()
            .push_back(limit_order);

        println!("Updated Order Book Side: {:#?}\n", order_book_side);
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct LimitOrder {
    pub time_placed: DateTime<Local>,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Side {
    Buy,
    Sell,
}

#[cfg(test)]
mod tests {
    use rust_decimal::dec;

    use super::*;

    #[test]
    fn new_initialises_correctly() {
        let order_book = OrderBook::new();

        assert_eq!(BTreeMap::new(), order_book.asks);
        assert_eq!(BTreeMap::new(), order_book.bids);
    }

    #[test]
    fn insert_adds_buy_side_orders_to_bids() {
        let bid1 = LimitOrder {
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Buy,
        };
        let bid2 = LimitOrder {
            limit_price: dec!(1200.2136),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Buy,
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
    fn insert_updates_bid_queue_for_existing_price_level() {
        let bid1 = LimitOrder {
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Buy,
        };
        let bid2 = LimitOrder {
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Buy,
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
    fn insert_adds_sell_side_orders_to_asks() {
        let ask1 = LimitOrder {
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Sell,
        };
        let ask2 = LimitOrder {
            limit_price: dec!(1200.2136),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Sell,
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
    fn insert_updates_ask_queue_for_existing_price_level() {
        let ask1 = LimitOrder {
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Sell,
        };
        let ask2 = LimitOrder {
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            time_placed: Local::now(),
            side: Side::Sell,
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
    fn insert_routes_buy_and_sell_orders_to_correct_sides() {
        panic!()
    }

    #[test]
    fn insert_preserves_fifo_order_within_price_level() {
        panic!()
    }
}
