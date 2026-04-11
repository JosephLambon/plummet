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
pub use event::EngineCommand;

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
                    // let event_tx = event_tx.clone();÷
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
                    id: order.id,
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
                    order_book.orders_placed += 1;

                    let match_event = EngineEvent::OrdersMatched(OrdersMatchedEvent {
                        instrument: order_book.instrument,
                        id: order_book.orders_placed,
                        matched_at: Local::now(),
                        ask_id: result.ask_id,
                        bid_id: result.bid_id,
                        bid_price: result.bid_price,
                        ask_price: result.ask_price,
                    });

                    // Send OrdersMatchedEvent EngineCommand to executor
                    let result = order_book.process(&match_event);

                    // Audit log OrdersMatched event
                    event_tx.send(match_event)?;

                    if let Ok(event) = result {
                        debug!("Trade successfully executed.");
                        event_tx.send(event)?;
                    } else {
                        //  error handling
                        error!("Unable to execute trade.");
                    }
                }
                Ok(CommandOutcome::Continue)
            }
            EngineCommand::CancelOrder(order) => {
                event_tx.send(EngineEvent::OrderCancelled(CancellationEvent {
                    instrument: order.instrument,
                    id: order.id,
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
    use std::sync::mpsc::TryRecvError;

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
}
