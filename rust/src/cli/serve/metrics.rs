//! Prometheus text exposition derived from the cached dashboard snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;

use chrono::{DateTime, Utc};

use super::dashboard;
use super::dashboard::snapshot::{AccountPayload, PacePayload, SnapshotPayload, WindowPayload};

const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

const METRIC_DEFINITIONS: &[(&str, &str)] = &[
    (
        "codexbar_up",
        "Whether CodexBar produced a dashboard snapshot for this scrape.",
    ),
    (
        "codexbar_snapshot_schema_version",
        "Dashboard snapshot schema version.",
    ),
    (
        "codexbar_snapshot_generated_timestamp_seconds",
        "Unix timestamp when the dashboard snapshot was generated.",
    ),
    (
        "codexbar_snapshot_age_seconds",
        "Age of the dashboard snapshot.",
    ),
    (
        "codexbar_snapshot_stale_after_seconds",
        "Age after which the dashboard snapshot is stale.",
    ),
    (
        "codexbar_snapshot_stale",
        "Whether the dashboard snapshot is older than its stale threshold.",
    ),
    (
        "codexbar_refresh_interval_seconds",
        "Configured dashboard refresh interval.",
    ),
    ("codexbar_build_info", "CodexBar build information."),
    (
        "codexbar_provider_enabled",
        "Whether the provider is enabled.",
    ),
    (
        "codexbar_provider_up",
        "Whether the provider usage fetch succeeded.",
    ),
    (
        "codexbar_provider_error_code",
        "Provider error code when the usage fetch failed.",
    ),
    (
        "codexbar_provider_updated_timestamp_seconds",
        "Unix timestamp of the provider data update.",
    ),
    (
        "codexbar_provider_data_age_seconds",
        "Age of the provider data.",
    ),
    (
        "codexbar_provider_status",
        "Provider service status: 0 operational, 1 degraded, 2 partial outage, 3 major outage, 4 unknown.",
    ),
    (
        "codexbar_provider_status_updated_timestamp_seconds",
        "Unix timestamp of the provider service status update.",
    ),
    (
        "codexbar_quota_used_percent",
        "Used quota percentage for a provider window.",
    ),
    (
        "codexbar_quota_remaining_percent",
        "Remaining quota percentage for a provider window.",
    ),
    (
        "codexbar_quota_reset_timestamp_seconds",
        "Unix timestamp when a provider quota window resets.",
    ),
    (
        "codexbar_quota_idle",
        "Whether a provider quota window is idle.",
    ),
    (
        "codexbar_reset_credits_available",
        "Available Codex rate-limit reset credits; -1 means unavailable or unsupported.",
    ),
    (
        "codexbar_reset_credits_next_expiry_timestamp_seconds",
        "Unix timestamp when the next available Codex rate-limit reset credit expires.",
    ),
    ("codexbar_credits_remaining", "Remaining provider credits."),
    (
        "codexbar_cost_today_usd",
        "Estimated provider cost today in US dollars.",
    ),
    (
        "codexbar_cost_last_30_days_usd",
        "Estimated provider cost over the last 30 days in US dollars.",
    ),
    (
        "codexbar_provider_accounts_up",
        "Whether provider account data was collected successfully.",
    ),
    (
        "codexbar_provider_accounts_total",
        "Number of provider accounts in the snapshot.",
    ),
    (
        "codexbar_account_active",
        "Whether a provider account is active.",
    ),
    (
        "codexbar_account_up",
        "Whether a provider account usage fetch succeeded.",
    ),
    (
        "codexbar_account_updated_timestamp_seconds",
        "Unix timestamp of the provider account data update.",
    ),
    (
        "codexbar_account_quota_used_percent",
        "Used quota percentage for a provider account window.",
    ),
    (
        "codexbar_account_quota_remaining_percent",
        "Remaining quota percentage for a provider account window.",
    ),
    (
        "codexbar_account_quota_reset_timestamp_seconds",
        "Unix timestamp when a provider account quota window resets.",
    ),
    (
        "codexbar_account_quota_idle",
        "Whether a provider account quota window is idle.",
    ),
    (
        "codexbar_account_pace_stage_info",
        "Current provider account pace stage.",
    ),
    (
        "codexbar_account_pace_delta_percent",
        "Difference between actual and expected provider account usage.",
    ),
    (
        "codexbar_account_pace_expected_used_percent",
        "Expected provider account usage percentage at the current time.",
    ),
    (
        "codexbar_account_pace_will_last_to_reset",
        "Whether provider account quota is expected to last until reset.",
    ),
    (
        "codexbar_account_pace_eta_seconds",
        "Estimated seconds until provider account quota is exhausted.",
    ),
    (
        "codexbar_account_pace_run_out_probability",
        "Estimated probability that provider account quota runs out before reset.",
    ),
];

/// Serve a scrape from the same cached, single-flight snapshot as the dashboard.
pub(super) async fn response(state: &dashboard::DashboardState) -> String {
    let body = match state.coordinator.get().await {
        Ok(snapshot) => render(snapshot.as_ref()),
        Err(_) => render_unavailable(),
    };
    super::http_response(200, CONTENT_TYPE, body, &[("Cache-Control", "no-store")])
}

fn render(snapshot: &SnapshotPayload) -> String {
    render_at(snapshot, Utc::now())
}

fn render_at(snapshot: &SnapshotPayload, now: DateTime<Utc>) -> String {
    let mut writer = MetricsWriter::new();
    let snapshot_age = age_seconds(now, snapshot.generated_at);
    writer.sample("codexbar_up", &[], 1);
    writer.sample(
        "codexbar_snapshot_schema_version",
        &[],
        snapshot.schema_version,
    );
    writer.sample(
        "codexbar_snapshot_generated_timestamp_seconds",
        &[],
        snapshot.generated_at.timestamp(),
    );
    writer.sample("codexbar_snapshot_age_seconds", &[], snapshot_age);
    writer.sample(
        "codexbar_snapshot_stale_after_seconds",
        &[],
        snapshot.stale_after_seconds,
    );
    writer.sample(
        "codexbar_snapshot_stale",
        &[],
        bool_value(snapshot_age > i64::from(snapshot.stale_after_seconds)),
    );
    writer.sample(
        "codexbar_refresh_interval_seconds",
        &[],
        snapshot.host.refresh_interval_seconds,
    );
    if let Some(version) = snapshot
        .host
        .codex_bar_version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty())
    {
        let schema_version = snapshot.schema_version.to_string();
        writer.sample(
            "codexbar_build_info",
            &[
                ("version", version),
                ("schema_version", schema_version.as_str()),
            ],
            1,
        );
    }

    for provider in &snapshot.providers {
        let provider_labels = [("provider", provider.id.as_str())];
        writer.sample(
            "codexbar_provider_enabled",
            &provider_labels,
            bool_value(provider.enabled),
        );
        writer.sample(
            "codexbar_provider_up",
            &provider_labels,
            bool_value(provider.error.is_none()),
        );
        if let Some(error) = &provider.error {
            writer.sample("codexbar_provider_error_code", &provider_labels, error.code);
        }
        if provider.error.is_none()
            && let Some(updated_at) = provider.updated_at
        {
            writer.sample(
                "codexbar_provider_updated_timestamp_seconds",
                &provider_labels,
                updated_at.timestamp(),
            );
            writer.sample(
                "codexbar_provider_data_age_seconds",
                &provider_labels,
                age_seconds(now, updated_at),
            );
        }
        if let Some(status) = &provider.status {
            writer.sample(
                "codexbar_provider_status",
                &provider_labels,
                provider_status_value(&status.level),
            );
            if let Some(updated_at) = status.updated_at {
                writer.sample(
                    "codexbar_provider_status_updated_timestamp_seconds",
                    &provider_labels,
                    updated_at.timestamp(),
                );
            }
        }

        for window in &provider.windows {
            render_provider_window(&mut writer, &provider.id, window);
        }
        if provider.id == "codex" {
            writer.sample(
                "codexbar_reset_credits_available",
                &provider_labels,
                provider
                    .reset_credits_available
                    .map(i64::from)
                    .unwrap_or(-1),
            );
            if provider
                .reset_credits_available
                .is_some_and(|available| available > 0)
                && let Some(expires_at) = provider
                    .windows
                    .iter()
                    .find(|window| window.kind == "reset-credits")
                    .and_then(|window| window.reset_at)
            {
                writer.sample(
                    "codexbar_reset_credits_next_expiry_timestamp_seconds",
                    &provider_labels,
                    expires_at.timestamp(),
                );
            }
        }
        if let Some(credits) = &provider.credits {
            writer.sample_f64(
                "codexbar_credits_remaining",
                &[
                    ("provider", provider.id.as_str()),
                    ("unit", credits.unit.as_str()),
                ],
                credits.remaining,
            );
        }
        if let Some(cost) = &provider.cost {
            if let Some(value) = cost.today_usd {
                writer.sample_f64("codexbar_cost_today_usd", &provider_labels, value);
            }
            if let Some(value) = cost.last_30_days_usd {
                writer.sample_f64("codexbar_cost_last_30_days_usd", &provider_labels, value);
            }
        }

        if provider.accounts.is_some() || provider.accounts_error.is_some() {
            writer.sample(
                "codexbar_provider_accounts_up",
                &provider_labels,
                bool_value(provider.accounts_error.is_none()),
            );
        }
        if let Some(accounts) = &provider.accounts {
            writer.sample(
                "codexbar_provider_accounts_total",
                &provider_labels,
                accounts.len(),
            );
            for account in accounts {
                render_account(&mut writer, &provider.id, account);
            }
        }
    }

    writer.finish()
}

fn render_unavailable() -> String {
    let mut writer = MetricsWriter::new();
    writer.sample("codexbar_up", &[], 0);
    writer.sample(
        "codexbar_reset_credits_available",
        &[("provider", "codex")],
        -1,
    );
    writer.finish()
}

fn render_provider_window(writer: &mut MetricsWriter, provider: &str, window: &WindowPayload) {
    if !window.usage_known {
        return;
    }
    let labels = [("provider", provider), ("window", window.kind.as_str())];
    writer.sample_f64("codexbar_quota_used_percent", &labels, window.used_percent);
    writer.sample_f64(
        "codexbar_quota_remaining_percent",
        &labels,
        window.remaining_percent,
    );
    if let Some(reset_at) = window.reset_at {
        writer.sample(
            "codexbar_quota_reset_timestamp_seconds",
            &labels,
            reset_at.timestamp(),
        );
    }
    writer.sample("codexbar_quota_idle", &labels, bool_value(window.idle));
}

fn render_account(writer: &mut MetricsWriter, provider: &str, account: &AccountPayload) {
    let labels = [("provider", provider), ("account", account.id.as_str())];
    writer.sample(
        "codexbar_account_active",
        &labels,
        bool_value(account.active),
    );
    writer.sample(
        "codexbar_account_up",
        &labels,
        bool_value(account.error.is_none()),
    );
    if account.error.is_none()
        && let Some(updated_at) = account.updated_at
    {
        writer.sample(
            "codexbar_account_updated_timestamp_seconds",
            &labels,
            updated_at.timestamp(),
        );
    }
    for window in &account.windows {
        render_account_window(writer, provider, &account.id, window);
    }
    if let Some(pace) = &account.pace {
        if let Some(primary) = &pace.primary {
            render_account_pace(writer, provider, &account.id, "session", primary);
        }
        if let Some(secondary) = &pace.secondary {
            render_account_pace(writer, provider, &account.id, "weekly", secondary);
        }
        if let Some(tertiary) = &pace.tertiary {
            render_account_pace(writer, provider, &account.id, "tertiary", tertiary);
        }
    }
}

fn render_account_window(
    writer: &mut MetricsWriter,
    provider: &str,
    account: &str,
    window: &WindowPayload,
) {
    if !window.usage_known {
        return;
    }
    let labels = [
        ("provider", provider),
        ("account", account),
        ("window", window.kind.as_str()),
    ];
    writer.sample_f64(
        "codexbar_account_quota_used_percent",
        &labels,
        window.used_percent,
    );
    writer.sample_f64(
        "codexbar_account_quota_remaining_percent",
        &labels,
        window.remaining_percent,
    );
    if let Some(reset_at) = window.reset_at {
        writer.sample(
            "codexbar_account_quota_reset_timestamp_seconds",
            &labels,
            reset_at.timestamp(),
        );
    }
    writer.sample(
        "codexbar_account_quota_idle",
        &labels,
        bool_value(window.idle),
    );
}

fn render_account_pace(
    writer: &mut MetricsWriter,
    provider: &str,
    account: &str,
    window: &str,
    pace: &PacePayload,
) {
    let labels = [
        ("provider", provider),
        ("account", account),
        ("window", window),
    ];
    let stage_labels = [
        ("provider", provider),
        ("account", account),
        ("window", window),
        ("stage", pace.stage.as_str()),
    ];
    writer.sample("codexbar_account_pace_stage_info", &stage_labels, 1);
    writer.sample_f64(
        "codexbar_account_pace_delta_percent",
        &labels,
        pace.delta_percent,
    );
    writer.sample_f64(
        "codexbar_account_pace_expected_used_percent",
        &labels,
        pace.expected_used_percent,
    );
    writer.sample(
        "codexbar_account_pace_will_last_to_reset",
        &labels,
        bool_value(pace.will_last_to_reset),
    );
    if let Some(value) = pace.eta_seconds {
        writer.sample_f64("codexbar_account_pace_eta_seconds", &labels, value);
    }
    if let Some(value) = pace.run_out_probability {
        writer.sample_f64("codexbar_account_pace_run_out_probability", &labels, value);
    }
}

fn bool_value(value: bool) -> u8 {
    u8::from(value)
}

fn age_seconds(now: DateTime<Utc>, updated_at: DateTime<Utc>) -> i64 {
    (now - updated_at).num_seconds().max(0)
}

fn provider_status_value(level: &str) -> u8 {
    match level.trim().to_ascii_lowercase().as_str() {
        "operational" | "none" | "green" | "ok" => 0,
        "degraded" | "degraded_performance" | "yellow" | "minor" => 1,
        "partial" | "partial_outage" | "orange" => 2,
        "major" | "major_outage" | "critical" | "red" | "outage" => 3,
        _ => 4,
    }
}

struct MetricsWriter {
    samples: BTreeMap<String, Vec<String>>,
    series: BTreeSet<String>,
}

impl MetricsWriter {
    fn new() -> Self {
        Self {
            samples: BTreeMap::new(),
            series: BTreeSet::new(),
        }
    }

    fn sample(&mut self, name: &str, labels: &[(&str, &str)], value: impl Display) {
        let series = format_series(name, labels);
        if self.series.insert(series.clone()) {
            self.samples
                .entry(name.to_string())
                .or_default()
                .push(format!("{series} {value}\n"));
        }
    }

    fn sample_f64(&mut self, name: &str, labels: &[(&str, &str)], value: f64) {
        if value.is_finite() {
            self.sample(name, labels, value);
        }
    }

    fn finish(self) -> String {
        let mut body = String::new();
        let mut samples = self.samples;
        for &(name, help) in METRIC_DEFINITIONS {
            body.push_str("# HELP ");
            body.push_str(name);
            body.push(' ');
            body.push_str(help);
            body.push('\n');
            body.push_str("# TYPE ");
            body.push_str(name);
            body.push_str(" gauge\n");
            if let Some(lines) = samples.remove(name) {
                for line in lines {
                    body.push_str(&line);
                }
            }
        }
        // HELP and TYPE are optional for any future metric accidentally omitted
        // from the definitions table, but its samples must still be emitted as
        // one contiguous family.
        for lines in samples.into_values() {
            for line in lines {
                body.push_str(&line);
            }
        }
        debug_assert!(body.ends_with('\n'));
        body
    }
}

fn format_series(name: &str, labels: &[(&str, &str)]) -> String {
    if labels.is_empty() {
        return name.to_string();
    }
    let mut labels = labels.to_vec();
    labels.sort_unstable_by(|left, right| left.0.cmp(right.0));
    let labels = labels
        .into_iter()
        .map(|(name, value)| format!(r#"{name}="{}""#, escape_label(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{name}{{{labels}}}")
}

fn escape_label(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' | '\r' => escaped.push_str("\\n"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::cli::serve::dashboard::coordinator::SnapshotBuildFn;
    use crate::cli::serve::dashboard::snapshot::{
        AccountPayload, CostPayload, CreditsPayload, DisplayPayload, HostPayload, IdentityPayload,
        PacePayload, ProviderErrorPayload, ProviderPacePayload, SnapshotProvider, StatusPayload,
        WindowPayload,
    };

    fn at(hour: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 14, hour, 0, 0)
            .single()
            .unwrap()
    }

    fn window(kind: &str, used: f64, reset_at: Option<chrono::DateTime<Utc>>) -> WindowPayload {
        WindowPayload {
            kind: kind.to_string(),
            label: "sensitive display label".to_string(),
            used_percent: used,
            remaining_percent: 100.0 - used,
            reset_at,
            usage_known: true,
            idle: false,
        }
    }

    fn provider() -> SnapshotProvider {
        SnapshotProvider {
            id: "codex".to_string(),
            name: "Sensitive provider name".to_string(),
            enabled: true,
            source: "sensitive-source".to_string(),
            status: Some(StatusPayload {
                level: "degraded".to_string(),
                label: "Sensitive status text".to_string(),
                updated_at: Some(at(2)),
            }),
            identity: Some(IdentityPayload {
                account_email: Some("owner@example.test".to_string()),
                plan: Some("Sensitive plan".to_string()),
            }),
            windows: vec![
                window("session", 25.0, Some(at(3))),
                WindowPayload {
                    kind: "reset-credits".to_string(),
                    label: "Reset credits".to_string(),
                    used_percent: 0.0,
                    remaining_percent: 100.0,
                    reset_at: Some(at(5)),
                    usage_known: false,
                    idle: false,
                },
            ],
            reset_credits_available: Some(3),
            credits: Some(CreditsPayload {
                remaining: 42.5,
                unit: "credits".to_string(),
            }),
            cost: Some(CostPayload {
                today_usd: Some(1.25),
                last_30_days_usd: Some(12.5),
            }),
            display: DisplayPayload {
                accent_color: "#ffffff".to_string(),
                sort_key: 10,
                priority: "Sensitive priority".to_string(),
            },
            error: None,
            updated_at: Some(at(1)),
            accounts: Some(vec![AccountPayload {
                id: "account-1".to_string(),
                label: "Sensitive account label".to_string(),
                active: true,
                identity: Some(IdentityPayload {
                    account_email: Some("account@example.test".to_string()),
                    plan: Some("Sensitive account plan".to_string()),
                }),
                windows: vec![window("weekly", 40.0, Some(at(4)))],
                pace: Some(ProviderPacePayload {
                    primary: None,
                    secondary: Some(PacePayload {
                        stage: "onTrack".to_string(),
                        delta_percent: 2.0,
                        expected_used_percent: 38.0,
                        will_last_to_reset: true,
                        eta_seconds: Some(3600.0),
                        run_out_probability: Some(0.1),
                        summary: "Sensitive pace summary".to_string(),
                    }),
                    tertiary: None,
                }),
                error: None,
                updated_at: Some(at(1)),
            }]),
            accounts_error: None,
        }
    }

    fn snapshot(providers: Vec<SnapshotProvider>) -> SnapshotPayload {
        SnapshotPayload {
            schema_version: 1,
            generated_at: at(0),
            stale_after_seconds: 180,
            host: HostPayload {
                codex_bar_version: Some("0.56.8".to_string()),
                refresh_interval_seconds: 60,
            },
            providers,
        }
    }

    fn samples<'a>(body: &'a str, metric: &'a str) -> impl Iterator<Item = &'a str> {
        body.lines().filter(move |line| {
            line.starts_with(metric)
                && line
                    .as_bytes()
                    .get(metric.len())
                    .is_some_and(|next| matches!(next, b' ' | b'{'))
        })
    }

    fn metric_name(line: &str) -> Option<&str> {
        if let Some(rest) = line
            .strip_prefix("# HELP ")
            .or_else(|| line.strip_prefix("# TYPE "))
        {
            return rest.split_ascii_whitespace().next();
        }
        if line.starts_with('#') || line.is_empty() {
            return None;
        }
        line.split(['{', ' ', '\t']).next()
    }

    fn assert_metric_families_are_contiguous(body: &str) {
        let mut closed = BTreeSet::new();
        let mut current = None;
        for line in body.lines() {
            let Some(name) = metric_name(line) else {
                continue;
            };
            if current == Some(name) {
                continue;
            }
            if let Some(previous) = current.replace(name) {
                closed.insert(previous);
            }
            assert!(
                !closed.contains(name),
                "metric family {name} is split into multiple groups"
            );
        }
    }

    #[test]
    fn renders_snapshot_data_without_identity_or_error_text() {
        let body = render(&snapshot(vec![provider()]));

        assert!(body.contains("codexbar_up 1\n"));
        assert!(body.contains("codexbar_build_info{schema_version=\"1\",version=\"0.56.8\"} 1\n"));
        assert!(body.contains("codexbar_provider_enabled{provider=\"codex\"} 1\n"));
        assert!(
            body.contains(
                "codexbar_quota_used_percent{provider=\"codex\",window=\"session\"} 25\n"
            )
        );
        assert!(
            body.contains("codexbar_credits_remaining{provider=\"codex\",unit=\"credits\"} 42.5\n")
        );
        assert!(body.contains("codexbar_reset_credits_available{provider=\"codex\"} 3\n"));
        assert!(body.contains(
            "codexbar_reset_credits_next_expiry_timestamp_seconds{provider=\"codex\"} 1789362000\n"
        ));
        assert!(body.contains("codexbar_cost_last_30_days_usd{provider=\"codex\"} 12.5\n"));
        assert!(
            body.contains("codexbar_account_active{account=\"account-1\",provider=\"codex\"} 1\n")
        );
        assert!(body.contains("codexbar_account_pace_stage_info{account=\"account-1\",provider=\"codex\",stage=\"onTrack\",window=\"weekly\"} 1\n"));
        assert!(body.ends_with('\n'));

        for secret in [
            "owner@example.test",
            "account@example.test",
            "Sensitive provider name",
            "Sensitive status text",
            "Sensitive plan",
            "Sensitive account label",
            "Sensitive account plan",
            "Sensitive pace summary",
            "sensitive-source",
        ] {
            assert!(!body.contains(secret), "secret leaked: {secret}");
        }
    }

    #[test]
    fn groups_help_type_and_all_samples_by_metric_family() {
        let first = provider();
        let mut second = provider();
        second.id = "claude".to_string();

        let body = render(&snapshot(vec![first, second]));

        assert_metric_families_are_contiguous(&body);
        assert_eq!(samples(&body, "codexbar_provider_enabled").count(), 2);
        assert_eq!(
            samples(&body, "codexbar_reset_credits_available").count(),
            1
        );
        assert!(body.contains(concat!(
            "# HELP codexbar_provider_enabled Whether the provider is enabled.\n",
            "# TYPE codexbar_provider_enabled gauge\n",
            "codexbar_provider_enabled{provider=\"codex\"} 1\n",
            "codexbar_provider_enabled{provider=\"claude\"} 1\n",
        )));
    }

    #[test]
    fn omits_absent_and_non_finite_samples() {
        let mut provider = provider();
        provider.updated_at = None;
        provider.status = None;
        provider.reset_credits_available = None;
        provider.credits = None;
        provider.cost = None;
        provider.accounts = None;
        provider.windows[0].reset_at = None;
        provider.windows[0].used_percent = f64::NAN;
        provider.windows[0].remaining_percent = f64::INFINITY;

        let body = render(&snapshot(vec![provider]));
        for metric in [
            "codexbar_provider_updated_timestamp_seconds",
            "codexbar_provider_status",
            "codexbar_credits_remaining",
            "codexbar_cost_today_usd",
            "codexbar_cost_last_30_days_usd",
            "codexbar_provider_accounts_total",
            "codexbar_quota_reset_timestamp_seconds",
            "codexbar_quota_used_percent",
            "codexbar_quota_remaining_percent",
        ] {
            assert_eq!(
                samples(&body, metric).count(),
                0,
                "unexpected {metric} sample"
            );
        }
    }

    #[test]
    fn distinguishes_exhausted_and_unavailable_codex_reset_credits() {
        let mut provider = provider();
        provider.reset_credits_available = Some(0);
        let exhausted = render(&snapshot(vec![provider.clone()]));
        assert!(exhausted.contains("codexbar_reset_credits_available{provider=\"codex\"} 0\n"));
        assert_eq!(
            samples(
                &exhausted,
                "codexbar_reset_credits_next_expiry_timestamp_seconds"
            )
            .count(),
            0
        );

        provider.reset_credits_available = None;
        let unavailable = render(&snapshot(vec![provider]));
        assert!(unavailable.contains("codexbar_reset_credits_available{provider=\"codex\"} -1\n"));
        assert_eq!(
            samples(
                &unavailable,
                "codexbar_reset_credits_next_expiry_timestamp_seconds"
            )
            .count(),
            0
        );
    }

    #[test]
    fn omits_unknown_quota_windows_and_preserves_over_quota_usage() {
        let mut provider = provider();
        provider.windows[0].usage_known = false;
        provider.accounts.as_mut().unwrap()[0].windows[0].usage_known = false;
        provider.windows.push(WindowPayload {
            kind: "over-quota".to_string(),
            label: "Over quota".to_string(),
            used_percent: 115.0,
            remaining_percent: 0.0,
            reset_at: Some(at(4)),
            usage_known: true,
            idle: false,
        });

        let body = render(&snapshot(vec![provider]));
        for metric in [
            "codexbar_quota_used_percent",
            "codexbar_quota_remaining_percent",
            "codexbar_quota_reset_timestamp_seconds",
            "codexbar_quota_idle",
        ] {
            assert!(
                samples(&body, metric).all(|line| !line.contains("window=\"session\"")),
                "unknown provider window leaked through {metric}"
            );
        }
        for metric in [
            "codexbar_account_quota_used_percent",
            "codexbar_account_quota_remaining_percent",
            "codexbar_account_quota_reset_timestamp_seconds",
            "codexbar_account_quota_idle",
        ] {
            assert_eq!(
                samples(&body, metric).count(),
                0,
                "unknown account window leaked through {metric}"
            );
        }
        assert!(body.contains(
            "codexbar_quota_used_percent{provider=\"codex\",window=\"over-quota\"} 115\n"
        ));
        assert!(body.contains(
            "codexbar_quota_remaining_percent{provider=\"codex\",window=\"over-quota\"} 0\n"
        ));
    }

    #[test]
    fn reports_failures_without_exporting_error_messages() {
        let mut failed = provider();
        failed.error = Some(ProviderErrorPayload {
            code: 17,
            message: "Bearer token and account email must stay private".to_string(),
            kind: Some("secret-kind".to_string()),
        });
        failed.accounts = None;
        failed.accounts_error = Some("account adapter secret".to_string());

        let body = render(&snapshot(vec![failed]));
        assert!(body.contains("codexbar_provider_up{provider=\"codex\"} 0\n"));
        assert!(body.contains("codexbar_provider_error_code{provider=\"codex\"} 17\n"));
        assert!(body.contains("codexbar_provider_accounts_up{provider=\"codex\"} 0\n"));
        assert_eq!(
            samples(&body, "codexbar_provider_updated_timestamp_seconds").count(),
            0
        );
        assert_eq!(
            samples(&body, "codexbar_provider_data_age_seconds").count(),
            0
        );
        assert!(!body.contains("Bearer token"));
        assert!(!body.contains("account adapter secret"));
        assert!(!body.contains("secret-kind"));
    }

    #[test]
    fn escapes_labels_and_suppresses_duplicate_series() {
        let mut first = provider();
        first.id = "co\"dex\\lan\nnode".to_string();
        first.windows.push(first.windows[0].clone());
        let mut duplicate = provider();
        duplicate.id = first.id.clone();

        let body = render(&snapshot(vec![first, duplicate]));
        assert!(body.contains(r#"provider="co\"dex\\lan\nnode""#));
        assert_eq!(samples(&body, "codexbar_provider_enabled").count(), 1);
        assert_eq!(samples(&body, "codexbar_quota_used_percent").count(), 1);
        assert_eq!(samples(&body, "codexbar_account_active").count(), 1);
    }

    #[test]
    fn freshness_metrics_are_deterministic_clamped_and_use_strict_staleness() {
        let snapshot = snapshot(vec![provider()]);
        let at_threshold = render_at(
            &snapshot,
            snapshot.generated_at + chrono::Duration::seconds(180),
        );
        assert!(at_threshold.contains("codexbar_snapshot_age_seconds 180\n"));
        assert!(at_threshold.contains("codexbar_snapshot_stale 0\n"));

        let stale = render_at(
            &snapshot,
            snapshot.generated_at + chrono::Duration::seconds(181),
        );
        assert!(stale.contains("codexbar_snapshot_stale 1\n"));

        let before_provider_update = render_at(&snapshot, snapshot.generated_at);
        assert!(
            before_provider_update
                .contains("codexbar_provider_data_age_seconds{provider=\"codex\"} 0\n")
        );

        let later = render_at(&snapshot, at(5));
        assert!(later.contains("codexbar_provider_data_age_seconds{provider=\"codex\"} 14400\n"));
    }

    #[tokio::test]
    async fn failed_snapshot_build_returns_valid_metrics_without_error_text() {
        let build: SnapshotBuildFn =
            Arc::new(|| Box::pin(async { Err("sensitive snapshot failure".to_string()) }));
        let state = dashboard::DashboardState::stub(
            build,
            60,
            Some(dashboard::snapshot::DashboardIdentity::Redacted),
        );

        let response = response(&state).await;
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains(&format!("Content-Type: {CONTENT_TYPE}\r\n")));
        assert!(response.contains("codexbar_up 0\n"));
        assert!(response.contains("codexbar_reset_credits_available{provider=\"codex\"} -1\n"));
        assert_eq!(
            response
                .lines()
                .filter(|line| line.starts_with("codexbar_"))
                .count(),
            2
        );
        assert!(!response.contains("sensitive snapshot failure"));
    }
}
