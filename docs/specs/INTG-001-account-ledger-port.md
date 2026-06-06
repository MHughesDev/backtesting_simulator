# Spec: INTG-001 — Account (Ledger Port)

**Spec ID:** INTG-001
**Type:** Integration (external, caller-owned ledger)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**Related:** [ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md), [run-request.md](DATA-002-run-request.md) §5


---

## 1. Purpose

The simulator does **not own or persist the portfolio ledger**. Instead, it calls an injected
`Account` port (a caller-provided interface) whenever it needs to:

- Query current **equity** (cash + mark-to-market holdings)
- Query **positions** (what instruments we hold, how many, at what cost basis)
- Query **buying power** (available capital for new trades, accounting for margin/collateral requirements)
- Query **collateral balance** (for margin products: perps, options, short positions)
- **Report fills** (tell the ledger about executed trades so it updates)

The Account lives in the **trading platform**. The simulator is stateless with respect to portfolio.

---

## 2. Why not owned by the simulator?

| Concern | Owner | Why |
|---|---|---|
| **Live trading ledger** | Platform | Live and backtest must use identical order/execution semantics; the ledger must be parity. |
| **Multi-account support** | Platform | A platform may manage many accounts; the simulator has no opinion about account identity/ownership. |
| **Settlement rules** | Platform | T+2 equities, T+0 crypto, T+1 futures, etc. — venue-specific and platform policy. |
| **Currency conversion** | Platform | Multi-currency accounting is not the simulator's concern. |
| **Accounting, tax** | Platform | Cost basis tracking, tax-lot selection, wash-sale rules. |
| **Risk limits** | Platform | Portfolio-level exposure/correlation limits are above the simulator's layer. |

The simulator cares only about **fills and mark-to-market**. The ledger cares about everything else.

---

## 3. The Account interface

```rust
pub trait Account {
  /// Current cash (or primary currency balance)
  fn cash(&self) -> Decimal;

  /// Total equity = cash + mark-to-market holdings value
  fn equity(&self) -> Decimal;

  /// Open positions: instrument_id → (qty, cost_basis_per_unit, mark_price)
  fn positions(&self) -> HashMap<InstrumentId, Position>;

  /// Available buying power given current positions and collateral
  fn buying_power(&self) -> Decimal;

  /// For margin products (perps, options, short): available collateral
  fn collateral_balance(&self) -> Decimal;

  /// Report a fill to the ledger (simulator calls after every trade)
  fn report_fill(&mut self, trade: &TradeRecord);

  /// Force-close a position (used at run end or on liquidation)
  fn force_close(&mut self, instrument_id: &InstrumentId, price: Decimal, reason: &str) -> Result<()>;
}

pub struct Position {
  pub qty: Decimal,
  pub cost_basis_per_unit: Decimal,
  pub mark_price: Decimal,
  pub unrealized_pnl: Decimal,
}
```

All methods are **synchronous** — no I/O, no network, no delays. Account state is read-only
except for `report_fill` and `force_close`, which update atomically.

---

## 4. Acceptance criteria

A conforming Account implementation must satisfy:

1. **Query Correctness**
   - [ ] `equity()` equals cash + sum of (position qty × mark price) across all open positions
   - [ ] `buying_power()` is deterministic and non-negative
   - [ ] `positions()` returns only open positions (zero or closed-out positions not listed)
   - [ ] Mark prices reflect the most recent observed price for each instrument

2. **Fill Reporting**
   - [ ] `report_fill()` updates cash and positions atomically in a single call
   - [ ] After a buy fill: qty increases, cash decreases
   - [ ] After a sell fill: qty decreases, cash increases
   - [ ] Fills are idempotent: reporting the same trade twice does not double-count

3. **Liquidation Support** (if strategy uses margin)
   - [ ] `force_close()` terminates a position at a given price
   - [ ] Portfolio can be fully liquidated at run end

4. **State Management**
   - [ ] Account is initialized once per run with starting capital
   - [ ] All subsequent state changes go through `report_fill()` or `force_close()`
   - [ ] No silent failures: errors are propagated to the simulator

5. **Latency**
   - [ ] All Account methods return in < 1ms (read operations sub-microsecond for in-memory state)
   - [ ] No blocking I/O, network, or disk reads

6. **Thread Safety** (if account is shared across parallel runs)
   - [ ] Account is `Send + Sync`
   - [ ] Multiple threads can query safely (read-only operations)
   - [ ] Write operations (`report_fill`, `force_close`) are atomic

---

## 5. Example implementation (reference)

A minimal, correct Account implementation (in-memory, single-currency):

```rust
pub struct SimpleAccount {
  cash: Decimal,
  positions: HashMap<InstrumentId, (Decimal, Decimal)>, // (qty, cost_basis)
  mark_prices: HashMap<InstrumentId, Decimal>,
}

impl SimpleAccount {
  pub fn new(starting_cash: Decimal) -> Self {
    Self {
      cash: starting_cash,
      positions: Default::default(),
      mark_prices: Default::default(),
    }
  }

  pub fn update_mark_price(&mut self, instrument_id: InstrumentId, price: Decimal) {
    self.mark_prices.insert(instrument_id, price);
  }
}

impl Account for SimpleAccount {
  fn cash(&self) -> Decimal {
    self.cash
  }

  fn equity(&self) -> Decimal {
    let holdings_value: Decimal = self.positions.iter()
      .map(|(id, (qty, _))| {
        qty * self.mark_prices.get(id).copied().unwrap_or_zero()
      })
      .sum();
    self.cash + holdings_value
  }

  fn positions(&self) -> HashMap<InstrumentId, Position> {
    self.positions.iter()
      .map(|(id, (qty, cost_basis))| {
        let mark_price = self.mark_prices.get(id).copied().unwrap_or_zero();
        (id.clone(), Position {
          qty: *qty,
          cost_basis_per_unit: *cost_basis,
          mark_price,
          unrealized_pnl: (*qty * mark_price) - (*qty * cost_basis),
        })
      })
      .collect()
  }

  fn buying_power(&self) -> Decimal {
    self.cash // Simplified: no margin, no leverage
  }

  fn collateral_balance(&self) -> Decimal {
    // For margin products, this would account for initial/maintenance margin
    self.equity()
  }

  fn report_fill(&mut self, trade: &TradeRecord) {
    let side_sign = if trade.side == TradeSide::Buy { 1 } else { -1 };
    let cost = side_sign * trade.qty * trade.fill_price;
    self.cash -= cost;

    let entry = self.positions.entry(trade.instrument_id.clone()).or_insert((Decimal::ZERO, Decimal::ZERO));
    entry.0 += side_sign * trade.qty;
    entry.1 = trade.fill_price; // Simplification: FIFO or weighted-average cost basis
  }

  fn force_close(&mut self, instrument_id: &InstrumentId, price: Decimal, _reason: &str) -> Result<()> {
    if let Some((qty, _)) = self.positions.remove(instrument_id) {
      self.cash += qty * price;
    }
    Ok(())
  }
}
```

---

## 6. Integration with Run Request

The Account is bound in the **Run Request** under `account`:

```jsonc
{
  "account": {
    "provider": "simple_account",  // Simulator calls this injected handler
    "initial_capital": 100000,     // Starting cash
    "currency": "USD"
  }
}
```

The simulator never instantiates the Account — the platform provides it at run time.

---

## 7. Key invariants

1. **No portfolio assumed.** The simulator assumes zero opening positions. If a backtest should
   start with existing holdings, the Account's `positions()` returns them; the simulator does not.

2. **Cash not owned by the simulator.** Anything the simulator needs to know about capital
   (buying power, margin balance) it **queries** from the Account, never assumes.

3. **Fills reported, not stored.** Every fill (successful trade) is reported to the Account
   immediately. The simulator does not maintain a fill history; the Account does.

4. **Atomic updates.** A single `report_fill()` call updates both position quantity and cash
   atomically. The simulator never leaves the Account in an inconsistent state.

5. **Mark-to-market from engine.** The Account queries mark prices; the **engine** provides them
   (current bid/ask, last trade, NAV, etc.). The Account does not decide fill prices.

---

## 8. Non-requirements

- **No account ownership.** The simulator does not own or persist accounts.
- **No multi-account orchestration.** If a strategy should trade across multiple accounts, that
  is platform logic, not simulator logic.
- **No settlement lag modeling.** T+2, T+0, etc. — if the platform needs to model settlement,
  it owns that Account extension.
- **No currency conversion.** All Account operations are in a single base currency (determined
  by the platform).
- **No tax / cost basis** tracking in the core Account. The platform may add those fields.
