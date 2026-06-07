use std::collections::BTreeMap;

use rust_decimal::Decimal;

use contracts::{BookAction, MarketPayload, Side};

/// The order book view maintained by Engine A.
/// Bids are stored keyed by price (best bid = highest price).
/// Asks are stored keyed by price (best ask = lowest price).
#[derive(Debug, Default, Clone)]
pub struct BookView {
    /// bid price → size
    pub bids: BTreeMap<Decimal, Decimal>,
    /// ask price → size
    pub asks: BTreeMap<Decimal, Decimal>,
}

impl BookView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a BookDelta event to the book.
    pub fn apply_delta(
        &mut self,
        side: &Side,
        price: Decimal,
        new_size: Decimal,
        action: &BookAction,
    ) {
        let book = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
            Side::Unknown => return,
        };
        match action {
            BookAction::Add | BookAction::Modify => {
                if new_size.is_zero() || new_size.is_sign_negative() {
                    book.remove(&price);
                } else {
                    book.insert(price, new_size);
                }
            }
            BookAction::Delete => {
                book.remove(&price);
            }
            BookAction::Clear => {
                book.clear();
            }
        }
    }

    /// Replace the book with a snapshot.
    pub fn apply_snapshot(&mut self, bids: &[(Decimal, Decimal)], asks: &[(Decimal, Decimal)]) {
        self.bids.clear();
        self.asks.clear();
        for (price, size) in bids {
            if !size.is_zero() && !size.is_sign_negative() {
                self.bids.insert(*price, *size);
            }
        }
        for (price, size) in asks {
            if !size.is_zero() && !size.is_sign_negative() {
                self.asks.insert(*price, *size);
            }
        }
    }

    /// Best bid price (highest bid).
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, s)| (*p, *s))
    }

    /// Best ask price (lowest ask).
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, s)| (*p, *s))
    }

    /// Mid price, if both sides have quotes.
    pub fn mid(&self) -> Option<Decimal> {
        let (bid, _) = self.best_bid()?;
        let (ask, _) = self.best_ask()?;
        Some((bid + ask) / Decimal::from(2))
    }

    /// Walk the ask side (for buy orders): return (fill_price, fill_qty, remaining_qty).
    /// `qty` = desired quantity to fill.
    pub fn walk_asks(&self, mut qty: Decimal) -> Vec<(Decimal, Decimal)> {
        let mut fills = Vec::new();
        for (price, size) in &self.asks {
            if qty.is_zero() || qty.is_sign_negative() {
                break;
            }
            let fill_qty = qty.min(*size);
            fills.push((*price, fill_qty));
            qty -= fill_qty;
        }
        fills
    }

    /// Walk the bid side (for sell orders): return (fill_price, fill_qty) pairs.
    pub fn walk_bids(&self, mut qty: Decimal) -> Vec<(Decimal, Decimal)> {
        let mut fills = Vec::new();
        for (price, size) in self.bids.iter().rev() {
            if qty.is_zero() || qty.is_sign_negative() {
                break;
            }
            let fill_qty = qty.min(*size);
            fills.push((*price, fill_qty));
            qty -= fill_qty;
        }
        fills
    }

    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }

    /// Apply a MarketPayload to update the book if applicable.
    pub fn update_from_payload(&mut self, payload: &MarketPayload) {
        match payload {
            MarketPayload::BookDelta {
                side,
                price,
                new_size,
                action,
            } => {
                self.apply_delta(side, *price, *new_size, action);
            }
            MarketPayload::BookSnapshot { bids, asks, .. } => {
                self.apply_snapshot(bids, asks);
            }
            _ => {}
        }
    }
}
