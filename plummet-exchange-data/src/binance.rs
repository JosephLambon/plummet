pub mod book;
pub use book::*;

mod snapshot;

#[derive(Debug)]
pub enum SymbolStatus {
    TRADING,
    HALT,
    BREAK
}

pub enum BinanceError {
    GetSnapshotError
}