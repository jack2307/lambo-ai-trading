//! Normalized market data types.
//!
//! Raw exchange payloads are converted into these shapes at the ingest boundary
//! and nowhere else, which keeps vendor quirks — Deribit quoting in BTC, the
//! gold feed giving closes only — out of every engine downstream.

use serde::{Deserialize, Serialize};

use crate::classify::FlowClass;

/// Call or put.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OptionType {
    Call,
    Put,
}

impl OptionType {
    /// Single-letter form used by both feeds (`C` / `P`).
    #[must_use]
    pub fn from_letter(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().chars().next()? {
            'C' => Some(Self::Call),
            'P' => Some(Self::Put),
            _ => None,
        }
    }

    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Call => 'C',
            Self::Put => 'P',
        }
    }
}

/// Which side crossed the spread.
///
/// This says who was impatient, not who opened a position. Nothing in this
/// codebase may infer "bought to open" from it — the tapes carry no
/// open/close flag, and pretending otherwise is the most common way an
/// options-flow system fools itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AggressorSide {
    Buy,
    Sell,
    Unknown,
}

impl AggressorSide {
    /// Accepts the spellings both feeds use: `LONG`/`SHORT` on the gold tape,
    /// `buy`/`sell` from Deribit.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "BUY" | "LONG" | "B" => Self::Buy,
            "SELL" | "SHORT" | "S" => Self::Sell,
            _ => Self::Unknown,
        }
    }
}

/// How far out a contract expires, used to weight its levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExpirationType {
    Daily,
    Weekly,
    Monthly,
}

impl ExpirationType {
    /// Fallback classification when the feed does not label a contract.
    #[must_use]
    pub fn from_dte(dte: f64) -> Self {
        if dte <= 1.0 {
            Self::Daily
        } else if dte <= 7.0 {
            Self::Weekly
        } else {
            Self::Monthly
        }
    }
}

/// Flags that travel with a print. They are hints from the venue, never
/// conclusions: a leg of a condor is *flagged* multi-leg, and the engines are
/// expected to treat it with suspicion rather than drop it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeFlags {
    pub block: bool,
    pub sweep: bool,
    pub multi_leg: bool,
    pub spread: bool,
}

/// One option print, normalized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionTrade {
    /// Venue trade id where one exists, otherwise a synthesised key.
    pub id: String,
    /// Epoch milliseconds.
    pub timestamp: i64,
    /// The **expiry** contract, not the instrument.
    ///
    /// Deribit names an instrument by expiry *and* strike
    /// (`BTC-18SEP26-83000-C`). Using that as the contract makes every strike
    /// its own expiration context, which reduces a max pain or a strike profile
    /// to a single point and silently discards most of a tape. The contract is
    /// `BTC-18SEP26`; the strike is a field.
    pub symbol: String,
    /// Full venue instrument name, kept for traceability.
    pub instrument: Option<String>,
    pub underlying: String,
    /// Epoch milliseconds of expiry.
    pub expiration: i64,
    /// Days to expiration at the time of the print.
    pub dte: f64,
    pub strike: f64,
    pub option_type: OptionType,
    /// Quoted option price, in whatever unit the venue uses. Converting it to
    /// USD is the market's job, not this struct's.
    pub trade_price: f64,
    pub contracts: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub aggressor_side: AggressorSide,
    pub flow_class: FlowClass,
    /// Always USD, whatever the quote convention was.
    pub premium_usd: f64,
    pub underlying_price: f64,
    pub exchange: Option<String>,
    pub sequence_id: Option<String>,
    pub implied_volatility: Option<f64>,
    pub flags: TradeFlags,
    /// Where this row came from, so a derived number can always be traced back.
    pub source: String,
}

impl OptionTrade {
    /// A stable identity derived from the print itself.
    ///
    /// Some feeds carry no venue trade id — the gold tape has none — and a
    /// caller that numbered prints by position would mint a different id every
    /// time the source was re-read. Anything de-duplicating by id would then
    /// see a whole new tape rather than the same one, and every premium total
    /// downstream would double.
    ///
    /// FNV-1a rather than [`std::collections::hash_map::DefaultHasher`]: the
    /// standard hasher is explicitly not guaranteed stable between Rust
    /// releases, and an identity that changes when the compiler is upgraded is
    /// not an identity.
    #[must_use]
    pub fn content_id(&self) -> String {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut hash = OFFSET;
        let mut eat = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(PRIME);
            }
        };
        eat(self.symbol.as_bytes());
        eat(self.instrument.as_deref().unwrap_or("").as_bytes());
        eat(&self.timestamp.to_le_bytes());
        eat(&self.strike.to_bits().to_le_bytes());
        eat(&[self.option_type.letter() as u8]);
        eat(&self.trade_price.to_bits().to_le_bytes());
        eat(&self.contracts.to_bits().to_le_bytes());
        eat(&[match self.aggressor_side {
            AggressorSide::Buy => b'B',
            AggressorSide::Sell => b'S',
            AggressorSide::Unknown => b'?',
        }]);
        format!("{hash:016x}")
    }

    /// Signed contribution to positioning at the strike: buyer-aggressed prints
    /// add, seller-aggressed prints subtract.
    #[must_use]
    pub fn signed_contracts(&self) -> f64 {
        f64::from(crate::classify::position_sign(self.flow_class)) * self.contracts
    }
}

/// One price bar.
///
/// `volume` is `None` where the source has none. The gold feed publishes closes
/// only, so a one-minute "bar" from it has `open == high == low == close`;
/// [`Bar::is_synthetic`] says so rather than letting a flat range look like a
/// quiet market.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// Epoch milliseconds of the bar's open.
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<f64>,
}

impl Bar {
    #[must_use]
    pub fn flat(time: i64, close: f64) -> Self {
        Self { time, open: close, high: close, low: close, close, volume: None }
    }

    /// True when the bar carries no range of its own.
    #[must_use]
    pub fn is_synthetic(&self) -> bool {
        self.high == self.low
    }

    #[must_use]
    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    /// Typical price, the input several indicators expect.
    #[must_use]
    pub fn hlc3(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }
}

/// One scheduled economic release, as the calendar store holds it.
///
/// Lives in the domain core rather than in the strategy crate so that the
/// Parquet store can read it without depending on the strategies: the store
/// reads files, the strategy crate installs the list once and every `news:`
/// filter reads it. The blackout keys on `impact` (3 = high) and, per market,
/// on `currency` (`[markets.<id>.trading] news_currencies`); an event whose
/// currency is `All` belongs to every market.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsEvent {
    /// Epoch milliseconds, UTC.
    pub time: i64,
    /// 1 low, 2 medium, 3 high.
    pub impact: u8,
    /// `"USD"`, `"EUR"`, … or `"All"`.
    pub currency: String,
    /// The release's name (`"Non-Farm Employment Change"`), for a status
    /// line; empty when the source did not say.
    #[serde(default)]
    pub name: String,
}

impl NewsEvent {
    /// True when this event belongs to a market whose news currencies are
    /// `currencies`: the list is `None` or empty (every currency), the event
    /// is global (`All`, any case), or its currency is in the list
    /// (case-insensitive).
    #[must_use]
    pub fn concerns(&self, currencies: Option<&[String]>) -> bool {
        match currencies {
            None | Some([]) => true,
            Some(list) => {
                self.currency.eq_ignore_ascii_case("All") || list.iter().any(|c| c.eq_ignore_ascii_case(&self.currency))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggressor_parses_both_feed_spellings() {
        assert_eq!(AggressorSide::parse("LONG"), AggressorSide::Buy);
        assert_eq!(AggressorSide::parse("buy"), AggressorSide::Buy);
        assert_eq!(AggressorSide::parse("SHORT"), AggressorSide::Sell);
        assert_eq!(AggressorSide::parse("sell"), AggressorSide::Sell);
        assert_eq!(AggressorSide::parse("?"), AggressorSide::Unknown);
    }

    #[test]
    fn option_type_round_trips_through_its_letter() {
        assert_eq!(OptionType::from_letter("C"), Some(OptionType::Call));
        assert_eq!(OptionType::from_letter("p"), Some(OptionType::Put));
        assert_eq!(OptionType::from_letter("x"), None);
        assert_eq!(OptionType::Call.letter(), 'C');
    }

    #[test]
    fn expiration_type_buckets_match_the_prototype() {
        assert_eq!(ExpirationType::from_dte(0.1), ExpirationType::Daily);
        assert_eq!(ExpirationType::from_dte(6.0), ExpirationType::Weekly);
        assert_eq!(ExpirationType::from_dte(45.0), ExpirationType::Monthly);
    }

    #[test]
    fn a_close_only_bar_reports_itself_as_synthetic() {
        let bar = Bar::flat(0, 4350.0);
        assert!(bar.is_synthetic());
        assert_eq!(bar.range(), 0.0);
        assert!(!Bar { high: 10.0, low: 8.0, ..Bar::flat(0, 9.0) }.is_synthetic());
    }
}

/// Give every print a stable id, disambiguating prints identical in content.
///
/// [`OptionTrade::content_id`] hashes what traded, which means two separate
/// fills at the same millisecond, the same price, the same size and the same
/// side on the same instrument land on one id. They are still two trades. On
/// the prototype's own tapes that collision was not rare: collapsing them lost
/// 419 of 11,770 gold prints and 422 of 6,440 BTC prints — about 3.6% of the
/// premium that actually crossed.
///
/// So the nth print sharing a hash gets `-n` appended. That stays stable across
/// re-runs as long as the source is read in the same order, which it is: tapes
/// are sorted by timestamp, and prints with byte-identical content are
/// interchangeable, so which one is `-0` does not matter.
///
/// A feed that supplies real venue trade ids should use those instead; this is
/// for the feeds that do not.
pub fn assign_content_ids(trades: &mut [OptionTrade]) {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for trade in trades.iter_mut() {
        let hash = trade.content_id();
        let count = seen.entry(hash.clone()).or_insert(0);
        trade.id = if *count == 0 { hash } else { format!("{hash}-{count}") };
        *count += 1;
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    fn print_of(price: f64) -> OptionTrade {
        OptionTrade {
            id: String::new(),
            timestamp: 1_788_000_000_000,
            symbol: "BTC-18SEP26".into(),
            instrument: Some("BTC-18SEP26-83000-C".into()),
            underlying: "BTC-PERP".into(),
            expiration: 1_789_718_400_000,
            dte: 20.0,
            strike: 83_000.0,
            option_type: OptionType::Call,
            trade_price: price,
            contracts: 2.0,
            bid: None,
            ask: None,
            aggressor_side: AggressorSide::Buy,
            flow_class: crate::classify::FlowClass::Lc,
            premium_usd: 1600.0,
            underlying_price: 80_000.0,
            exchange: None,
            sequence_id: None,
            implied_volatility: None,
            flags: TradeFlags::default(),
            source: "test".into(),
        }
    }

    #[test]
    fn the_same_print_always_gets_the_same_id() {
        assert_eq!(print_of(0.01).content_id(), print_of(0.01).content_id());
    }

    #[test]
    fn a_different_print_gets_a_different_id() {
        assert_ne!(print_of(0.01).content_id(), print_of(0.02).content_id());
    }

    #[test]
    fn two_identical_prints_stay_two_prints() {
        // The bug this exists to prevent: a tape carrying the same fill twice
        // is a tape with two fills on it, and de-duplicating by a pure content
        // hash silently deletes one of them.
        let mut tape = vec![print_of(0.01), print_of(0.01), print_of(0.02)];
        assign_content_ids(&mut tape);
        let ids: std::collections::HashSet<&str> = tape.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids.len(), 3, "identical prints must still get distinct ids");
    }

    #[test]
    fn assigning_ids_twice_over_the_same_tape_gives_the_same_ids() {
        let mut once = vec![print_of(0.01), print_of(0.01), print_of(0.02)];
        let mut twice = once.clone();
        assign_content_ids(&mut once);
        assign_content_ids(&mut twice);
        let left: Vec<&str> = once.iter().map(|t| t.id.as_str()).collect();
        let right: Vec<&str> = twice.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(left, right, "a re-migration must not mint new ids");
    }

    #[test]
    fn fields_that_are_not_part_of_the_print_do_not_change_its_id() {
        // `source` and `id` describe how the row reached us, not what traded.
        // A print backfilled over REST and the same print seen live must land
        // on one identity or the store will keep both.
        let mut live = print_of(0.01);
        live.source = "deribit-ws".into();
        live.id = "12345".into();
        let mut backfilled = print_of(0.01);
        backfilled.source = "deribit-rest".into();
        assert_eq!(live.content_id(), backfilled.content_id());
    }
}
