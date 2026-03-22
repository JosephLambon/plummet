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
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
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
