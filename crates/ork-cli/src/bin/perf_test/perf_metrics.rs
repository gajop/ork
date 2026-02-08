use std::collections::VecDeque;
use std::io::BufRead;

use super::SchedulerMetrics;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StatusCounts {
    pub pending: i64,
    pub dispatched: i64,
    pub running: i64,
    pub success: i64,
    pub failed: i64,
}

pub fn status_counts_from_rows(rows: Vec<(String, i64)>) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for (status, count) in rows {
        match status.as_str() {
            "pending" => counts.pending = count,
            "dispatched" => counts.dispatched = count,
            "running" => counts.running = count,
            "success" => counts.success = count,
            "failed" => counts.failed = count,
            _ => {}
        }
    }
    counts
}

pub fn parse_metrics_line(line: &str) -> Option<SchedulerMetrics> {
    let json_start = line.find("SCHEDULER_METRICS: ")?;
    let json_str = &line[json_start + "SCHEDULER_METRICS: ".len()..];
    serde_json::from_str::<SchedulerMetrics>(json_str).ok()
}

pub fn append_metrics_from_reader<R: BufRead>(
    reader: R,
    all_metrics: &mut VecDeque<SchedulerMetrics>,
) {
    for line in reader.lines().map_while(Result::ok) {
        if let Some(metrics) = parse_metrics_line(&line) {
            all_metrics.push_back(metrics);
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MetricsSummary {
    pub total_runs_ms: u128,
    pub total_tasks_ms: u128,
    pub total_status_updates_ms: u128,
    pub total_sleep_ms: u128,
    pub total_loop_ms: u128,
    pub loops: usize,
    pub last_timestamp: u64,
}

impl MetricsSummary {
    fn pct(part: u128, total: u128) -> f64 {
        if total == 0 {
            0.0
        } else {
            (part as f64 / total as f64) * 100.0
        }
    }

    pub fn runs_pct(self) -> f64 {
        Self::pct(self.total_runs_ms, self.total_loop_ms)
    }

    pub fn tasks_pct(self) -> f64 {
        Self::pct(self.total_tasks_ms, self.total_loop_ms)
    }

    pub fn status_updates_pct(self) -> f64 {
        Self::pct(self.total_status_updates_ms, self.total_loop_ms)
    }

    pub fn sleep_pct(self) -> f64 {
        Self::pct(self.total_sleep_ms, self.total_loop_ms)
    }
}

pub fn summarize_metrics(all_metrics: &VecDeque<SchedulerMetrics>) -> Option<MetricsSummary> {
    if all_metrics.is_empty() {
        return None;
    }

    Some(MetricsSummary {
        total_runs_ms: all_metrics.iter().map(|m| m.process_pending_runs_ms).sum(),
        total_tasks_ms: all_metrics.iter().map(|m| m.process_pending_tasks_ms).sum(),
        total_status_updates_ms: all_metrics
            .iter()
            .map(|m| m.process_status_updates_ms)
            .sum(),
        total_sleep_ms: all_metrics.iter().map(|m| m.sleep_ms).sum(),
        total_loop_ms: all_metrics.iter().map(|m| m.total_loop_ms).sum(),
        loops: all_metrics.len(),
        last_timestamp: all_metrics.back().map(|m| m.timestamp).unwrap_or(0),
    })
}

pub fn format_metrics_display(all_metrics: &VecDeque<SchedulerMetrics>) -> String {
    if let Some(summary) = summarize_metrics(all_metrics) {
        format!(
            "runs:{:.1}s/{:.1}% tasks:{:.1}s/{:.1}% updates:{:.1}s/{:.1}% sleep:{:.1}s/{:.1}% total:{:.1}s ({} loops, last_ts:{})",
            summary.total_runs_ms as f64 / 1000.0,
            summary.runs_pct(),
            summary.total_tasks_ms as f64 / 1000.0,
            summary.tasks_pct(),
            summary.total_status_updates_ms as f64 / 1000.0,
            summary.status_updates_pct(),
            summary.total_sleep_ms as f64 / 1000.0,
            summary.sleep_pct(),
            summary.total_loop_ms as f64 / 1000.0,
            summary.loops,
            summary.last_timestamp
        )
    } else {
        "waiting for metrics...".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(
        ts: u64,
        runs: u128,
        tasks: u128,
        updates: u128,
        sleep: u128,
        total: u128,
    ) -> SchedulerMetrics {
        SchedulerMetrics {
            timestamp: ts,
            process_pending_runs_ms: runs,
            process_pending_tasks_ms: tasks,
            process_status_updates_ms: updates,
            sleep_ms: sleep,
            total_loop_ms: total,
        }
    }

    #[test]
    fn test_status_counts_from_rows_maps_known_values() {
        let counts = status_counts_from_rows(vec![
            ("pending".to_string(), 3),
            ("running".to_string(), 2),
            ("success".to_string(), 7),
            ("failed".to_string(), 1),
            ("dispatched".to_string(), 4),
            ("unknown".to_string(), 99),
        ]);
        assert_eq!(
            counts,
            StatusCounts {
                pending: 3,
                dispatched: 4,
                running: 2,
                success: 7,
                failed: 1,
            }
        );
    }

    #[test]
    fn test_parse_metrics_line_handles_missing_and_invalid_payloads() {
        assert!(parse_metrics_line("random line").is_none());
        assert!(parse_metrics_line("SCHEDULER_METRICS: {").is_none());

        let parsed = parse_metrics_line(
            r#"INFO SCHEDULER_METRICS: {"timestamp":1,"process_pending_runs_ms":2,"process_pending_tasks_ms":3,"process_status_updates_ms":4,"sleep_ms":5,"total_loop_ms":6}"#,
        )
        .expect("should parse metrics");
        assert_eq!(parsed.timestamp, 1);
        assert_eq!(parsed.total_loop_ms, 6);
    }

    #[test]
    fn test_append_metrics_from_reader_collects_only_valid_lines() {
        let input = "\
noise
SCHEDULER_METRICS: {\"timestamp\":10,\"process_pending_runs_ms\":1,\"process_pending_tasks_ms\":1,\"process_status_updates_ms\":1,\"sleep_ms\":1,\"total_loop_ms\":4}
SCHEDULER_METRICS: {
SCHEDULER_METRICS: {\"timestamp\":11,\"process_pending_runs_ms\":2,\"process_pending_tasks_ms\":3,\"process_status_updates_ms\":4,\"sleep_ms\":1,\"total_loop_ms\":10}
";
        let mut all = VecDeque::new();
        append_metrics_from_reader(std::io::Cursor::new(input), &mut all);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].timestamp, 10);
        assert_eq!(all[1].timestamp, 11);
    }

    #[test]
    fn test_summarize_and_format_metrics() {
        let mut all = VecDeque::new();
        all.push_back(metric(10, 100, 200, 300, 400, 1000));
        all.push_back(metric(20, 50, 100, 150, 200, 500));

        let summary = summarize_metrics(&all).expect("summary");
        assert_eq!(summary.total_runs_ms, 150);
        assert_eq!(summary.total_tasks_ms, 300);
        assert_eq!(summary.total_status_updates_ms, 450);
        assert_eq!(summary.total_sleep_ms, 600);
        assert_eq!(summary.total_loop_ms, 1500);
        assert_eq!(summary.loops, 2);
        assert_eq!(summary.last_timestamp, 20);

        let text = format_metrics_display(&all);
        assert!(text.contains("runs:0.1s/10.0%"));
        assert!(text.contains("tasks:0.3s/20.0%"));
        assert!(text.contains("updates:0.5s/30.0%") || text.contains("updates:0.4s/30.0%"));
        assert!(text.contains("sleep:0.6s/40.0%"));
        assert!(text.contains("last_ts:20"));
    }

    #[test]
    fn test_format_metrics_display_empty() {
        let all = VecDeque::new();
        assert_eq!(format_metrics_display(&all), "waiting for metrics...");
        assert!(summarize_metrics(&all).is_none());
    }
}
