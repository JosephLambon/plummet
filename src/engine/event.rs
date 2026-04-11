use chrono::{DateTime, Local};
use rust_decimal::Decimal;

use crate::{
    book::{LimitOrder, Side, Trade, order::OrderState},
    engine::InstrumentKey,
};

#[derive(Debug, Clone)]
pub enum EngineCommand {
    PlaceOrder(LimitOrder),
    CancelOrder(LimitOrder),
    Shutdown,
}

pub enum CommandOutcome {
    Continue,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    OrderPlaced(OrderPlacedEvent),
    OrderCancelled(CancellationEvent),
    OrdersMatched(OrdersMatchedEvent),
    TradeExecuted(Trade),
    Shutdown,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct OrderPlacedEvent {
    pub instrument: InstrumentKey,
    pub id: u64,
    pub state: OrderState,
    pub placed_at: DateTime<Local>,
    pub accepted_at: DateTime<Local>,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
    pub quantity_traded: Decimal,
    pub quantity_remaining: Decimal,
}

#[derive(Debug, Clone)]
pub struct OrdersMatchedEvent {
    pub instrument: InstrumentKey,
    pub id: u64,
    pub bid_id: u64,
    pub ask_id: u64,
    pub ask_price: Decimal,
    pub bid_price: Decimal,
    pub matched_at: DateTime<Local>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct CancellationEvent {
    pub instrument: InstrumentKey,
    pub id: u64,
    pub cancelled_at: DateTime<Local>,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
    pub quantity_traded: Decimal,
    pub quantity_remaining: Decimal,
}
