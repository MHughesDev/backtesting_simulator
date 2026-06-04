# Asset Taxonomy

The backtesting suite covers eleven asset classes. This folder contains one spec per class.
Each spec answers three questions driven by the research phase:

1. **What the asset is** — mechanics, subtypes, how it trades.
2. **What a proper backtest requires** — data, mechanics that must be simulated, what gets
   wrong if you skip something.
3. **Implications** — how this asset's needs shape the contracts, the engine, and the metrics.

## Key principle: asset ≠ engine

The asset class does not determine the engine. The instrument's `price_formation` field does.
Two assets in the same "class" can use different engines:
- `BTC-USD @ Coinbase` (CEX spot) → Engine A (CLOB)
- `WETH/USDC @ Uniswap v3` (DEX) → Engine B (AMM)

Both are "crypto" — completely different execution mechanics.

## Index

| File | Asset class | Engine(s) |
|---|---|---|
| [equities.md](equities.md) | Stocks, REITs, ADRs | A |
| [etfs.md](etfs.md) | ETFs, leveraged/inverse, mutual funds | A + C |
| [crypto-spot-cex.md](crypto-spot-cex.md) | Crypto spot on centralized exchanges | A |
| [dex-amm.md](dex-amm.md) | DEX pools (Uniswap, Raydium, Curve) | B |
| [futures.md](futures.md) | Expiring futures (equity, commodity, crypto) | A |
| [perpetuals.md](perpetuals.md) | Perpetual swaps (linear, inverse) | A |
| [options.md](options.md) | Options, warrants (equity, ETF, index, crypto) | E |
| [bonds.md](bonds.md) | Bonds, treasuries, MBS, CDs | D |
| [fx.md](fx.md) | Currency pairs (spot FX) | A |
| [nfts.md](nfts.md) | NFTs, digital collectibles | G |
| [prediction-markets.md](prediction-markets.md) | Binary event contracts | H |
