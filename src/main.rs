pub mod book;

use std::collections::VecDeque;

use book::*;

use chrono::Local;
use rust_decimal::dec;

fn main() {
    println!("\nWelcome. This is Joe's Order Book.");
    println!("==================================\n\n");

    let mut order_book = OrderBook::new();

    let order1 = LimitOrder {
        time_placed: Local::now(),
        stock_symbol: String::from("GOOGL"),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let order2 = LimitOrder {
        time_placed: Local::now(),
        stock_symbol: String::from("AAPL"),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
    };

    let mut bid_queue: VecDeque<LimitOrder> = VecDeque::new();
    bid_queue.push_back(order1.clone());
    let mut ask_queue: VecDeque<LimitOrder> = VecDeque::new();
    ask_queue.push_back(order2.clone());

    order_book.bids.insert(order1.limit_price, bid_queue);
    order_book.asks.insert(order2.limit_price, ask_queue);

    println!("BIDS");
    for (index, mut order) in order_book.bids.into_iter().enumerate() {
        let current = order.1.pop_front().unwrap();

        println!(
            "Order {index}: {:?} {} shares of {} at limit price £{}",
            current.side, current.quantity, current.stock_symbol, current.limit_price
        );
        println!("Time placed: {}\n", current.time_placed);
    }

    println!("ASKS");
    for (index, mut order) in order_book.asks.into_iter().enumerate() {
        let current = order.1.pop_front().unwrap();

        println!(
            "Order {index}: {:?} {} shares of {} at limit price £{}",
            current.side, current.quantity, current.stock_symbol, current.limit_price
        );
        println!("Time placed: {}\n", current.time_placed);
    }
}
