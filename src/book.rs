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

    pub fn add_to_order_book(&mut self, limit_order: LimitOrder) {
        match limit_order.side {
            Side::Buy => {
                OrderBook::add_to_queue(&mut self.bids, limit_order);
            }
            Side::Sell => {
                OrderBook::add_to_queue(&mut self.asks, limit_order);
            }
        }
    }

    fn add_to_queue(
        order_book_side: &mut BTreeMap<Decimal, VecDeque<LimitOrder>>,
        limit_order: LimitOrder,
    ) {
        let order_price_exists = order_book_side.contains_key(&limit_order.limit_price);

        match order_price_exists {
            false => {
                order_book_side.insert(limit_order.limit_price, VecDeque::from([limit_order]));
            }
            true => {
                let existing_queue = order_book_side.get_mut(&limit_order.limit_price);

                if let Some(queue) = existing_queue {
                    queue.push_back(limit_order);
                } else {
                    eprintln!("Unable to fetch existing queue.");
                }
            }
        }

        println!("Updated Order Book Side: {:#?}\n", order_book_side);
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct LimitOrder {
    pub time_placed: DateTime<Local>,
    pub stock_symbol: String,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
}

#[derive(Debug, Clone)]
pub enum Side {
    Buy,
    Sell,
}
