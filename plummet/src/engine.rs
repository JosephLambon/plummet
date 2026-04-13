use std::{
    collections::{HashMap, hash_map::Entry},
    sync::mpsc::{self, SendError},
    thread,
};

use chrono::Local;
use rust_decimal::dec;
use tracing::{debug, error};

use crate::book::OrderBook;

pub mod event;
use event::*;

// Re-export
pub use event::{EngineCommand, EngineEvent};

#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub enum InstrumentKey {
    Btc,
    Eth,
}

pub struct Engine {
    pub senders: HashMap<InstrumentKey, mpsc::Sender<EngineCommand>>,
    pub event_tx: mpsc::Sender<EngineEvent>,
    pub events: mpsc::Receiver<EngineEvent>,
}
impl Engine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<EngineEvent>();

        Engine {
            senders: HashMap::new(),
            event_tx: tx,
            events: rx,
        }
    }

    pub fn add_instrument(&mut self, ticker_symbol: InstrumentKey) {
        if let Entry::Vacant(entry) = self.senders.entry(ticker_symbol) {
            let (tx, rx) = mpsc::channel::<EngineCommand>();
            let event_tx = self.event_tx.clone();

            entry.insert(tx);

            thread::spawn(move || {
                let mut order_book = OrderBook::new(ticker_symbol);
                // Listen for commands until shutdown
                while let Ok(command) = rx.recv() {
                    match Self::handle_command(&mut order_book, command, &event_tx) {
                        Ok(CommandOutcome::Shutdown) | Err(_) => break,
                        Ok(CommandOutcome::Continue) => {}
                    }
                }
            });
        }
    }

    fn handle_command(
        order_book: &mut OrderBook,
        command: EngineCommand,
        event_tx: &mpsc::Sender<EngineEvent>,
    ) -> Result<CommandOutcome, SendError<EngineEvent>> {
        match command {
            EngineCommand::PlaceOrder(order) => {
                // Audit log event
                event_tx.send(EngineEvent::OrderPlaced(OrderPlacedEvent {
                    instrument: order.instrument,
                    order_id: order.id,
                    state: order.state,
                    placed_at: order.placed_at,
                    accepted_at: Local::now(),
                    limit_price: order.limit_price,
                    quantity: order.quantity,
                    side: order.side,
                    quantity_traded: dec!(0),
                    quantity_remaining: order.quantity_remaining,
                }))?;

                order_book.insert(order);

                while let Some(result) = order_book.match_sides() {
                    debug!("Match found.");

                    let match_event = EngineEvent::OrdersMatched(OrdersMatchedEvent {
                        instrument: order_book.instrument,
                        matched_at: Local::now(),
                        ask_order_id: result.ask_id,
                        bid_order_id: result.bid_id,
                        bid_price: result.bid_price,
                        ask_price: result.ask_price,
                    });

                    let result = order_book.process(&match_event);

                    // OrdersMatched event must reach
                    // events stream before result handled
                    event_tx.send(match_event)?;

                    if let Ok(event) = result {
                        debug!("Trade successfully executed.");
                        event_tx.send(event)?;
                    } else {
                        // Enhance in future so recoverable errors don't force shutdown
                        error!("Unable to execute trade.");
                        return Ok(CommandOutcome::Shutdown);
                    }
                }
                Ok(CommandOutcome::Continue)
            }
            EngineCommand::CancelOrder(order) => {
                event_tx.send(EngineEvent::OrderCancelled(CancellationEvent {
                    instrument: order.instrument,
                    order_id: order.id,
                    cancelled_at: Local::now(),
                    limit_price: order.limit_price,
                    quantity: order.quantity,
                    side: order.side,
                    quantity_traded: dec!(0),
                    quantity_remaining: order.quantity_remaining,
                }))?;
                debug!("CANCELLATION PLACEHOLDER");
                Ok(CommandOutcome::Continue)
            }
            EngineCommand::Shutdown => {
                // Audit log shutdown event
                event_tx.send(EngineEvent::Shutdown)?;
                // Execute trade
                println!("\n\n THREAD CLOSING... \n\n");
                Ok(CommandOutcome::Shutdown)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc::TryRecvError, time::Duration};

    use crate::book::{LimitOrder, Side, order::OrderState};

    use super::*;

    #[test]
    fn add_instrument_new_instrument() {
        let mut engine = Engine::new();

        engine.add_instrument(InstrumentKey::Btc);

        assert_eq!(engine.senders.len(), 1);
    }

    #[test]
    fn add_instrument_duplicate_instrument() {
        let mut engine = Engine::new();

        let (tx, rx) = mpsc::channel();
        // Manually insert a BTC sender
        engine.senders.insert(InstrumentKey::Btc, tx);
        // Should NOT overwrite existing sender
        engine.add_instrument(InstrumentKey::Btc);
        // Manually inserted channel not disconnected
        assert_ne!(rx.try_recv().err().unwrap(), TryRecvError::Disconnected);
    }

    #[test]
    fn add_instrument_multiple_instruments() {
        let mut engine = Engine::new();

        engine.add_instrument(InstrumentKey::Btc);
        engine.add_instrument(InstrumentKey::Eth);

        assert_eq!(engine.senders.len(), 2);
    }

    #[test]
    fn add_instrument_event_channel_disconnects() {
        let mut engine = Engine::new();
        engine.add_instrument(InstrumentKey::Btc);

        // Disconnect channel by dropping Receiver
        drop(engine.events);

        let tx = engine.senders.get(&InstrumentKey::Btc).unwrap();

        let result = tx.send(EngineCommand::PlaceOrder(LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            state: OrderState::New,
            placed_at: Local::now(),
            limit_price: dec!(1500),
            quantity: dec!(10),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
        }));

        let deadline = Local::now() + Duration::from_secs(1);
        loop {
            // Implicit assertion
            // After first event send is attempted, handle_command Err bubbles up
            // This then causes thread to be exited, so subsequent EngineCommand sends Err
            if tx.send(EngineCommand::Shutdown).is_err() {
                break;
            }
            assert!(
                Local::now() < deadline,
                "worker thread did not exit within 1s after event channel disconnected"
            );
            // Yield current thread to allow other to execute if blocked
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn add_instrument_valid_order_placed_succeeds() {
        let mut engine = Engine::new();
        engine.add_instrument(InstrumentKey::Btc);

        let tx = engine.senders.get(&InstrumentKey::Btc).unwrap();

        tx.send(EngineCommand::PlaceOrder(LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            state: OrderState::New,
            placed_at: Local::now(),
            limit_price: dec!(1500),
            quantity: dec!(10),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
        }))
        .unwrap();
        tx.send(EngineCommand::PlaceOrder(LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 2,
            state: OrderState::New,
            placed_at: Local::now(),
            limit_price: dec!(1500),
            quantity: dec!(10),
            side: Side::Sell,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
        }))
        .unwrap();
        tx.send(EngineCommand::Shutdown).unwrap();

        thread::sleep(Duration::from_millis(250));

        let mut events = engine.events.iter();

        let mut event = events.next().unwrap();
        assert!(matches!(
            event,
            EngineEvent::OrderPlaced(ref e) if e.order_id == 1
            && e.instrument == InstrumentKey::Btc
            && e.state == OrderState::New
            && e.limit_price == dec!(1500)
            && e.side == Side::Buy
            && e.quantity == dec!(10)
        ));
        event = events.next().unwrap();
        assert!(matches!(
            event,
            EngineEvent::OrderPlaced(ref e) if e.order_id == 2
            && e.instrument == InstrumentKey::Btc
            && e.state == OrderState::New
            && e.limit_price == dec!(1500)
            && e.side == Side::Sell
            && e.quantity == dec!(10)
        ));
        event = events.next().unwrap();
        assert!(matches!(
            event,
            EngineEvent::OrdersMatched(ref e) if e.instrument == InstrumentKey::Btc
            && e.bid_order_id == 1
            && e.ask_order_id == 2
            && e.ask_price == dec!(1500)
            && e.bid_price == dec!(1500)
        ));
        event = events.next().unwrap();
        assert!(matches!(
            event,
            EngineEvent::TradeExecuted(ref t) if t.trade_id == 1
            && t.instrument == InstrumentKey::Btc
            && t.bid_order_id == 1
            && t.ask_order_id == 2
            && t.executed_quantity == dec!(10)
            && t.execution_price == dec!(1500)
        ));
        event = events.next().unwrap();
        assert_eq!(event, EngineEvent::Shutdown);
    }

    #[test]
    fn add_instrument_invalid_order_placed() {
        // MUST add guard code & Result output to OrderBook::insert in a future story
        panic!()
    }

    #[test]
    fn add_instrument_order_cancelled() {
        let mut engine = Engine::new();
        engine.add_instrument(InstrumentKey::Btc);

        let tx = engine.senders.get(&InstrumentKey::Btc).unwrap();

        let order = LimitOrder {
            instrument: InstrumentKey::Btc,
            id: 1,
            state: OrderState::New,
            placed_at: Local::now(),
            limit_price: dec!(1500),
            quantity: dec!(10),
            side: Side::Buy,
            quantity_traded: dec!(0),
            quantity_remaining: dec!(10),
        };

        tx.send(EngineCommand::PlaceOrder(order.clone())).unwrap();
        tx.send(EngineCommand::CancelOrder(order)).unwrap();
        tx.send(EngineCommand::Shutdown).unwrap();

        thread::sleep(Duration::from_millis(250));

        let mut events = engine.events.iter();

        let mut event = events.next().unwrap();
        assert!(matches!(
            event,
            EngineEvent::OrderPlaced(ref e) if e.order_id == 1
            && e.instrument == InstrumentKey::Btc
            && e.state == OrderState::New
            && e.limit_price == dec!(1500)
            && e.side == Side::Buy
            && e.quantity == dec!(10)
        ));
        event = events.next().unwrap();
        assert!(matches!(
            event,
            EngineEvent::OrderCancelled(ref e) if e.order_id == 1
            && e.instrument == InstrumentKey::Btc
            && e.quantity_traded == dec!(0)
            && e.quantity_remaining == dec!(10)
            && e.limit_price == dec!(1500)
            && e.side == Side::Buy
            && e.quantity == dec!(10)
        ));
        event = events.next().unwrap();
        assert_eq!(event, EngineEvent::Shutdown);
    }

    #[test]
    fn add_instrument_shutdown() {
        let mut engine = Engine::new();
        engine.add_instrument(InstrumentKey::Btc);

        let tx = engine.senders.get(&InstrumentKey::Btc).unwrap();

        tx.send(EngineCommand::Shutdown).unwrap();

        thread::sleep(Duration::from_millis(250));

        let mut events = engine.events.iter();

        assert_eq!(events.next().unwrap(), EngineEvent::Shutdown);

        let deadline = Local::now() + Duration::from_secs(1);
        loop {
            // Implicit assertion
            // After first Shutdown send, thread should have exited
            // Subsequent EngineCommand sends should Err
            if tx.send(EngineCommand::Shutdown).is_err() {
                break;
            }

            assert!(
                Local::now() < deadline,
                "worker thread did not exit within 1s of first Shutdown command"
            );
            // Yield current thread to allow other to execute if blocked
            thread::sleep(Duration::from_millis(10));
        }
    }
}
