# Plummet

A simulated Limit Order Book written in Rust.

## 🏗️ WIP
```
- Writing by hand, to aid learning Rust
- AI used only for writing user stories
```

### Architecture

#### Engine
- One thread per Instrument. Uses message passing via **channels**
- Within each thread:
    - Listens for **commands**
    - **Emits events**, driving the order book to match/execute trades

#### Order Book
- **Listens for events** emitted by Engine
- **Handles** ask/buy sides
- **Matches** orders
- **Executes** trades


1. Implement OrderMatchedEvent match_event_id
2. Implement Guard code to OrderBook::insert() that checks for order state