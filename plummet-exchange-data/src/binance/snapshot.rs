use reqwest::blocking::{Client};

use crate::{Level, OrderBookSnapshot, binance::{BinanceError, L2OrderBook, SymbolStatus}};

pub struct L2OrderBookSnapshot {
    last_update_id: u64,
    bids: Vec<Level>,
    asks: Vec<Level>,
}

impl OrderBookSnapshot for L2OrderBookSnapshot {
    type Output =  L2OrderBook;
    type Error = BinanceError;

    fn get(symbol: &str, depth: u16, status: SymbolStatus) -> Result<Self, Self::Error> {
        let client = Client::new();

        let body = client.get(format!("https://api1.binance.com/api/v3/depth?symbol=BNBBTC&limit={depth}&symbolStatus={status}"))
        .send()?
        .json();

        Ok(Self { last_update_id: (), bids: (), asks: () })
    }
}