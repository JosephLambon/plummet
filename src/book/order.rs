use rust_decimal::Decimal;

use chrono::prelude::{DateTime, Local};

#[derive(Debug, PartialEq, Clone)]
pub struct LimitOrder {
    pub id: u64,
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
