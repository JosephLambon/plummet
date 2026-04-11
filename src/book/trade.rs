use chrono::prelude::{DateTime, Local};
use rust_decimal::Decimal;

use crate::engine::InstrumentKey;

#[derive(Debug, Clone)]
pub struct Trade {
    pub instrument: InstrumentKey,
    pub trade_id: u64,
    pub bid_order_id: u64,
    pub ask_order_id: u64,
    pub executed_at: DateTime<Local>,
    pub execution_price: Decimal,
    pub executed_quantity: Decimal,
}
