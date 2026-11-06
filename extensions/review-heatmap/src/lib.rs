//! Manatan SRS Review Heatmap: a calendar of past review activity and upcoming
//! due counts, with streak statistics and per-user configuration. Uses the
//! generic `collection.review-log` / `due-forecast` capabilities.
//!
//! The Rust side computes the activity report (day buckets, streaks, the
//! dynamic legend) and emits a self-contained HTML fragment; the embedded
//! JS (src/web/heatmap.js) renders the interactive calendar client-side and
//! talks to the app through the `srsExtensions` bridge (open the browser on
//! a day's cards, open the options dialog, report its height).

use std::collections::BTreeMap;

use manatan_flashcards_sdk::{collection, host, store, types::ReviewKind, ui, Extension};
use serde_json::{json, Value};

const DAY: i64 = 86_400;

/// Forecast horizon requested when the user sets no limit. A ten-year bound
/// keeps the payload small and protects against corrupt due dates.
const MAX_FORECAST_DAYS: u32 = 3_650;

/// The dynamic legend: thresholds at these multiples of the (floored) daily
/// average split past days into the ten theme buckets q11..q20, and their
/// negations split future days into the grey buckets q1..q10.
const DYNAMIC_LEGEND_FACTORS: [f64; 9] = [0.125, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 4.0];

const THEMES: [&str; 5] = ["lime", "olive", "ice", "magenta", "flame"];
const MODES: [&str; 2] = ["year", "months"];

/// Streak value → text color class ("first threshold ≥ value wins;
/// beyond the last threshold the last class sticks").
const STREAK_LEVELS: [(f64, &str); 6] = [
    (0.0, "rh-col0"),
    (14.0, "rh-col12"),
    (30.0, "rh-col14"),
    (90.0, "rh-col16"),
    (180.0, "rh-col19"),
    (365.0, "rh-col20"),
];

/// "Days learned" percentage → text color class.
const PCT_LEVELS: [(f64, &str); 10] = [
    (0.0, "rh-col0"),
    (25.0, "rh-col11"),
    (50.0, "rh-col12"),
    (60.0, "rh-col13"),
    (70.0, "rh-col14"),
    (80.0, "rh-col15"),
    (85.0, "rh-col16"),
    (90.0, "rh-col17"),
    (95.0, "rh-col18"),
    (99.0, "rh-col19"),
];

struct Config {
    colors: String,
    mode: String,
    /// Days of history shown; 0 = no limit.
    limhist: u32,
    /// Days of forecast shown; 0 = no limit.
    limfcst: u32,
    /// Ignore review data before this unix-ms timestamp; 0 = unset.
    limdate_ms: i64,
    display_deckbrowser: bool,
    display_stats: bool,
    /// Show the streak stats row even when the heatmap is hidden.
    statsvis: bool,
}

impl Config {
    fn load() -> Config {
        Config::from_value(serde_json::from_str(&store::config()).unwrap_or(Value::Null))
    }

    /// Parse (and sanitize) a config value; unknown fields and out-of-range
    /// values fall back to the defaults.
    fn from_value(parsed: Value) -> Config {
        let str_or = |value: Option<&Value>, allowed: &[&str], default: &str| {
            value
                .and_then(Value::as_str)
                .filter(|v| allowed.contains(v))
                .unwrap_or(default)
                .to_string()
        };
        let display = parsed.get("display");
        Config {
            colors: str_or(parsed.get("colors"), &THEMES, "lime"),
            mode: str_or(parsed.get("mode"), &MODES, "year"),
            limhist: parsed
                .get("limhist")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(100_000) as u32,
            limfcst: parsed
                .get("limfcst")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(100_000) as u32,
            limdate_ms: parsed
                .get("limdate")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .max(0),
            display_deckbrowser: display
                .and_then(|d| d.get("deckbrowser"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            display_stats: display
                .and_then(|d| d.get("stats"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            statsvis: parsed
                .get("statsvis")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    fn as_json(&self) -> Value {
        json!({
            "colors": self.colors,
            "mode": self.mode,
            "limhist": self.limhist,
            "limfcst": self.limfcst,
            "limdate": self.limdate_ms,
            "display": {
                "deckbrowser": self.display_deckbrowser,
                "stats": self.display_stats,
            },
            "statsvis": self.statsvis,
        })
    }
}

struct Stats {
    streak_max: i64,
    streak_cur: i64,
    daily_avg: i64,
    pct_days_active: i64,
}

/// Streaks, "days learned" and the daily average. Streaks are runs of
/// consecutive active days; the
/// average is over active days only; the percentage denominator is the span
/// since the first recorded review.
fn compute_stats(history: &BTreeMap<i64, u32>, today: i64) -> Stats {
    if history.is_empty() {
        return Stats {
            streak_max: 0,
            streak_cur: 0,
            daily_avg: 0,
            pct_days_active: 0,
        };
    }
    let first_day = *history.keys().next().expect("history is non-empty");
    let last_day = *history.keys().next_back().expect("history is non-empty");
    let total: i64 = history.values().map(|&c| c as i64).sum();

    let mut streak_max: i64 = 0;
    let mut current: i64 = 0;
    let mut previous: Option<i64> = None;
    for &day in history.keys() {
        current = match previous {
            Some(p) if day == p + 1 => current + 1,
            _ => 1,
        };
        streak_max = streak_max.max(current);
        previous = Some(day);
    }
    let streak_cur = if last_day == today || last_day == today - 1 {
        current
    } else {
        0
    };

    let days_learned = history.len() as i64;
    let daily_avg = (total as f64 / days_learned.max(1) as f64).round() as i64;
    let days_total = (today - first_day) + 1;
    let pct_days_active = if days_total <= 1 {
        100
    } else {
        (days_learned as f64 / days_total as f64 * 100.0).round() as i64
    };

    Stats {
        streak_max,
        streak_cur,
        daily_avg,
        pct_days_active,
    }
}

/// The color class for a stat value: the first threshold ≥ value wins, and
/// values beyond every threshold keep the last class.
fn level_class(value: i64, levels: &[(f64, &'static str)]) -> &'static str {
    let mut selected = levels.last().expect("levels non-empty").1;
    for &(threshold, class) in levels {
        if value as f64 <= threshold {
            selected = class;
            break;
        }
    }
    selected
}

/// "1 day" / "2 days" — pluralize only for |count| > 1.
fn maybe_pluralize(count: i64, term: &str) -> String {
    let suffix = if count.abs() > 1 { "s" } else { "" };
    format!("{count} {term}{suffix}")
}

#[derive(Default)]
struct ReviewHeatmap;

impl ReviewHeatmap {
    fn render_dashboard(&self, context: &str) -> String {
        let context: Value = serde_json::from_str(context).unwrap_or(Value::Null);
        let view = match context.get("surface").and_then(Value::as_str) {
            Some("stats") => "stats",
            _ => "deckbrowser",
        };
        let config = Config::load();

        let display = match view {
            "stats" => config.display_stats,
            _ => config.display_deckbrowser,
        };
        let show_heatmap = display;
        let show_stats = display || config.statsvis;
        if !show_heatmap && !show_stats {
            // Nothing to show for this view: emit only the height report so
            // the host panel can collapse instead of reserving blank space.
            return format!(
                "<script>{}</script>",
                r#"(function(){var c=window.srsExtensions;if(c&&c.postMessage){c.postMessage(JSON.stringify({type:"resize",height:0}));}})();"#
            );
        }

        let offset = host::utc_offset_seconds() as i64;
        let now_ms = host::now_millis();
        let today = (now_ms / 1000 + offset).div_euclid(DAY);

        // History window: the tighter of "last N days" and "nothing before
        // this date" (both optional).
        let mut start_day_limit: Option<i64> = None;
        if config.limhist > 0 {
            start_day_limit = Some(today - config.limhist as i64);
        }
        if config.limdate_ms > 0 {
            let limdate_day = (config.limdate_ms / 1000 + offset).div_euclid(DAY);
            start_day_limit = Some(start_day_limit.map_or(limdate_day, |d| d.max(limdate_day)));
        }
        let since_ms = start_day_limit.map_or(0, |day| (day * DAY - offset) * 1000);

        // Past activity, bucketed by local day. Manual reschedules do not
        // count as study activity.
        let mut history: BTreeMap<i64, u32> = BTreeMap::new();
        for entry in collection::review_log(since_ms.max(0), now_ms) {
            if matches!(entry.kind, ReviewKind::Manual | ReviewKind::Rescheduled) {
                continue;
            }
            let day = (entry.timestamp / 1000 + offset).div_euclid(DAY);
            if let Some(limit) = start_day_limit {
                if day < limit {
                    continue;
                }
            }
            *history.entry(day).or_default() += 1;
        }

        let css = include_str!("web/heatmap.css");
        if history.is_empty() {
            // Without review history, show an empty state with an options link.
            return format!(
                r#"<div id="review-heatmap-root" class="rh-container rh-theme-{colors} rh-mode-{mode} rh-view-{view}">
<style>{css}</style>
No activity data to show (<span class="linkspan" id="rh-nodata-opts">options</span>).
</div>
<script>
window.RH_DATA = {data};
{js}
</script>
"#,
                colors = config.colors,
                mode = config.mode,
                data = json_for_script(&json!({
                    "days": {}, "today": today, "offset": offset,
                    "minDay": today, "maxDay": today,
                    "legend": [], "mode": config.mode, "theme": config.colors,
                    "view": view, "config": config.as_json(),
                })),
                js = include_str!("web/heatmap.js"),
            );
        }

        // Future due counts as negative values, so the same legend colors
        // them with the grey ramp. Offset 0 = today; when today was already
        // studied the positive review count wins.
        let forecast_days = if config.limfcst > 0 {
            config.limfcst
        } else {
            MAX_FORECAST_DAYS
        };
        let mut activity: BTreeMap<i64, i64> = BTreeMap::new();
        let mut max_day = today;
        for (i, count) in collection::due_forecast(forecast_days).iter().enumerate() {
            if *count > 0 {
                let day = today + i as i64;
                activity.insert(day, -(*count as i64));
                max_day = day;
            }
        }
        for (&day, &count) in &history {
            activity.insert(day, count as i64);
        }
        // History is inserted second so today's review count overrides the
        // remaining-due count.

        let stats = compute_stats(&history, today);
        let min_day = *history.keys().next().expect("non-empty");

        let avg_for_legend = (stats.daily_avg.max(20)) as f64;
        let legend: Vec<f64> = DYNAMIC_LEGEND_FACTORS
            .iter()
            .map(|f| f * avg_for_legend)
            .collect();
        let heatmap_legend: Vec<f64> = legend
            .iter()
            .rev()
            .map(|v| -v)
            .chain(std::iter::once(0.0))
            .chain(legend.iter().copied())
            .collect();
        // Daily-average colors use the positive half with a zero floor.
        let avg_levels: Vec<(f64, &str)> = std::iter::once(0.0)
            .chain(legend.iter().copied())
            .zip([
                "rh-col0", "rh-col11", "rh-col12", "rh-col13", "rh-col14",
                "rh-col15", "rh-col16", "rh-col17", "rh-col18", "rh-col19",
            ])
            .collect();

        let days: BTreeMap<String, i64> = activity
            .iter()
            .map(|(day, count)| (day.to_string(), *count))
            .collect();
        let data = json_for_script(&json!({
            "days": days,
            "today": today,
            "offset": offset,
            "minDay": min_day,
            "maxDay": max_day.max(today),
            "legend": heatmap_legend,
            "mode": config.mode,
            "theme": config.colors,
            "view": view,
            "config": config.as_json(),
        }));

        let heatmap_block = if show_heatmap {
            r##"<div class="heatmap">
    <div class="heatmap-controls">
        <div class="heatmap-navigation" role="group" aria-label="Heatmap period">
        <button type="button" title="Previous period" aria-label="Previous period" id="rh-btn-prev" class="hm-btn"><svg aria-hidden="true" viewBox="0 0 32 32"><path d="M22 2 L8 16 L22 30 L26 26 L16 16 L26 6 Z"/></svg></button>
        <button type="button" title="Return to today" aria-label="Return to today" id="rh-btn-home" class="hm-btn rh-home"><span id="rh-period-label" aria-live="polite"></span><span class="rh-home-hint">Today</span></button>
        <button type="button" title="Next period" aria-label="Next period" id="rh-btn-next" class="hm-btn"><svg aria-hidden="true" viewBox="0 0 32 32"><path d="M10 2 L24 16 L10 30 L6 26 L16 16 L6 6 Z"/></svg></button>
        </div>
        <button type="button" title="Options" aria-label="Heatmap options" id="rh-btn-opts" class="hm-btn opts-btn"><svg aria-hidden="true" viewBox="0 0 32 32"><path d="M16 11 A5 5 0 1 0 16 21 A5 5 0 1 0 16 11 M14 2 L18 2 L19 6.3 L22.2 7.6 L26 5.4 L28.6 8 L26.4 11.8 L27.7 15 L32 16 L27.7 17 L26.4 20.2 L28.6 24 L26 26.6 L22.2 24.4 L19 25.7 L18 30 L14 30 L13 25.7 L9.8 24.4 L6 26.6 L3.4 24 L5.6 20.2 L4.3 17 L0 16 L4.3 15 L5.6 11.8 L3.4 8 L6 5.4 L9.8 7.6 L13 6.3 Z" fill-rule="evenodd"/></svg></button>
    </div>
    <div id="rh-cal"></div>

</div>
"##.to_string()
        } else {
            String::new()
        };

        let stats_block = if show_stats {
            format!(
                r#"<dl class="streak">
    <div class="rh-stat"><dt class="streak-info">Daily average:</dt><dd><span title="Average reviews on active days"
        class="sstats {class_daily_avg}">{text_daily_avg}</span></dd></div>
    <div class="rh-stat"><dt class="streak-info">Days learned:</dt><dd><span title="Percentage of days with review activity over entire review history"
        class="sstats {class_pct}">{pct}%</span></dd></div>
    <div class="rh-stat"><dt class="streak-info">Longest streak:</dt><dd><span title="Longest continuous streak of review activity. All types of repetitions included."
        class="sstats {class_streak_max}">{text_streak_max}</span></dd></div>
    <div class="rh-stat"><dt class="streak-info">Current streak:</dt><dd><span title="Current card review activity streak. All types of repetitions included."
        class="sstats {class_streak_cur}">{text_streak_cur}</span></dd></div>
</dl>
"#,
                class_daily_avg = level_class(stats.daily_avg, &avg_levels),
                text_daily_avg = maybe_pluralize(stats.daily_avg, "card"),
                class_pct = level_class(stats.pct_days_active, &PCT_LEVELS),
                pct = stats.pct_days_active,
                class_streak_max = level_class(stats.streak_max, &STREAK_LEVELS),
                text_streak_max = maybe_pluralize(stats.streak_max, "day"),
                class_streak_cur = level_class(stats.streak_cur, &STREAK_LEVELS),
                text_streak_cur = maybe_pluralize(stats.streak_cur, "day"),
            )
        } else {
            String::new()
        };

        let mut container_classes = format!(
            "rh-container rh-theme-{} rh-mode-{} rh-view-{}",
            config.colors, config.mode, view
        );
        if !show_heatmap {
            container_classes.push_str(" rh-disable-heatmap");
        }
        if !show_stats {
            container_classes.push_str(" rh-disable-stats");
        }

        format!(
            r#"<div id="review-heatmap-root" class="{container_classes}">
<style>{css}</style>
{heatmap_block}{stats_block}</div>
<script>
window.RH_DATA = {data};
{js}
</script>
"#,
            js = include_str!("web/heatmap.js"),
        )
    }
}

/// Serialize for embedding inside a `<script>` element: `<` is escaped so
/// user data can never close the script block.
fn json_for_script(value: &Value) -> String {
    value.to_string().replace('<', "\\u003c")
}

/// The options dialog the extension opens via `ui.open-dialog`.
/// The host shows it in a dialog webview with
/// the standard extension bridge; saving posts `saveOptions` back into
/// `on-message`.
fn options_dialog_html(config: &Config) -> String {
    let css = r#"
.rh-opts { padding: 14px 16px 10px; }
.rh-opts-row { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
.rh-opts-row > label:first-child { flex: 1; }
.rh-opts select, .rh-opts input[type="number"], .rh-opts input[type="date"] {
    font: inherit; padding: 2px 4px; }
.rh-opts input[type="number"] { width: 70px; }
.rh-opts .rh-opts-hint { color: #888; font-size: 11px; }
.rh-opts fieldset { border: 1px solid #bbb; border-radius: 3px;
    margin: 0 0 10px; padding: 4px 10px 8px; }
.rh-opts fieldset label { display: block; margin-top: 4px; }
.rh-opts .rh-opts-check { display: block; margin: 0 0 10px; }
.rh-opts .rh-opts-buttons { display: flex; gap: 8px; margin-top: 14px; }
.rh-opts .rh-opts-buttons .rh-spacer { flex: 1; }
.rh-opts button { font: inherit; padding: 4px 14px; }
.night_mode .rh-opts fieldset, .nightMode .rh-opts fieldset { border-color: #555; }
"#;
    format!(
        r#"<div class="rh-opts">
<style>{css}</style>
<div class="rh-opts-row">
    <label for="rh-opt-colors" title="Sets the general color scheme of the heatmap">Color scheme</label>
    <select id="rh-opt-colors">
        <option value="lime">Lime</option>
        <option value="olive">Olive</option>
        <option value="ice">Ice</option>
        <option value="magenta">Magenta</option>
        <option value="flame">Flame</option>
    </select>
</div>
<div class="rh-opts-row">
    <label for="rh-opt-mode" title="Phones show a shorter fragment of the same heatmap; this selects the desktop overview">Desktop overview</label>
    <select id="rh-opt-mode">
        <option value="year">Yearly Overview</option>
        <option value="months">Continuous Timeline</option>
    </select>
</div>
<div class="rh-opts-row">
    <label for="rh-opt-limhist">History limit</label>
    <input type="number" id="rh-opt-limhist" min="0" max="100000">
    <span class="rh-opts-hint">days (0 = no limit)</span>
</div>
<div class="rh-opts-row">
    <label for="rh-opt-limfcst">Forecast limit</label>
    <input type="number" id="rh-opt-limfcst" min="0" max="100000">
    <span class="rh-opts-hint">days (0 = no limit)</span>
</div>
<div class="rh-opts-row">
    <label for="rh-opt-limdate" title="Ignore review activity before this date">Ignore data before</label>
    <input type="date" id="rh-opt-limdate">
</div>
<fieldset>
    <legend>Show the heatmap on</legend>
    <label><input type="checkbox" id="rh-opt-deckbrowser"> Main screen</label>
    <label><input type="checkbox" id="rh-opt-stats"> Stats screen</label>
</fieldset>
<label class="rh-opts-check"><input type="checkbox" id="rh-opt-statsvis"> Show streak stats even when heatmap disabled</label>
<div class="rh-opts-buttons">
    <button id="rh-opt-defaults">Restore Defaults</button>
    <span class="rh-spacer"></span>
    <button id="rh-opt-cancel">Cancel</button>
    <button id="rh-opt-ok">OK</button>
</div>
</div>
<script>
window.RH_OPTS = {config_json};
{options_js}
</script>
"#,
        config_json = json_for_script(&config.as_json()),
        options_js = include_str!("web/options.js"),
    )
}

impl Extension for ReviewHeatmap {
    fn render(&mut self, surface: String, context: String) -> String {
        // The host renders both the deck screen and the stats screen through
        // the "dashboard" surface, with the concrete view in the context.
        if surface != "dashboard" {
            return String::new();
        }
        self.render_dashboard(&context)
    }

    /// The bridge behind the surface JS and the options dialog: the gear
    /// button posts `openOptions`, the dialog's OK posts `saveOptions`.
    fn on_message(&mut self, _source: String, payload: String) -> String {
        let message: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        match message.get("type").and_then(Value::as_str) {
            Some("openOptions") => {
                ui::open_dialog(
                    "Review Heatmap Options",
                    &options_dialog_html(&Config::load()),
                );
                "ok".to_string()
            }
            Some("saveOptions") => {
                // Round-trip through Config so only known fields with sane
                // values are persisted.
                let config =
                    Config::from_value(message.get("config").cloned().unwrap_or(Value::Null));
                store::set_config(&config.as_json().to_string());
                ui::refresh_surface("dashboard");
                "ok".to_string()
            }
            _ => String::new(),
        }
    }
}

manatan_flashcards_sdk::export_extension!(ReviewHeatmap);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_collection_has_zero_statistics() {
        let stats = compute_stats(&BTreeMap::new(), 20_000);
        assert_eq!(stats.daily_avg, 0);
        assert_eq!(stats.pct_days_active, 0);
        assert_eq!(stats.streak_cur, 0);
        assert_eq!(stats.streak_max, 0);
    }
}
