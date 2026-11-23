//! Analytics' sums (docs/architecture/ANALYTICS.md §3.2; `CallStats`): Home, Outcome, Cost,
//! Speed and Flags, worked out in ehr from the calls' plain columns, never from a sealed one.
//! Names, counts, times and dollars only. `aggregate` is pure, so it is tested without a store;
//! `store.rs › stats_rows` and `stats_tools` read the rows with the filter bar's filters.

use std::collections::BTreeMap;

use crate::store::Stages;

/// The target for one turn, from the caller's silence to the first word they hear
/// (DEBUGGING.md §1; the console's TURN_TARGET_MS).
pub const TURN_TARGET_MS: f64 = 800.0;
/// The wait chart's buckets: this wide, this many, the last one open-ended.
pub const WAIT_BUCKET_MS: f64 = 200.0;
pub const WAIT_BUCKETS: usize = 16;
/// The outcomes, as the store words them (store.rs › summarize).
pub const OUTCOMES: [&str; 6] = ["booked", "handed_off", "abandoned", "crashed", "locked", "ended"];

/// The filter bar's filters (analytics-filter.ts), each empty or false for any.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub origin: String,
    pub outcome: String,
    pub ehr: String,
    pub seat: String,
    /// `prompt`, `system`, `hospital` or `model`, with its label.
    pub version_kind: String,
    pub version_label: String,
    pub flagged: bool,
    pub slow: bool,
}

/// One call's plain columns, as the sums need them.
#[derive(Debug, Clone, Default)]
pub struct StatsRow {
    pub id: String,
    /// The local start, `YYYY-MM-DDTHH:MM:SS`; empty when the record has none.
    pub started: String,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub system_version: Option<String>,
    pub hospital_version: Option<String>,
    pub outcome: String,
    pub end_reason: Option<String>,
    pub reached: Vec<String>,
    pub cost_usd: f64,
    /// The voice's credits by vendor (migration 0015).
    pub voice_credits: BTreeMap<String, f64>,
    pub turn_waits_ms: Vec<f64>,
    pub turn_stages: Vec<Stages>,
    pub requests_replayed: i32,
    /// Each staff flag's kind, source and unverified count, in order.
    pub staff_flags: Vec<(String, String, i32)>,
    pub review_flags: Vec<String>,
}

impl StatsRow {
    /// Its waits are a caller's: no request came from the replay cache.
    fn live(&self) -> bool {
        self.requests_replayed == 0
    }
}

/// One tool call: its tool, its own run in ms (none for an older record), and whether it failed.
#[derive(Debug, Clone, Default)]
pub struct ToolRow {
    pub tool: String,
    pub ms: Option<f64>,
    pub failed: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Totals {
    pub calls: i64,
    /// By `OUTCOMES`.
    pub outcomes: [i64; 6],
    pub cost_usd: f64,
    pub voice: VoiceCost,
    pub flagged_calls: i64,
    pub staff_flags: i64,
    pub review_flags: i64,
    /// Turns answered on calls with no replayed request, and those within the target.
    pub turns_answered: i64,
    pub turns_within: i64,
    pub wait_p50_ms: f64,
    pub wait_p90_ms: f64,
    pub replayed_calls: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Day {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub calls: i64,
    pub outcomes: [i64; 6],
    pub cost_usd: f64,
    pub voice: VoiceCost,
    pub staff_flags: i64,
    pub review_flags: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelCost {
    pub model: String,
    pub calls: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CostlyCall {
    pub id: String,
    pub started: String,
    pub cost_usd: f64,
    pub voice: VoiceCost,
    pub outcome: String,
}

/// Dollars per credit for each vendor whose plan has a price (the voice crate's
/// `cost::usd_per_credit`, from the active voice settings).
pub type Prices = BTreeMap<String, f64>;

/// The voice's cost of some calls: credits by vendor, and in dollars at the active plans'
/// prices. `priced`: every vendor with credits has a price, so `usd` is the whole voice.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoiceCost {
    pub credits: BTreeMap<String, f64>,
    pub usd: f64,
    pub priced: bool,
}

impl VoiceCost {
    fn add(&mut self, credits: &BTreeMap<String, f64>) {
        for (k, v) in credits {
            *self.credits.entry(k.clone()).or_default() += v;
        }
    }

    /// Dollars and `priced`, once every call is added.
    fn priced_at(mut self, prices: &Prices) -> Self {
        self.usd = self.credits.iter().filter_map(|(k, c)| prices.get(k).map(|p| c * p)).sum();
        self.priced = self.credits.iter().all(|(k, c)| *c == 0.0 || prices.contains_key(k));
        self
    }

    fn of(credits: &BTreeMap<String, f64>, prices: &Prices) -> Self {
        let mut v = VoiceCost::default();
        v.add(credits);
        v.priced_at(prices)
    }

    /// All its credits, whatever the vendor: for ordering only.
    fn all_credits(&self) -> f64 {
        self.credits.values().sum()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolSpeed {
    pub tool: String,
    pub calls: i64,
    pub failed: i64,
    pub p50_ms: f64,
    pub p90_ms: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlowTurn {
    pub call_id: String,
    pub started: String,
    /// 1 for the first caller turn.
    pub turn: i32,
    pub wait_ms: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlagCount {
    pub kind: String,
    pub source: String,
    pub count: i64,
    pub unverified: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VersionStats {
    /// `prompt`, `system`, `hospital` or `model`.
    pub kind: String,
    pub label: String,
    pub calls: i64,
    pub booked: i64,
    pub turns_answered: i64,
    pub turns_within: i64,
    pub wait_p50_ms: f64,
    pub wait_p90_ms: f64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub totals: Totals,
    pub days: Vec<Day>,
    /// How many calls reached each step, by the step's name.
    pub reached: Vec<(String, i64)>,
    /// The end reasons, most first, at most 10.
    pub end_reasons: Vec<(String, i64)>,
    pub models: Vec<ModelCost>,
    /// The 5 most costly calls.
    pub costliest: Vec<CostlyCall>,
    /// Turns by wait, `WAIT_BUCKET_MS` wide, the last open-ended.
    pub wait_hist: Vec<i64>,
    /// The mean of each stage over the turns that have stages (live calls only), and how many.
    pub stages: Stages,
    pub staged_turns: i64,
    pub tools: Vec<ToolSpeed>,
    /// The 10 slowest turns of live calls.
    pub slowest: Vec<SlowTurn>,
    pub staff_flags: Vec<FlagCount>,
    pub review_flags: Vec<(String, i64)>,
    pub versions: Vec<VersionStats>,
}

/// The value at fraction `q` of the sorted list, nearest rank; 0 for none.
pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

fn sorted(mut xs: Vec<f64>) -> Vec<f64> {
    xs.sort_by(f64::total_cmp);
    xs
}

fn outcome_index(o: &str) -> Option<usize> {
    OUTCOMES.iter().position(|x| *x == o)
}

/// The totals of some calls.
pub fn totals(rows: &[StatsRow], prices: &Prices) -> Totals {
    let mut t = Totals { calls: rows.len() as i64, ..Totals::default() };
    let mut waits = Vec::new();
    for r in rows {
        if let Some(i) = outcome_index(&r.outcome) {
            t.outcomes[i] += 1;
        }
        t.cost_usd += r.cost_usd;
        t.voice.add(&r.voice_credits);
        if !r.staff_flags.is_empty() || !r.review_flags.is_empty() {
            t.flagged_calls += 1;
        }
        t.staff_flags += r.staff_flags.len() as i64;
        t.review_flags += r.review_flags.len() as i64;
        if r.live() {
            waits.extend(r.turn_waits_ms.iter().copied());
        } else {
            t.replayed_calls += 1;
        }
    }
    let waits = sorted(waits);
    t.turns_answered = waits.len() as i64;
    t.turns_within = waits.iter().filter(|w| **w <= TURN_TARGET_MS).count() as i64;
    t.wait_p50_ms = percentile(&waits, 0.5);
    t.wait_p90_ms = percentile(&waits, 0.9);
    t.voice = t.voice.priced_at(prices);
    t
}

/// Every sum the five views show.
pub fn aggregate(rows: &[StatsRow], tools: &[ToolRow], prices: &Prices) -> Stats {
    let mut s = Stats { totals: totals(rows, prices), ..Stats::default() };

    let mut days: BTreeMap<String, Day> = BTreeMap::new();
    let mut reached: BTreeMap<String, i64> = BTreeMap::new();
    let mut ends: BTreeMap<String, i64> = BTreeMap::new();
    let mut models: BTreeMap<String, ModelCost> = BTreeMap::new();
    let mut flags: BTreeMap<(String, String), FlagCount> = BTreeMap::new();
    let mut reviews: BTreeMap<String, i64> = BTreeMap::new();
    let mut versions: BTreeMap<(String, String), (VersionStats, Vec<f64>)> = BTreeMap::new();
    let mut hist = vec![0i64; WAIT_BUCKETS];
    let mut stage_sum = Stages::default();
    let mut slow: Vec<SlowTurn> = Vec::new();

    for r in rows {
        let day = r.started.get(..10).unwrap_or("").to_string();
        let d = days.entry(day.clone()).or_insert_with(|| Day { day, ..Day::default() });
        d.calls += 1;
        if let Some(i) = outcome_index(&r.outcome) {
            d.outcomes[i] += 1;
        }
        d.cost_usd += r.cost_usd;
        d.voice.add(&r.voice_credits);
        d.staff_flags += r.staff_flags.len() as i64;
        d.review_flags += r.review_flags.len() as i64;

        let mut steps: Vec<&String> = r.reached.iter().collect();
        steps.sort();
        steps.dedup();
        for step in steps {
            *reached.entry(step.clone()).or_default() += 1;
        }
        if let Some(e) = &r.end_reason {
            *ends.entry(e.clone()).or_default() += 1;
        }
        let model = r.model.clone().unwrap_or_default();
        let m = models.entry(model.clone()).or_insert_with(|| ModelCost { model, ..ModelCost::default() });
        m.calls += 1;
        m.cost_usd += r.cost_usd;

        for (kind, source, unverified) in &r.staff_flags {
            let f = flags
                .entry((kind.clone(), source.clone()))
                .or_insert_with(|| FlagCount { kind: kind.clone(), source: source.clone(), ..FlagCount::default() });
            f.count += 1;
            f.unverified += i64::from(*unverified);
        }
        for reason in &r.review_flags {
            *reviews.entry(reason.clone()).or_default() += 1;
        }

        for (kind, label) in [("prompt", &r.prompt_version), ("system", &r.system_version), ("hospital", &r.hospital_version), ("model", &r.model)] {
            let Some(label) = label.clone().filter(|l| !l.is_empty()) else { continue };
            let (v, waits) = versions.entry((kind.to_string(), label.clone())).or_insert_with(|| {
                (VersionStats { kind: kind.to_string(), label, ..VersionStats::default() }, Vec::new())
            });
            v.calls += 1;
            v.booked += i64::from(r.outcome == "booked");
            v.cost_usd += r.cost_usd;
            if r.live() {
                waits.extend(r.turn_waits_ms.iter().copied());
            }
        }

        if r.live() {
            for (i, w) in r.turn_waits_ms.iter().enumerate() {
                let b = ((w / WAIT_BUCKET_MS).floor() as usize).min(WAIT_BUCKETS - 1);
                hist[b] += 1;
                slow.push(SlowTurn { call_id: r.id.clone(), started: r.started.clone(), turn: i as i32 + 1, wait_ms: *w });
            }
            for st in &r.turn_stages {
                stage_sum.ink += st.ink;
                stage_sum.claude += st.claude;
                stage_sum.tools += st.tools;
                stage_sum.ehr += st.ehr;
                stage_sum.voice += st.voice;
                stage_sum.other += st.other;
                s.staged_turns += 1;
            }
        }
    }

    s.days = days.into_values().map(|d| Day { voice: d.voice.clone().priced_at(prices), ..d }).collect();
    s.reached = reached.into_iter().collect();
    let mut ends: Vec<(String, i64)> = ends.into_iter().collect();
    ends.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ends.truncate(10);
    s.end_reasons = ends;
    let mut models: Vec<ModelCost> = models.into_values().collect();
    models.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd));
    s.models = models;
    // By all its dollars (Claude's and the priced voice's), then the voice's credits.
    let costs: Vec<(&StatsRow, VoiceCost)> = rows.iter().map(|r| (r, VoiceCost::of(&r.voice_credits, prices))).collect();
    let mut costly: Vec<&(&StatsRow, VoiceCost)> = costs.iter().collect();
    costly.sort_by(|(a, av), (b, bv)| (b.cost_usd + bv.usd).total_cmp(&(a.cost_usd + av.usd)).then(bv.all_credits().total_cmp(&av.all_credits())));
    s.costliest = costly
        .into_iter()
        .take(5)
        .map(|(r, v)| CostlyCall { id: r.id.clone(), started: r.started.clone(), cost_usd: r.cost_usd, voice: v.clone(), outcome: r.outcome.clone() })
        .collect();
    s.wait_hist = hist;
    if s.staged_turns > 0 {
        let n = s.staged_turns as f64;
        s.stages = Stages {
            ink: stage_sum.ink / n,
            claude: stage_sum.claude / n,
            tools: stage_sum.tools / n,
            ehr: stage_sum.ehr / n,
            voice: stage_sum.voice / n,
            other: stage_sum.other / n,
        };
    }

    let mut by_tool: BTreeMap<String, (ToolSpeed, Vec<f64>)> = BTreeMap::new();
    for t in tools {
        let (ts, ms) = by_tool.entry(t.tool.clone()).or_insert_with(|| (ToolSpeed { tool: t.tool.clone(), ..ToolSpeed::default() }, Vec::new()));
        ts.calls += 1;
        ts.failed += i64::from(t.failed);
        if let Some(m) = t.ms {
            ms.push(m);
        }
    }
    let mut tool_list: Vec<ToolSpeed> = by_tool
        .into_values()
        .map(|(mut ts, ms)| {
            let ms = sorted(ms);
            ts.p50_ms = percentile(&ms, 0.5);
            ts.p90_ms = percentile(&ms, 0.9);
            ts
        })
        .collect();
    tool_list.sort_by(|a, b| b.p90_ms.total_cmp(&a.p90_ms));
    s.tools = tool_list;

    slow.sort_by(|a, b| b.wait_ms.total_cmp(&a.wait_ms));
    slow.truncate(10);
    s.slowest = slow;

    let mut flag_list: Vec<FlagCount> = flags.into_values().collect();
    flag_list.sort_by(|a, b| b.count.cmp(&a.count).then(a.kind.cmp(&b.kind)));
    s.staff_flags = flag_list;
    let mut review_list: Vec<(String, i64)> = reviews.into_iter().collect();
    review_list.sort_by_key(|r| std::cmp::Reverse(r.1));
    s.review_flags = review_list;

    s.versions = versions
        .into_values()
        .map(|(mut v, waits)| {
            let waits = sorted(waits);
            v.turns_answered = waits.len() as i64;
            v.turns_within = waits.iter().filter(|w| **w <= TURN_TARGET_MS).count() as i64;
            v.wait_p50_ms = percentile(&waits, 0.5);
            v.wait_p90_ms = percentile(&waits, 0.9);
            v
        })
        .collect();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, day: &str, outcome: &str, waits: &[f64]) -> StatsRow {
        StatsRow {
            id: id.into(),
            started: format!("{day}T10:00:00"),
            model: Some("claude-sonnet-5-5".into()),
            prompt_version: Some("11-21 | 3".into()),
            outcome: outcome.into(),
            reached: vec!["greet".into(), "verify".into()],
            cost_usd: 0.02,
            turn_waits_ms: waits.to_vec(),
            ..StatsRow::default()
        }
    }

    #[test]
    fn the_percentile_is_the_nearest_rank() {
        let xs = [100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0];
        assert_eq!(percentile(&xs, 0.5), 500.0);
        assert_eq!(percentile(&xs, 0.9), 900.0);
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    #[test]
    fn the_sums_count_outcomes_days_waits_and_leave_replays_out() {
        let mut replayed = row("kept_a", "2026-11-21", "booked", &[5.0, 6.0]);
        replayed.requests_replayed = 3;
        let mut flagged = row("test_b", "2026-11-22", "handed_off", &[900.0]);
        flagged.staff_flags = vec![("referral_check".into(), "code".into(), 0)];
        flagged.review_flags = vec!["token_cap".into()];
        flagged.voice_credits = BTreeMap::from([("cartesia".to_string(), 120.0)]);
        let rows = vec![row("test_a", "2026-11-22", "booked", &[400.0, 700.0]), replayed, flagged];
        let tools = vec![
            ToolRow { tool: "find_slots".into(), ms: Some(300.0), failed: false },
            ToolRow { tool: "find_slots".into(), ms: Some(500.0), failed: true },
        ];
        let s = aggregate(&rows, &tools, &Prices::from([("cartesia".to_string(), 0.001)]));
        assert_eq!(s.totals.calls, 3);
        assert_eq!(s.totals.outcomes[0], 2, "booked");
        assert_eq!(s.totals.outcomes[1], 1, "handed off");
        assert_eq!((s.totals.turns_answered, s.totals.turns_within), (3, 2), "the replayed call's turns are left out");
        assert_eq!(s.totals.replayed_calls, 1);
        assert_eq!((s.totals.flagged_calls, s.totals.staff_flags, s.totals.review_flags), (1, 1, 1));
        assert_eq!(s.totals.voice.credits["cartesia"], 120.0);
        assert!(s.totals.voice.priced && (s.totals.voice.usd - 0.12).abs() < 1e-9);
        assert_eq!(s.days.iter().map(|d| (d.day.as_str(), d.calls)).collect::<Vec<_>>(), [("2026-11-21", 1), ("2026-11-22", 2)]);
        assert_eq!(s.reached, [("greet".to_string(), 3), ("verify".to_string(), 3)]);
        assert_eq!(s.wait_hist[2], 1, "400 ms");
        assert_eq!(s.wait_hist[4], 1, "900 ms");
        assert_eq!(s.slowest[0].wait_ms, 900.0);
        assert_eq!(s.tools[0].calls, 2);
        assert_eq!(s.tools[0].failed, 1);
        assert_eq!(s.staff_flags[0].kind, "referral_check");
        let words = s.versions.iter().find(|v| v.kind == "prompt").unwrap();
        assert_eq!((words.calls, words.booked, words.turns_answered), (3, 2, 3));
    }
}
