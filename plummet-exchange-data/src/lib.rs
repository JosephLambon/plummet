pub mod binance;
use rust_decimal::Decimal;

pub trait OrderBookUpsert {
    fn upsert(&mut self, level: Level, side: Side);
}
pub trait SymbolStatus {}

pub trait OrderBookSnapshot {
    type Output;
    type Error;

    fn get<S: SymbolStatus>(symbol: &str, depth: u16, status: S) -> Result<Self, Self::Error> where Self: Sized;
}


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
