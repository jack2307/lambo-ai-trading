//! The method-level indicators, pinned against the research Python on real bars.
//!
//! THE GATE FOR THESE THREE PORTS. `supertrend`, `zigzag` and `avwap` are
//! offered on the chart WITH the numbers the 2026-09-18 bias study measured
//! them at (`fd_indicators::MEASURED`, served on `/api/chart/catalog`). A
//! number is about a series, so the series the chart draws has to be the
//! series the study measured - not one that matches its description. The
//! zigzag has already shown how wide that gap is: three of four independent
//! implementation choices differed from the Python and all three still
//! reproduced the owner's morning table.
//!
//! So each fixture here is real XAUUSD H1 bars carrying the Python's own
//! output, written by `py/research/method_fixtures.py`, and each test agrees
//! bar for bar or prints how many bars it does not.
//!
//! The bars are the 2,000 in `zigzag-h1-xauusd.csv`: the last 2,000 of the
//! 25,708-bar file the study ran on, May to September 2026. What that slice
//! cannot exercise is the broker's winter offset - it is summer throughout -
//! and that branch is covered by a unit test on constructed bars in
//! `fd-indicators` rather than pretended at here.

use fd_core::types::Bar;
use fd_indicators::{IndicatorSpec, compute_indicators};

/// A fixture row: the bar, and whatever the Python published for it.
struct Fixture {
    bars: Vec<Bar>,
    /// Column values by header name, `None` where the Python emitted nothing.
    columns: Vec<(String, Vec<Option<f64>>)>,
    /// Non-numeric columns (the zigzag's UP/DOWN/FLAT labels), by name.
    labels: Vec<(String, Vec<String>)>,
}

/// Parse one of the fixtures. The last `#` comment line is the header.
fn read_fixture(text: &str) -> Fixture {
    let header = text
        .lines()
        .filter(|l| l.starts_with('#'))
        .next_back()
        .expect("a header comment")
        .trim_start_matches('#')
        .trim();
    let names: Vec<&str> = header.split(',').map(str::trim).collect();
    assert_eq!(&names[..5], &["time_ms", "open", "high", "low", "close"], "fixture shape");

    let mut bars = Vec::new();
    let mut extra: Vec<Vec<String>> = vec![Vec::new(); names.len() - 5];
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), names.len(), "row: {line}");
        bars.push(Bar {
            time: f[0].parse().expect("time"),
            open: f[1].parse().expect("open"),
            high: f[2].parse().expect("high"),
            low: f[3].parse().expect("low"),
            close: f[4].parse().expect("close"),
            volume: None,
        });
        for (i, cell) in f[5..].iter().enumerate() {
            extra[i].push((*cell).to_string());
        }
    }

    let mut columns = Vec::new();
    let mut labels = Vec::new();
    for (i, name) in names[5..].iter().enumerate() {
        let raw = &extra[i];
        // A column is numeric when every non-empty cell parses. The zigzag's
        // label column does not, and is kept as text rather than coerced.
        if raw.iter().all(|c| c.is_empty() || c.parse::<f64>().is_ok()) {
            let vals = raw
                .iter()
                .map(|c| if c.is_empty() { None } else { Some(c.parse::<f64>().expect("number")) })
                .collect();
            columns.push(((*name).to_string(), vals));
        } else {
            labels.push(((*name).to_string(), raw.clone()));
        }
    }
    Fixture { bars, columns, labels }
}

fn column<'a>(fixture: &'a Fixture, name: &str) -> &'a [Option<f64>] {
    &fixture.columns.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("column {name}")).1
}

/// Compare a computed series against a fixture column and report the damage.
///
/// Not a bare `assert_eq!` per bar: the first mismatch tells you nothing
/// about whether a port is off by a rounding on one bar or has the wrong
/// recursion. The count, the worst difference and where the mismatches sit
/// tell you which - the brief for these ports says to report the count and
/// where they cluster rather than loosen the assertion, and this is what
/// produces that sentence.
fn assert_matches(name: &str, got: &[f64], want: &[Option<f64>]) {
    assert_eq!(got.len(), want.len(), "{name}: length");
    let mut wrong: Vec<usize> = Vec::new();
    let mut worst = 0.0f64;
    let mut worst_at = 0usize;
    for i in 0..got.len() {
        let ok = match want[i] {
            // An empty cell is the Python declining to answer, and the port
            // must decline too. A number where the study had none is not a
            // rounding difference, it is a fabricated warmup value.
            None => got[i].is_nan(),
            Some(v) => {
                let diff = (got[i] - v).abs();
                if got[i].is_finite() && diff > worst {
                    worst = diff;
                    worst_at = i;
                }
                fd_core::parity_eq(got[i], v)
            }
        };
        if !ok {
            wrong.push(i);
        }
    }
    assert!(
        wrong.is_empty(),
        "{name}: {} of {} bars differ from the Python; first at {}, last at {}, \
         worst finite difference {worst:e} at bar {worst_at}",
        wrong.len(),
        got.len(),
        wrong.first().copied().unwrap_or(0),
        wrong.last().copied().unwrap_or(0),
    );
    // Said out loud, because a test that passes on an all-NaN series would
    // also pass on a port that computes nothing.
    assert!(got.iter().any(|v| v.is_finite()), "{name}: nothing was computed");
    println!("{name}: {} bars, worst difference {worst:e} at bar {worst_at}", got.len());
}

#[test]
fn supertrend_matches_the_research_definition() {
    // `bias_defs.py::supertrend(bars, 10, 3.0)` — the `supertrend(10,3)` row:
    // 2.5 flips per 100 H1 bars, 2% undone within three, 13 bars of lag, 19%
    // of turns missed. Those numbers are served beside this indicator, so
    // this is the test that keeps them true of the line on the chart.
    let fixture = read_fixture(include_str!("fixtures/supertrend-h1-xauusd.csv"));
    let spec = IndicatorSpec::new("supertrend").with("period", 10.0).with("mult", 3.0);
    let got = compute_indicators(&fixture.bars, std::slice::from_ref(&spec)).expect("computed");

    assert_matches(
        "supertrend line",
        got.get("supertrend_10_3.supertrend").expect("the line"),
        column(&fixture, "supertrend"),
    );
    assert_matches(
        "supertrend direction",
        got.get("supertrend_10_3.direction").expect("the direction"),
        column(&fixture, "direction"),
    );

    // The warmup is real and is not a rounding: ATR(10) is not finite until
    // bar 9, and the Python emits no direction at all there.
    let direction = got.get("supertrend_10_3.direction").expect("the direction");
    assert!(direction[8].is_nan(), "ATR(10) is not warm at bar 8");
    assert!(direction[9].is_finite(), "and is at bar 9");
}

#[test]
fn zigzag_indicator_matches_the_research_definition() {
    // THE SAME FIXTURE THE STRUCTURE ROW IS PINNED BY, read through the
    // indicator instead of through `htf::structure_zigzag`. That is the whole
    // point of moving the zigzag into `fd-indicators`: the line a viewer
    // draws on the Desk and the label the H1 row prints are now one function,
    // so they cannot disagree, and this test says so in the one place where
    // both spellings are visible at once.
    let fixture = read_fixture(include_str!("fixtures/zigzag-h1-xauusd.csv"));
    let want = &fixture.labels.iter().find(|(n, _)| n == "label").expect("labels").1;

    let spec = IndicatorSpec::new("zigzag").with("k", 3.0).with("atrPeriod", 14.0);
    let got = compute_indicators(&fixture.bars, std::slice::from_ref(&spec)).expect("computed");
    let direction = got.get("zigzag_3_14.direction").expect("the direction");
    let line = got.get("zigzag_3_14.zigzag").expect("the line");

    let as_label = |v: f64| {
        if v > 0.0 {
            "UP"
        } else if v < 0.0 {
            "DOWN"
        } else {
            "FLAT"
        }
    };
    let wrong: Vec<usize> = (0..want.len()).filter(|&i| as_label(direction[i]) != want[i]).collect();
    assert!(
        wrong.is_empty(),
        "{} of {} labels differ from the research definition; first at bar {}",
        wrong.len(),
        want.len(),
        wrong.first().copied().unwrap_or(0)
    );

    // The line holds the CONFIRMED pivot and is NaN until the first one, at
    // the same bar the direction stops being FLAT. A level published before
    // a pivot is confirmed would be a repaint waiting to happen.
    let first = want.iter().position(|l| l != "FLAT").expect("a pivot in 2000 bars");
    assert!(line[first - 1].is_nan(), "no level before the first confirmed pivot");
    assert!(line[first].is_finite(), "a level from the bar the first pivot is confirmed on");
    // 290 FLAT bars: a 2000-bar slice restarts both the ATR(14) warmup and
    // the wait for the first threshold cross. Asserted in the structure
    // row's own test too, and repeated here so the two cannot drift apart.
    assert_eq!(want.iter().filter(|l| *l == "FLAT").count(), 290);
}

#[test]
fn anchored_vwap_matches_the_research_definition() {
    // `bias_defs.py::anchored_vwap(bars, 'day'|'week')`. Both anchors, from
    // one fixture, because the difference between them is the entire content
    // of the `anchor` parameter and a test of one would not notice the other
    // being wired to the same bucket.
    let fixture = read_fixture(include_str!("fixtures/avwap-h1-xauusd.csv"));

    for (anchor, column_name) in [(1.0, "avwapDay"), (7.0, "avwapWeek")] {
        let spec = IndicatorSpec::new("avwap").with("anchor", anchor);
        let got = compute_indicators(&fixture.bars, std::slice::from_ref(&spec)).expect("computed");
        let key = format!("avwap_{}", if anchor == 1.0 { "1" } else { "7" });
        assert_matches(&key, got.get(&key).expect("the series"), column(&fixture, column_name));
    }

    // The two anchors must not produce the same series. They agree on the
    // first bars of a week - one bar into Monday the week mean IS the day
    // mean - and diverge after, which is the shape to assert rather than
    // "they differ somewhere".
    let day = compute_indicators(&fixture.bars, &[IndicatorSpec::new("avwap").with("anchor", 1.0)]).unwrap();
    let week = compute_indicators(&fixture.bars, &[IndicatorSpec::new("avwap").with("anchor", 7.0)]).unwrap();
    let differing = (0..fixture.bars.len())
        .filter(|&i| (day["avwap_1"][i] - week["avwap_7"][i]).abs() > 1e-9)
        .count();
    assert!(differing > fixture.bars.len() / 2, "the week anchor is not the day anchor ({differing} bars differ)");

    // An anchor that is neither draws nothing rather than quietly drawing a
    // session line. A viewer who typed 30 into the box must be able to tell.
    let odd = compute_indicators(&fixture.bars, &[IndicatorSpec::new("avwap").with("anchor", 30.0)]).unwrap();
    assert!(odd["avwap_30"].iter().all(|v| v.is_nan()), "an unknown anchor is not a day");
}

#[test]
fn the_catalog_carries_what_each_definition_was_measured_at() {
    // The measurement rides with the definition or it does not exist. This
    // asserts the numbers in the table against the decision note's H1 row,
    // because a typo here is a lie on the picker and nothing else would
    // catch it.
    let zigzag = fd_indicators::measured("zigzag");
    let h1 = zigzag.iter().find(|m| m.timeframe == "1h").expect("an H1 row");
    assert_eq!((h1.flips_per_100_bars, h1.undone_within_3_pct), (6.1, 16.0));
    assert_eq!((h1.median_lag_bars, h1.missed_pct, h1.sample_bars), (6.0, 1.0, 25_708));
    assert!(h1.source.ends_with("2026-09-18-market-bias-definitions.md"));

    let supertrend = fd_indicators::measured("supertrend");
    assert_eq!(supertrend.len(), 2, "H1 and H4 are different numbers about different bars");
    assert!(supertrend.iter().all(|m| m.caution.is_none()));

    // THE WARNING THE STUDY WROTE, carried as data. `avwap day` is named in
    // the note as one not to put on a card; if this ever becomes `None` the
    // picker goes silent about 57% of turns reversing within three bars.
    let avwap = fd_indicators::measured("avwap");
    let day_h1 = avwap
        .iter()
        .find(|m| m.timeframe == "1h" && m.params.contains(&("anchor", 1.0)))
        .expect("avwap day on H1");
    assert_eq!((day_h1.flips_per_100_bars, day_h1.undone_within_3_pct), (19.7, 57.0));
    assert!(day_h1.caution.expect("the study's warning").contains("57%"));
    let week_h1 = avwap
        .iter()
        .find(|m| m.timeframe == "1h" && m.params.contains(&("anchor", 7.0)))
        .expect("avwap week on H1");
    assert_eq!((week_h1.flips_per_100_bars, week_h1.undone_within_3_pct), (9.2, 53.0));

    // Unmeasured is empty, and the route leaves the field off entirely. An
    // SMA has never been measured this way and must not look as though it
    // has.
    assert!(fd_indicators::measured("sma").is_empty());
}

#[test]
fn the_wire_shape_says_what_was_measured_and_stays_silent_otherwise() {
    // The catalog's own serialisation, because the useful failure here is
    // not "the number is wrong" but "the client cannot find it": a params
    // list that arrives as `[["anchor",1.0]]` or a `measured: []` on an SMA
    // both compile, both look fine in Rust, and both mislead the picker.
    use fd_api::dto::{IndicatorInfo, MeasuredDto};
    use std::collections::BTreeMap;

    let row = |def: &'static fd_indicators::IndicatorDef| IndicatorInfo {
        id: def.id.to_string(),
        name: def.name.to_string(),
        pane: "overlay",
        params: def.params.iter().map(|(n, v)| ((*n).to_string(), *v)).collect::<BTreeMap<_, _>>(),
        outputs: def.outputs.iter().map(|o| (*o).to_string()).collect(),
        measured: {
            let rows = fd_indicators::measured(def.id);
            (!rows.is_empty()).then(|| rows.iter().map(MeasuredDto::from).collect::<Vec<_>>())
        },
    };

    let avwap = serde_json::to_value(row(fd_indicators::definition("avwap").unwrap())).unwrap();
    let measured = avwap["measured"].as_array().expect("avwap is measured");
    assert_eq!(measured.len(), 3, "avwap day on H1 and H4, avwap week on H1");
    let day_h1 = measured
        .iter()
        .find(|m| m["timeframe"] == "1h" && m["params"]["anchor"] == 1.0)
        .expect("the day row, keyed by the parameter cell it describes");
    assert_eq!(day_h1["flipsPer100Bars"], 19.7);
    assert_eq!(day_h1["undoneWithin3Pct"], 57.0);
    assert_eq!(day_h1["medianLagBars"], 3.0);
    assert_eq!(day_h1["missedPct"], 2.0);
    assert_eq!(day_h1["sampleBars"], 25_708);
    assert_eq!(day_h1["definition"], "avwap day");
    assert!(day_h1["source"].as_str().unwrap().starts_with("docs/decisions/"));
    assert!(
        day_h1["caution"].as_str().expect("the study's warning").contains("not to put this on a card"),
        "the warning must reach the wire, or the picker offers it silently"
    );
    // The week row is measured too and is NOT under the same warning: the
    // note names the day anchor, not the week, and inventing a warning it
    // did not write would be as wrong as dropping the one it did.
    let week_h1 = measured.iter().find(|m| m["params"]["anchor"] == 7.0).expect("the week row");
    assert!(week_h1.get("caution").is_none());

    // Silence, not an empty list, for anything unmeasured.
    let sma = serde_json::to_value(row(fd_indicators::definition("sma").unwrap())).unwrap();
    assert!(sma.get("measured").is_none(), "an unmeasured definition says nothing: {sma}");
}
