# The logistic model on options features does not generalise

**Date:** 2026-09-12
**Question:** Does a logistic model over the 24 options-flow features predict the
sixty-minute forward move?
**Outcome:** Not on the data available. The in-sample score was memorisation.

## What was measured

The golden feature table, horizon 60m, label band 0.5 ATR, neutral rows dropped.
Training: 800 epochs, lr 0.1, L2 2.0, class weights on. Four-fold walk-forward
with an expanding window, train on the past and test on the future.

```
gold   974 labelled rows (463 up), 380 neutral dropped, baseline 0.5246
       in-sample AUC    0.7699
       walk-forward AUC 0.5210   (776 out-of-sample predictions)
       walk-forward acc 0.4961   vs baseline 0.5464

btc    169 labelled rows (79 up), 95 neutral dropped, baseline 0.5325
       in-sample AUC    0.9541
       walk-forward AUC 0.2440   (132 out-of-sample predictions)
       walk-forward acc 0.3258   vs baseline 0.5152
```

Per-fold AUC on gold: 0.4542, 0.5573, 0.8266, 0.6160 — unstable across folds,
which is what noise looks like when it is measured four times.

## The finding is the gap, not either number

Gold falls 0.77 → 0.52; BTC 0.95 → 0.24. Out-of-sample accuracy is *below* the
majority-class baseline in both: the model is worse than a constant prediction.

Twenty-four features against 169 rows (BTC) is not a fair test of anything. The
useful conclusion is not that options flow fails — it is that **this feature set
is easily memorised**, so every in-sample number produced from it, now and later,
should be treated as uninformative until a walk-forward says otherwise.

## Parity

The Rust implementation matches the JavaScript oracle on all 24 coefficients,
bias, standardiser, sample probabilities, AUC, log loss and baseline accuracy
(`fd-model/tests/parity.rs`). The result above is a property of the data, not of
the port.

## What would reopen this

Materially more tape. The gold window is about five days and BTC's about one.
`fd-ingest --bin collect` accumulates it; the question is worth re-asking when
the labelled row count is in the thousands with the feature count unchanged.

## What this does not say

That the features are wrong, or that a non-linear model would fail. Neither has
been tested on enough data to say.
