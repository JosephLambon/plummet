pub mod order;
use chrono::Local;
pub use order::{LimitOrder, Side};

use crate::engine::event::EngineEvent;

use std::{
    collections::{BTreeMap, VecDeque},
    io::{Error, ErrorKind},
};

use tracing::{Level, debug, instrument, trace};

use rust_decimal::{Decimal};

mod trade;
pub use trade::Trade;

#[derive(Debug)]
pub struct OrderBook {
    pub asks: BTreeMap<Decimal, VecDeque<LimitOrder>>,
    pub bids: BTreeMap<Decimal, VecDeque<LimitOrder>>,
    pub orders_placed: u64,
    pub events_processed: u64,
    pub executed_trades: u64,
    pub cancelled_trades: u64,
}

#[derive(Debug, PartialEq)]
pub struct MatchResult {
    pub bid_id: u64,
    pub ask_id: u64,
    pub ask_price: Decimal,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
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
                let bid_queue = self.bids.get_mut(&matched.ask_price).ok_or(Error::new(
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
                
                let bid_id = bid.id;
                let ask_id = ask.id;
                let execution_price = ask.limit_price;
                
                // adjust quantitites
                let quantity = Decimal::min(bid.quantity_remaining, ask.quantity_remaining);
                let bid_fulfilled = bid.adjust_quantities(quantity);
                let ask_fulfilled = ask.adjust_quantities(quantity);
                
                // Remove if needed
                if bid_fulfilled {
                    bid.state = order::OrderState::Fulfilled;
                    bid_queue.pop_front();
                } else {
                    bid.state = order::OrderState::PartiallyFulfilled;
                }
                
                if ask_fulfilled {
                    ask.state = order::OrderState::Fulfilled;
                    ask_queue.pop_front();
                } else {
                    ask.state = order::OrderState::PartiallyFulfilled;
                }
                
                self.events_processed += 1;
                self.executed_trades += 1;
                Ok(EngineEvent::TradeExecuted(Trade {
                    trade_id: self.executed_trades,
                    bid_order_id: bid_id,
                    ask_order_id: ask_id,
                    executed_at: Local::now(),
                    execution_price: execution_price,
                    executed_quantity: quantity,
                }))
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
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use chrono::prelude::Local;
    use rust_decimal::dec;

    use crate::{book::order::OrderState, engine::event::OrdersMatchedEvent};

    use super::*;

    struct TestOrders {
        left: LimitOrder,
        right: LimitOrder,
    }

    fn create_orders(
        ids: (u64, u64),
        prices: (Decimal, Decimal),
        sides: (Side, Side),
        quantities: (Decimal, Decimal),
    ) -> (LimitOrder, LimitOrder) {
        (
            LimitOrder {
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
        let order_book = OrderBook::new();

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

        let mut order_book = OrderBook::new();

        order_book.insert(bid1);
        order_book.insert(bid2);

        assert!(order_book.asks.is_empty());
        assert_eq!(order_book.bids.len(), 2);
        assert!(order_book.bids.contains_key(&dec!(1200.2134)));
        assert!(order_book.bids.contains_key(&dec!(1200.2136)));
    }

    #[test]
    fn insert_existing_bid_level() {
        let (bid1, bid2) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Buy),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new();

        order_book.insert(bid1.clone());
        order_book.insert(bid2.clone());

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

        let mut order_book = OrderBook::new();

        order_book.insert(ask1);
        order_book.insert(ask2);

        assert!(order_book.bids.is_empty());
        assert_eq!(order_book.asks.len(), 2);
        assert!(order_book.asks.contains_key(&dec!(1200.2134)));
        assert!(order_book.asks.contains_key(&dec!(1200.2136)));
    }

    #[test]
    fn insert_existing_ask_level() {
        let (ask1, ask2) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Sell, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new();

        order_book.insert(ask1.clone());
        order_book.insert(ask2.clone());

        let expected = VecDeque::from([ask1, ask2]);

        assert_eq!(order_book.asks.len(), 1);
        assert!(order_book.asks.contains_key(&dec!(1200.2134)));
        assert_eq!(order_book.asks.get(&dec!(1200.2134)).unwrap(), &expected);
    }

    #[test]
    fn insert_routes_to_correct_side() {
        let (bid, ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2136)),
            (Side::Buy, Side::Sell),
            (dec!(10), dec!(10)),
        );

        let mut order_book = OrderBook::new();

        order_book.insert(bid.clone());
        order_book.insert(ask.clone());

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

        let mut order_book = OrderBook::new();

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

        let mut order_book = OrderBook::new();

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
            id: 3,
            limit_price: dec!(1200.2134),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

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

        let mut order_book = OrderBook::new();

        let expected = MatchResult {
            ask_id: ask.id,
            bid_id: bid.id,
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

        let mut order_book = OrderBook::new();

        let expected = MatchResult {
            ask_id: ask.id,
            bid_id: bid.id,
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

        let mut order_book = OrderBook::new();

        order_book.insert(bid);
        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_no_bids() {
        let ask = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Sell,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

        order_book.insert(ask);

        assert_eq!(order_book.match_sides(), None);
    }

    #[test]
    fn match_sides_no_asks() {
        let bid = LimitOrder {
            id: 3,
            limit_price: dec!(1200.2133),
            quantity: dec!(10),
            placed_at: Local::now(),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(0),
            state: order::OrderState::Open,
        };

        let mut order_book = OrderBook::new();

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

        let mut order_book = OrderBook::new();

        let expected = MatchResult {
            ask_id: ask1.id,
            bid_id: bid1.id,
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
        let order_book = OrderBook::new();

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

        let mut order_book = OrderBook::new();

        let bid_id = bid.id;
        let ask_id = ask.id;
        let ask_price = ask.limit_price;

        order_book.insert(bid);
        order_book.insert(ask);

        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            id: 1,
            bid_id: bid_id,
            ask_id: ask_id,
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

        assert_eq!(
            order_book.asks.get_key_value(&ask_price).unwrap().1.len(),
            0
        );
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


        let mut order_book = OrderBook::new();

        let bid_id = bid.id;
        let ask_id = ask.id;
        let ask_price = ask.limit_price;

        order_book.insert(bid);
        order_book.insert(ask);

        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            id: 1,
            bid_id: bid_id,
            ask_id: ask_id,
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

        assert_eq!(
            order_book.asks.get_key_value(&ask_price).unwrap().1.len(),
            0
        );
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
        let (mut bid, mut ask) = create_orders(
            (1, 2),
            (dec!(1200.2134), dec!(1200.2134)),
            (Side::Buy, Side::Sell),
            (dec!(15), dec!(10)),
        );
        
        let mut order_book = OrderBook::new();
        
        let bid_id = bid.id;
        let ask_id = ask.id;
        let ask_price = ask.limit_price;
        
        order_book.insert(bid);
        order_book.insert(ask);
        
        let result = order_book.process(&EngineEvent::OrdersMatched(OrdersMatchedEvent {
            id: 1,
            bid_id: bid_id,
            ask_id: ask_id,
            ask_price: dec!(1500),
            matched_at: Local::now(),
        }));
        
        let err = result.unwrap_err();
        
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "Unable to find price level on bid side");
    }
    
    #[test]
    fn process_match_invalid_order_state() {
        // MUST ADD Guard code for state
        panic!();
    }
    #[test]
    fn process_cancellation() {
        panic!();
    }
}
