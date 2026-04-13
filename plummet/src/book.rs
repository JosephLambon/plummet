use chrono::Local;

use std::{
    collections::{BTreeMap, VecDeque},
    io::{Error, ErrorKind},
};
use tracing::{Level, debug, instrument, trace};

use rust_decimal::{Decimal, dec};

use crate::engine::{InstrumentKey, event::EngineEvent};

pub mod order;
pub use order::{LimitOrder, OrderState, Side};

mod trade;
pub use trade::Trade;

#[derive(Debug)]
pub struct OrderBook {
    pub instrument: InstrumentKey,
    pub asks: BTreeMap<Decimal, VecDeque<LimitOrder>>,
    pub bids: BTreeMap<Decimal, VecDeque<LimitOrder>>,
    orders_placed: u64,
    pub events_processed: u64,
    pub executed_trades: u64,
    pub cancelled_trades: u64,
}

struct ExecutedTradeResult {
    trade: Trade,
    bid_price: Decimal,
    bid_fulfilled: bool,
    ask_fulfilled: bool,
}

#[derive(Debug, PartialEq)]
pub struct MatchResult {
    pub bid_id: u64,
    pub ask_id: u64,
    pub ask_price: Decimal,
    pub bid_price: Decimal,
}

impl OrderBook {
    pub fn new(instrument: InstrumentKey) -> Self {
        Self {
            instrument,
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            orders_placed: 0,
            events_processed: 0,
            executed_trades: 0,
            cancelled_trades: 0,
        }
    }

    #[instrument(level = Level::DEBUG, skip(self))]
    pub fn process(&mut self, event: &EngineEvent) -> Result<EngineEvent, Error> {
        match event {
            EngineEvent::OrdersMatched(matched) => {
                let executed: ExecutedTradeResult = {
                    let bid_queue = self.bids.get_mut(&matched.bid_price).ok_or(Error::new(
                        ErrorKind::NotFound,
                        "Unable to find price level on bid side",
                    ))?;
                    let bid = bid_queue.front_mut().ok_or(Error::new(
                        ErrorKind::NotFound,
                        "Unable to find price level on bid side",
                    ))?;

                    let ask_queue = self.asks.get_mut(&matched.ask_price).ok_or(Error::new(
                        ErrorKind::NotFound,
                        "Unable to find price level on ask side",
                    ))?;
                    let ask = ask_queue.front_mut().ok_or(Error::new(
                        ErrorKind::NotFound,
                        "Unable to find price level on bid side",
                    ))?;

                    if !bid.is_open() || !ask.is_open() {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            // Use ':?' to use the Debug formatter
                            // Implement Display on OrderState enum to remove ':?' requirement
                            format!(
                                "Invalid order state. Bid: {:?}; Ask: {:?}",
                                bid.state, ask.state
                            ),
                        ));
                    } else {
                        let quantity = Decimal::min(bid.quantity_remaining, ask.quantity_remaining);

                        let bid_fulfilled = bid.adjust_quantities(quantity)?;
                        let ask_fulfilled = ask.adjust_quantities(quantity)?;

                        if bid_fulfilled {
                            bid.state = OrderState::Fulfilled
                        } else if quantity > dec!(0) {
                            bid.state = OrderState::PartiallyFulfilled
                        }
                        if ask_fulfilled {
                            ask.state = OrderState::Fulfilled
                        } else if quantity > dec!(0) {
                            ask.state = OrderState::PartiallyFulfilled
                        }

                        self.executed_trades += 1;

                        ExecutedTradeResult {
                            trade: Trade {
                                instrument: self.instrument,
                                trade_id: self.executed_trades,
                                bid_order_id: bid.id,
                                ask_order_id: ask.id,
                                executed_at: Local::now(),
                                execution_price: ask.limit_price,
                                executed_quantity: quantity,
                            },
                            bid_price: bid.limit_price,
                            ask_fulfilled,
                            bid_fulfilled,
                        }
                    }
                };

                if executed.bid_fulfilled {
                    self.remove_front_order(&executed.bid_price, Side::Buy)?;
                }
                if executed.ask_fulfilled {
                    self.remove_front_order(&executed.trade.execution_price, Side::Sell)?;
                }

                self.events_processed += 1;
                Ok(EngineEvent::TradeExecuted(executed.trade))
            }
            EngineEvent::OrderCancelled(cancelled) => {
                self.events_processed += 1;
                self.cancelled_trades += 1;

                Err(Error::new(ErrorKind::Unsupported, "Not implemented"))
            }
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                "Executor does not support that EngineEvent",
            )),
        }
    }

    #[instrument(level = Level::DEBUG, skip_all)]
    pub fn match_sides(&self) -> Option<MatchResult> {
        debug!("Checking for match");
        let lowest_ask = self.asks.first_key_value()?;
        let highest_bid = self.bids.last_key_value()?;

        debug!(
            bid = %highest_bid.0,
            ask = %lowest_ask.0,
        );

        // Get orders from front of price level's queues
        let bid = highest_bid.1.front()?;
        let ask = lowest_ask.1.front()?;

        if bid.limit_price >= ask.limit_price && bid.is_open() && ask.is_open() {
            Some(MatchResult {
                bid_id: bid.id,
                ask_id: ask.id,
                bid_price: bid.limit_price,
                ask_price: ask.limit_price,
            })
        } else {
            None
        }
    }

    #[instrument(level = Level::DEBUG, skip_all)]
    pub fn insert(&mut self, mut limit_order: LimitOrder) {
        debug!(
            price = %limit_order.limit_price,
            quantity = %limit_order.quantity,
            side = ?limit_order.side,
            "Inserting order ID to order book"
        );

        self.orders_placed += 1;
        limit_order.state = order::OrderState::Open;

        match limit_order.side {
            Side::Buy => {
                OrderBook::push_back_or_create_price_level(&mut self.bids, limit_order);
            }
            Side::Sell => {
                OrderBook::push_back_or_create_price_level(&mut self.asks, limit_order);
            }
        }
    }

    #[instrument(level = Level::TRACE, skip_all)]
    fn push_back_or_create_price_level(
        order_book_side: &mut BTreeMap<Decimal, VecDeque<LimitOrder>>,
        limit_order: LimitOrder,
    ) {
        trace!(
            price_level = %limit_order.limit_price,
            order_id = %limit_order.id,
            "Pushing order to price level"
        );
        order_book_side
            .entry(limit_order.limit_price)
            .or_default()
            .push_back(limit_order);
    }

    #[instrument(level = Level::TRACE, skip_all)]
    fn remove_front_order(&mut self, price_level: &Decimal, side: Side) -> Result<(), Error> {
        let side = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        let queue = side.get_mut(price_level).ok_or(Error::new(
            ErrorKind::InvalidInput,
            "Price level does not exist.",
        ))?;

        let order = queue
            .pop_front()
            .ok_or(Error::new(ErrorKind::InvalidData, "Queue was empty."))?;

        trace!(
            price_level = %order.limit_price,
            order_id = %order.id,
            "Removing order from price level's queue"
        );

        if queue.is_empty() {
            OrderBook::remove_price_level(side, &order.limit_price)?
        }

        Ok(())
    }

    #[instrument(level = Level::TRACE, skip_all)]
    fn remove_price_level(
        side: &mut BTreeMap<Decimal, VecDeque<LimitOrder>>,
        price_level: &Decimal,
    ) -> Result<(), Error> {
        let _ = side.remove(price_level).ok_or(Error::new(
            ErrorKind::InvalidInput,
            "Price level does not exist.",
        ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::prelude::Local;
    use rust_decimal::dec;

    use crate::{
        book::order::OrderState,
        engine::event::{CancellationEvent, OrdersMatchedEvent},
    };

    use super::*;

    fn create_orders(
        ids: (u64, u64),
        prices: (Decimal, Decimal),
        sides: (Side, Side),
        quantities: (Decimal, Decimal),
    ) -> (LimitOrder, LimitOrder) {
        (
            LimitOrder {
                instrument: InstrumentKey::Btc,
                id: ids.0,
                limit_price: prices.0,
                quantity: quantities.0,
                placed_at: Local::now(),
                side: sides.0,
                quantity_traded: dec!(0),
                quantity_remaining: quantities.0,
                state: order::OrderState::New,
            },
            LimitOrder {
                instrument: InstrumentKey::Btc,
                id: ids.1,
                limit_price: prices.1,
                quantity: quantities.1,
                placed_at: Local::now(),
                side: sides.1,
                quantity_traded: dec!(0),
                quantity_remaining: quantities.1,
                state: order::OrderState::New,
            },
        )
    }

    #[test]
    fn new_initialises() {
        let order_book = OrderBook::new(InstrumentKey::Btc);

        assert_eq!(BTreeMap::new(), order_book.asks);
        assert_eq!(BTreeMap::new(), order_book.bids);
    }

    #[test]
    fn insert_new_bids() {
        let (bid1, bid2) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2136)),
            (Side::Buy, Side::Buy),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid1);
        order_book.insert(bid2);

        assert!(order_book.asks.is_empty());
        assert_eq!(order_book.bids.len(), 2);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert!(order_book.bids.contains_key(&dec!(1200.2136)));
    }

    #[test]
    fn insert_existing_bid_level() {
        let (mut bid1, mut bid2) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Buy),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid1.clone());
        order_book.insert(bid2.clone());

        bid1.state = OrderState::Open;
        bid2.state = OrderState::Open;
        let expected = VecDeque::from([bid1, bid2]);

        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert_eq!(order_book.bids.get(&dec!(1200.2134)).unwrap(), &expected);
    }
    #[test]
    fn insert_new_asks() {
        let (ask1, ask2) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2136)),
            (Side::Sell, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(ask1);
        order_book.insert(ask2);

        assert!(order_book.bids.is_empty());
        assert_eq!(order_book.asks.len(), 2);
        assert!(order_book.asks.contains_key(&dec!(1200.2134)));
        assert!(order_book.asks.contains_key(&dec!(1200.2136)));
    }

    #[test]
    fn insert_existing_ask_level() {
        let (mut ask1, mut ask2) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Sell, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(ask1.clone());
        order_book.insert(ask2.clone());

        ask1.state = OrderState::Open;
        ask2.state = OrderState::Open;
        let expected = VecDeque::from([ask1, ask2]);

        assert_eq!(order_book.asks.len(), 1);
        assert!(order_book.asks.contains_key(&dec!(1200.2134)));
        assert_eq!(order_book.asks.get(&dec!(1200.2134)).unwrap(), &expected);
    }

    #[test]
    fn insert_routes_to_correct_side() {
        let (mut bid, mut ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2136)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid.clone());
        order_book.insert(ask.clone());

        bid.state = OrderState::Open;
        ask.state = OrderState::Open;
        let expected_bids = VecDeque::from([bid]);
        let expected_asks = VecDeque::from([ask]);

        assert_eq!(order_book.asks.len(), 1);
        assert!(order_book.asks.contains_key(&dec!(1200.2136)));
        assert_eq!(
            order_book.asks.get(&dec!(1200.2136)).unwrap(),
            &expected_asks
        );

        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert_eq!(
            order_book.bids.get(&dec!(1200.2134)).unwrap(),
            &expected_bids
        );
    }

    #[test]
    fn insert_orders_placed_count() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2136)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        assert_eq!(order_book.orders_placed, 0);

        order_book.insert(bid.clone());

        assert_eq!(order_book.orders_placed, 1);

        order_book.insert(ask.clone());

        assert_eq!(order_book.orders_placed, 2);
    }

    #[test]
    fn insert_order_states() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2136)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        assert_eq!(bid.state, OrderState::New);

        order_book.insert(bid);

        assert_eq!(
            order_book
                .bids
                .get(&dec!(1200.2134))
                .unwrap()
                .front()
                .unwrap()
                .state,
            OrderState::Open
        );
        assert_eq!(ask.state, OrderState::New);

        order_book.insert(ask);

        assert_eq!(
            order_book
                .asks
                .get(&dec!(1200.2136))
                .unwrap()
                .front()
                .unwrap()
                .state,
            OrderState::Open
        );
    }

    #[test]
    fn insert_fifo_order_within_price_level() {
        let bid1: LimitOrder = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };
        let bid2 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 2,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };
        let bid3 = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 3,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid1.clone());
        order_book.insert(bid2.clone());
        order_book.insert(bid3.clone());

        let expected = VecDeque::from([bid1.clone(), bid2.clone(), bid3.clone()]);

        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert_eq!(
            order_book
                .bids
                .get(&dec!(1200.2134))
                .unwrap()
                .front()
                .unwrap(),
            &bid1
        );
        assert_eq!(
            order_book
                .bids
                .get(&dec!(1200.2134))
                .unwrap()
                .back()
                .unwrap(),
            &bid3
        );
        assert_eq!(order_book.bids.get(&dec!(1200.2134)).unwrap(), &expected);
    }

    #[test]
    fn match_sides_bid_above_ask() {
        let (bid, ask) = create_orders(
            (1, 3),
            (dec!(1200.2134), dec!(1200.2133)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let expected = MatchResult {
            ask_id: ask.id,
            bid_id: bid.id,
            bid_price: bid.limit_price,
            ask_price: ask.limit_price,
        };

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides().unwrap(), expected);
    }

    #[test]
    fn match_sides_bid_matching_ask() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let expected = MatchResult {
            ask_id: ask.id,
            bid_id: bid.id,
            bid_price: bid.limit_price,
            ask_price: ask.limit_price,
        };

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides().unwrap(), expected);
    }

    #[test]
    fn match_sides_bid_below_ask() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2132), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_no_bids() {
        let ask = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_no_asks() {
        let bid = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_fifo_order() {
        let (bid1, ask1) = create_orders(
            (1, 3),
            (dec!(1200.2134), dec!(1200.2133)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );
        let (bid2, ask2) = create_orders(
            (2, 4),
            (dec!(1200.2134), dec!(1200.2133)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let expected = MatchResult {
            ask_id: ask1.id,
            bid_id: bid1.id,
            bid_price: bid1.limit_price,
            ask_price: ask1.limit_price,
        };

        order_book.insert(bid1);
        order_book.insert(bid2);
        order_book.insert(ask1);
        order_book.insert(ask2);

        assert_eq!(order_book.match_sides().unwrap(), expected);
    }

    #[test]
    fn match_sides_empty_order_book() {
        let order_book = OrderBook::new(InstrumentKey::Btc);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn process_match_open_orders() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(15), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let bid_id = bid.id;
        let ask_id = ask.id;
        let ask_price = ask.limit_price;
        let bid_price = bid.limit_price;

        order_book.insert(bid);
        order_book.insert(ask);

        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            instrument: InstrumentKey::Btc,
            bid_order_id: bid_id,
            ask_order_id: ask_id,
            bid_price: bid_price,
            ask_price: ask_price,
            matched_at: Local::now(),
        }));

        let EngineEvent::TradeExecuted(trade) = result.unwrap() else {
            panic!("Expected TradeExecuted event.");
        };

        assert_eq!(trade.executed_quantity, dec!(10));
        assert_eq!(trade.ask_order_id, 2);
        assert_eq!(trade.bid_order_id, 1);
        assert_eq!(trade.execution_price, ask_price);

        assert_eq!(order_book.asks.len(), 0);
        assert_eq!(
            order_book
                .bids
                .get_key_value(&dec!(1200.2134))
                .unwrap()
                .1
                .len(),
            1
        );
    }

    #[test]
    fn process_match_partially_filled_orders() {
        let (mut bid, mut ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(15), dec!(10)),
        );
        bid.state = OrderState::PartiallyFulfilled;
        bid.quantity_remaining = bid.quantity_remaining - dec!(5);
        bid.quantity_traded = bid.quantity_traded + dec!(5);

        ask.state = OrderState::PartiallyFulfilled;
        ask.quantity_remaining = ask.quantity_remaining - dec!(5);
        ask.quantity_traded = ask.quantity_traded + dec!(5);

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let bid_id = bid.id;
        let ask_id = ask.id;
        let ask_price = ask.limit_price;
        let bid_price = bid.limit_price;

        order_book.insert(bid);
        order_book.insert(ask);

        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            instrument: InstrumentKey::Btc,
            bid_order_id: bid_id,
            ask_order_id: ask_id,
            bid_price: bid_price,
            ask_price: ask_price,
            matched_at: Local::now(),
        }));

        let EngineEvent::TradeExecuted(trade) = result.unwrap() else {
            panic!("Expected TradeExecuted event.");
        };

        assert_eq!(trade.executed_quantity, dec!(5));
        assert_eq!(trade.ask_order_id, 2);
        assert_eq!(trade.bid_order_id, 1);
        assert_eq!(trade.execution_price, ask_price);

        assert_eq!(order_book.asks.len(), 0);
        assert_eq!(
            order_book
                .bids
                .get_key_value(&dec!(1200.2134))
                .unwrap()
                .1
                .len(),
            1
        );
    }

    #[test]
    fn process_match_price_level_does_not_exist() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(15), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let bid_id = bid.id;
        let ask_id = ask.id;

        order_book.insert(bid);
        order_book.insert(ask);

        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            instrument: InstrumentKey::Btc,
            bid_order_id: bid_id,
            ask_order_id: ask_id,
            bid_price: dec!(1200.2134),
            ask_price: dec!(1500),
            matched_at: Local::now(),
        }));

        let err = result.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "Unable to find price level on ask side");
    }

    #[test]
    fn process_match_invalid_order_state() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(15), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        let bid_id = bid.id;
        let ask_id = ask.id;
        let bid_price = bid.limit_price;

        order_book.insert(bid);
        order_book.insert(ask);

        let bid = order_book
            .bids
            .get_mut(&bid_price)
            .unwrap()
            .front_mut()
            .unwrap();
        bid.state = OrderState::Fulfilled;

        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            instrument: InstrumentKey::Btc,
            bid_order_id: bid_id,
            ask_order_id: ask_id,
            bid_price: dec!(1200.2134),
            ask_price: dec!(1200.2134),
            matched_at: Local::now(),
        }));

        let err = result.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(
            err.to_string(),
            "Invalid order state. Bid: Fulfilled; Ask: Open"
        );
    }

    #[test]
    fn process_cancellation() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(15), dec!(10)),
        );

        let mut order_book = OrderBook::new(InstrumentKey::Btc);

        order_book.insert(bid);
        order_book.insert(ask);

        let result = order_book.process(&EngineEvent::OrderCancelled(CancellationEvent {
            instrument: InstrumentKey::Btc,
            order_id: 1,
            cancelled_at: Local::now(),
            limit_price: dec!(1200.2134),
            quantity: dec!(15),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(15),
        }));

        let err = result.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Unsupported);
        assert_eq!(err.to_string(), "Not implemented");
    }
}
