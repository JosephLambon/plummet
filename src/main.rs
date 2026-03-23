pub mod book;
use book::*;

use chrono::Local;
use rust_decimal::dec;

fn main() {
    println!("\nWelcome. This is Joe's Order Book.");
    println!("==================================\n\n");

    let mut order_book = OrderBook::new();

    let bid1 = LimitOrder {
        time_placed: Local::now(),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let bid2 = LimitOrder {
        time_placed: Local::now(),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let bid3 = LimitOrder {
        time_placed: Local::now(),
        limit_price: dec!(1234.5320),
        quantity: dec!(50),
        side: Side::Buy,
    };
    let ask1 = LimitOrder {
        time_placed: Local::now(),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
    };
    let ask2 = LimitOrder {
        time_placed: Local::now(),
        limit_price: dec!(1123.5696),
        quantity: dec!(50),
        side: Side::Sell,
    };
    let ask3 = LimitOrder {
        time_placed: Local::now(),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: Side::Sell,
    };

    order_book.insert(bid1);
    order_book.insert(bid2);
    order_book.insert(bid3);
    order_book.insert(ask1);
    order_book.insert(ask2);
    order_book.insert(ask3);
}
