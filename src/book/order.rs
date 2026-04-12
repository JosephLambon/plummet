use std::io::Error;

use rust_decimal::{Decimal, dec};

use chrono::prelude::{DateTime, Local};

use crate::engine::InstrumentKey;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct LimitOrder {
    pub instrument: InstrumentKey,
    pub id: u64,
    pub state: OrderState,
    pub placed_at: DateTime<Local>,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
    pub quantity_traded: Decimal,
    pub quantity_remaining: Decimal,
}

impl LimitOrder {
    pub fn is_open(&self) -> bool {
        self.state == OrderState::Open || self.state == OrderState::PartiallyFulfilled
    }

    pub fn adjust_quantities(&mut self, qty: Decimal) -> Result<bool, Error> {
        self.quantity_traded += qty;
        self.quantity_remaining -= qty;

        if self.quantity_remaining < dec!(0) {
            Err(Error::other("Quantity remaining dropped below 0."))
        } else {
            Ok(self.quantity_remaining == dec!(0))
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, Copy)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum OrderState {
    New,                // Instantiated
    Open,               // Resting in order book
    PartiallyFulfilled, // Partially executed
    Fulfilled,          // Wholly executed
    Cancelled,          // Cancelled
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use super::*;

    #[test]
    fn is_open_true() {
        let bid1 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
            state: OrderState::Open,
        };

        assert!(bid1.is_open());

        let bid2 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
            state: OrderState::PartiallyFulfilled,
        };

        assert!(bid2.is_open());
    }

    #[test]
    fn is_open_false() {
        let bid1 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
            state: OrderState::New,
        };

        assert!(!bid1.is_open());

        let bid2 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
            state: OrderState::Fulfilled,
        };

        assert!(!bid2.is_open());

        let bid3 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
            state: OrderState::Cancelled,
        };

        assert!(!bid3.is_open());
    }

    #[test]
    fn adjust_quantities_ok() {
        let mut bid = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(15),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(15),
            state: OrderState::New,
        };
        let mut ask = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 2,
            limit_price: dec!(100),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
            state: OrderState::New,
        };

        let qty = Decimal::min(bid.quantity_remaining, ask.quantity_remaining);

        let bid_result = bid.adjust_quantities(qty);
        let ask_result = ask.adjust_quantities(qty);

        assert!(!bid_result.unwrap());
        assert_eq!(bid.quantity_remaining, dec!(5));
        assert_eq!(bid.quantity_traded, dec!(10));
        assert!(ask_result.unwrap());
        assert_eq!(ask.quantity_remaining, dec!(0));
        assert_eq!(ask.quantity_traded, dec!(10));
    }

    #[test]
    fn adjust_quantities_err() {
        let mut bid = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(100),
            quantity: dec!(15),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(15),
            state: OrderState::New,
        };

        let qty = dec!(100);

        let result = bid.adjust_quantities(qty);

        let err = result.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Other);
        assert_eq!(err.to_string(), "Quantity remaining dropped below 0.");
        assert_eq!(bid.quantity_remaining, dec!(-85));
        assert_eq!(bid.quantity_traded, dec!(100));
    }
}
