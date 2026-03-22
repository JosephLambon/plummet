use std::collections::BTreeMap;

use rust_decimal::{Decimal, dec};

use chrono::prelude::*;

fn main() {
    println!("\nWelcome. This is Joe's Order Book.");
    println!("==================================\n\n");

    let mut orders: BTreeMap<Decimal, LimitOrder> = BTreeMap::new();

    let order1 = LimitOrder {
        time_placed: Local::now(),
        stock_symbol: String::from("GOOGL"),
        limit_price: dec!(1234.5600),
        quantity: dec!(50),
        side: LimitOrderAction::Buy,
    };
    let order2 = LimitOrder {
        time_placed: Local::now(),
        stock_symbol: String::from("AAPL"),
        limit_price: dec!(1123.5698),
        quantity: dec!(50),
        side: LimitOrderAction::Sell,
    };

    orders.insert(order1.limit_price, order1);
    orders.insert(order2.limit_price, order2);

    for (index, order) in orders.iter().enumerate() {
        println!(
            "Order {index}: {:?} {} shares of {} at limit price £{}",
            order.1.side, order.1.quantity, order.1.stock_symbol, order.1.limit_price
        );
        println!("Time placed: {}\n", order.1.time_placed);
    }
}

pub struct LimitOrder {
    time_placed: DateTime<Local>,
    stock_symbol: String,
    limit_price: Decimal,
    quantity: Decimal,
    side: LimitOrderAction,
}

#[derive(Debug)]
pub enum LimitOrderAction {
    Buy,
    Sell,
}
