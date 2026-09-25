//! Count each condition of `rsi-reversal-vol` separately, on real bars.
//!
//! The rule took ZERO trades over 100,586 gold bars and 100,798 BTC bars, and
//! `volume-thrust` on the same files took plenty - so the feed has volume and
//! the fault is in the rule. Guessing which of five ANDed conditions never
//! passes is what this avoids: it counts them one at a time and prints the pass
//! rate of each, and of the pairs, so the blocking one names itself.
//!
//!   why_no_trade --market=xauusd --interval=15m --data=E:/rust/flowdesk/data

use fd_core::types::Bar;
use fd_indicators::{compute_indicators, IndicatorSpec};
use fd_store::read_bars;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn shape(b: &Bar) -> (f64, f64, f64, f64) {
    let body = (b.close - b.open).abs();
    let range = b.high - b.low;
    (body, range, b.high - b.close.max(b.open), b.close.min(b.open) - b.low)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = arg("data", "data");
    let market = arg("market", "xauusd");
    let interval = arg("interval", "15m");
    let symbol = if market == "btcusd" { "BTCUSD" } else { "XAUUSD" };
    let path = std::path::Path::new(&data).join("bars").join(format!("{symbol}-{interval}.parquet"));
    let bars = read_bars(&path)?;
    println!("bars: {} from {path:?}", bars.len());

    let set = compute_indicators(
        &bars,
        &[
            IndicatorSpec::new("rsi").with("period", 14.0),
            IndicatorSpec::new("stochrsi")
                .with("rsiPeriod", 14.0)
                .with("period", 14.0)
                .with("smoothK", 3.0)
                .with("smoothD", 3.0),
        ],
    )?;
    // Print the keys the set actually holds, so a naming mistake is visible
    // rather than silently returning NaN for ever.
    let mut keys: Vec<&String> = set.keys().collect();
    keys.sort();
    println!("series keys present: {keys:?}");

    let rsi = set.get("rsi_14:rsi").map(|v| &v[..]).unwrap_or(&[]);
    let k = set.get("stochrsi_14_14_3_3:k").map(|v| &v[..]).unwrap_or(&[]);
    println!("rsi len {}  k len {}", rsi.len(), k.len());
    if rsi.is_empty() || k.is_empty() {
        println!("A SERIES IS MISSING - that alone is the whole answer.");
        return Ok(());
    }
    let finite_rsi = rsi.iter().filter(|v| v.is_finite()).count();
    let finite_k = k.iter().filter(|v| v.is_finite()).count();
    println!("finite rsi {finite_rsi}  finite k {finite_k}");

    let (mut n, mut c_rsi_lo, mut c_rsi_up, mut c_k_up, mut c_vol, mut c_bull) = (0, 0, 0, 0, 0, 0);
    let (mut c_all_but_pattern, mut c_all) = (0, 0);
    let (mut n_star, mut n_outside, mut n_maru) = (0, 0, 0);

    for i in 70..bars.len() {
        if !rsi[i].is_finite() || !rsi[i - 1].is_finite() || !k[i].is_finite() || !k[i - 1].is_finite() {
            continue;
        }
        n += 1;
        let rsi_lo = rsi[i] < 30.0;
        let rsi_up = rsi[i] > rsi[i - 1];
        let k_up = k[i] > k[i - 1];
        let mean = {
            let w = &bars[i - 30..i];
            if w.iter().any(|b| b.volume.unwrap_or(0.0) <= 0.0) {
                None
            } else {
                Some(w.iter().map(|b| b.volume.unwrap()).sum::<f64>() / 30.0)
            }
        };
        let vol_ok = matches!((mean, bars[i].volume), (Some(m), Some(v)) if v > m);

        // the three patterns, counted separately
        let (fb, fr, _, _) = shape(&bars[i - 2]);
        let (mb, mr, _, _) = shape(&bars[i - 1]);
        let (lb, lr, _, _) = shape(&bars[i]);
        let star_bull = fr > 0.0 && mr > 0.0 && lr > 0.0
            && mb / mr <= 0.35
            && fb / fr >= 0.35
            && bars[i - 2].close < bars[i - 2].open
            && bars[i - 1].high < bars[i - 2].close.max(bars[i - 2].open)
            && bars[i].close > bars[i].open
            && bars[i].close > (bars[i - 2].open + bars[i - 2].close) / 2.0
            && lb / lr >= 0.35;
        let outside_bull = bars[i].high > bars[i - 1].high
            && bars[i].low < bars[i - 1].low
            && bars[i].close > bars[i].open;
        let (body, range, up, lo) = shape(&bars[i]);
        let maru_bull = range > 0.0
            && body / range >= 0.9
            && up / range <= 0.05
            && lo / range <= 0.05
            && bars[i].close > bars[i].open;
        if star_bull { n_star += 1; }
        if outside_bull { n_outside += 1; }
        if maru_bull { n_maru += 1; }
        let bull = star_bull || outside_bull || maru_bull;

        if rsi_lo { c_rsi_lo += 1; }
        if rsi_up { c_rsi_up += 1; }
        if k_up { c_k_up += 1; }
        if vol_ok { c_vol += 1; }
        if bull { c_bull += 1; }
        if rsi_lo && rsi_up && k_up && vol_ok { c_all_but_pattern += 1; }
        if rsi_lo && rsi_up && k_up && vol_ok && bull { c_all += 1; }
    }

    let pct = |c: usize| if n == 0 { 0.0 } else { 100.0 * c as f64 / n as f64 };
    println!();
    println!("bars considered: {n}");
    println!("  RSI < 30                 {c_rsi_lo:>8}  {:.2}%", pct(c_rsi_lo));
    println!("  RSI turning up           {c_rsi_up:>8}  {:.2}%", pct(c_rsi_up));
    println!("  StochRSI k turning up    {c_k_up:>8}  {:.2}%", pct(c_k_up));
    println!("  volume over 30-bar mean  {c_vol:>8}  {:.2}%", pct(c_vol));
    println!("  a BULLISH pattern        {c_bull:>8}  {:.2}%", pct(c_bull));
    println!("      of which star        {n_star:>8}");
    println!("      of which outside     {n_outside:>8}");
    println!("      of which marubozu    {n_maru:>8}");
    println!("  all four but the pattern {c_all_but_pattern:>8}  {:.4}%", pct(c_all_but_pattern));
    println!("  ALL FIVE                 {c_all:>8}  {:.4}%", pct(c_all));
    Ok(())
}
