use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::timers::engine::UpcomingEvent;

/// Per-axis weights and the urgency window (minutes). Defaults come from
/// SPEC §5.4 and live in the `[scorer]` section of `config.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Weights {
    pub urgency: f64,
    pub completion: f64,
    pub reward: f64,
    pub effort: f64,
    /// Minutes ahead at which urgency starts ramping up; spawns inside the
    /// last 10 minutes always score 100.
    pub urgency_window_minutes: i64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            urgency: 0.5,
            completion: 0.2,
            reward: 0.2,
            effort: 0.1,
            urgency_window_minutes: 120,
        }
    }
}

/// A single rankable trackable (achievement, WV objective, etc.) normalized
/// for the scoring function. Caller maps domain data → Scoreable.
#[derive(Debug, Clone)]
pub struct Scoreable {
    pub id: String,
    /// In [0, 1]: fraction of progress already done. `done == 1.0`.
    pub completion_ratio: f64,
    /// Raw reward value (AP or acclaim equivalent). Capped at 100 internally.
    pub reward_value: u32,
    /// Estimated minutes to finish. 0 = "instant / unknown".
    pub effort_minutes: u32,
    /// IDs of boss / meta events whose spawn satisfies this trackable.
    pub related_event_ids: Vec<String>,
}

/// Final score in [0, ~100]. The scoring loop in the UI ranks descending.
pub fn score(
    item: &Scoreable,
    upcoming: &[UpcomingEvent],
    weights: &Weights,
    now: DateTime<Utc>,
) -> f64 {
    let urgency = urgency_for(&item.related_event_ids, upcoming, weights.urgency_window_minutes, now);
    let completion = item.completion_ratio.clamp(0.0, 1.0) * 100.0;
    let reward = (item.reward_value as f64).min(100.0);
    let effort = effort_score(item.effort_minutes);

    urgency * weights.urgency
        + completion * weights.completion
        + reward * weights.reward
        + effort * weights.effort
}

pub fn rank<I>(items: I, upcoming: &[UpcomingEvent], weights: &Weights, now: DateTime<Utc>) -> Vec<(Scoreable, f64)>
where
    I: IntoIterator<Item = Scoreable>,
{
    let mut scored: Vec<(Scoreable, f64)> = items
        .into_iter()
        .map(|item| {
            let s = score(&item, upcoming, weights, now);
            (item, s)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

fn urgency_for(
    related: &[String],
    upcoming: &[UpcomingEvent],
    window_minutes: i64,
    now: DateTime<Utc>,
) -> f64 {
    if related.is_empty() || window_minutes <= 10 {
        return 0.0;
    }
    let mut best = 0.0_f64;
    for id in related {
        for ev in upcoming.iter().filter(|e| &e.id == id) {
            let ends_at = ev.start_at + Duration::minutes(ev.duration_minutes as i64);
            let mins_until = (ev.start_at - now).num_minutes();
            let active_now = now >= ev.start_at && now < ends_at;
            let u = if active_now || mins_until <= 10 {
                100.0
            } else if mins_until <= window_minutes {
                let span = (window_minutes - 10) as f64;
                let pos = (mins_until - 10) as f64;
                100.0 * (1.0 - pos / span)
            } else {
                0.0
            };
            if u > best {
                best = u;
            }
        }
    }
    best
}

fn effort_score(minutes: u32) -> f64 {
    // Baseline 10 min → 100. Linear in 1/effort. Achievements ≤ 10 min all
    // score 100; 20 min → 50; 100 min → 10.
    if minutes == 0 {
        return 100.0;
    }
    (100.0 * 10.0 / (minutes as f64).max(10.0)).min(100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    use crate::timers::engine::UpcomingKind;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 24, 12, 0, 0).unwrap()
    }

    fn ev(id: &str, start_minutes_from_now: i64, duration: u32) -> UpcomingEvent {
        UpcomingEvent {
            id: id.into(),
            name: id.into(),
            map: "Test".into(),
            kind: UpcomingKind::WorldBoss,
            start_at: now() + Duration::minutes(start_minutes_from_now),
            duration_minutes: duration,
        }
    }

    fn item(related: &[&str], completion: f64, reward: u32, effort: u32) -> Scoreable {
        Scoreable {
            id: "x".into(),
            completion_ratio: completion,
            reward_value: reward,
            effort_minutes: effort,
            related_event_ids: related.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn urgency_full_when_event_currently_active() {
        let evs = vec![ev("teq", -5, 30)];
        let u = urgency_for(&["teq".into()], &evs, 120, now());
        assert_eq!(u, 100.0);
    }

    #[test]
    fn urgency_full_when_within_10_min() {
        let evs = vec![ev("teq", 7, 30)];
        let u = urgency_for(&["teq".into()], &evs, 120, now());
        assert_eq!(u, 100.0);
    }

    #[test]
    fn urgency_zero_when_outside_window() {
        let evs = vec![ev("teq", 240, 30)];
        let u = urgency_for(&["teq".into()], &evs, 120, now());
        assert_eq!(u, 0.0);
    }

    #[test]
    fn urgency_decays_linearly_between_10_and_window() {
        // window=120, span=110. At 65min ahead: pos=55, fraction=0.5, urgency=50
        let evs = vec![ev("teq", 65, 30)];
        let u = urgency_for(&["teq".into()], &evs, 120, now());
        assert!((u - 50.0).abs() < 0.01, "got {u}");
    }

    #[test]
    fn urgency_takes_max_across_related_events() {
        let evs = vec![ev("a", 100, 5), ev("b", 5, 5)];
        let u = urgency_for(&["a".into(), "b".into()], &evs, 120, now());
        assert_eq!(u, 100.0); // b is imminent
    }

    #[test]
    fn urgency_zero_when_no_related_events_in_feed() {
        let evs = vec![ev("other", 5, 5)];
        let u = urgency_for(&["nope".into()], &evs, 120, now());
        assert_eq!(u, 0.0);
    }

    #[test]
    fn completion_boosts_score() {
        let weights = Weights::default();
        let upcoming: Vec<UpcomingEvent> = vec![];
        let low = item(&[], 0.1, 0, 30);
        let high = item(&[], 0.9, 0, 30);
        assert!(score(&high, &upcoming, &weights, now()) > score(&low, &upcoming, &weights, now()));
    }

    #[test]
    fn shorter_effort_boosts_score() {
        let weights = Weights::default();
        let upcoming: Vec<UpcomingEvent> = vec![];
        let short = item(&[], 0.5, 0, 5);
        let long = item(&[], 0.5, 0, 60);
        assert!(score(&short, &upcoming, &weights, now()) > score(&long, &upcoming, &weights, now()));
    }

    #[test]
    fn rank_returns_descending() {
        let weights = Weights::default();
        let evs = vec![ev("imminent", 3, 5)];
        let items = vec![
            Scoreable {
                id: "boring".into(),
                completion_ratio: 0.0,
                reward_value: 0,
                effort_minutes: 60,
                related_event_ids: vec![],
            },
            Scoreable {
                id: "urgent".into(),
                completion_ratio: 0.0,
                reward_value: 0,
                effort_minutes: 60,
                related_event_ids: vec!["imminent".into()],
            },
            Scoreable {
                id: "almost_done".into(),
                completion_ratio: 0.95,
                reward_value: 0,
                effort_minutes: 60,
                related_event_ids: vec![],
            },
        ];
        let ranked = rank(items, &evs, &weights, now());
        let order: Vec<&str> = ranked.iter().map(|(i, _)| i.id.as_str()).collect();
        assert_eq!(order[0], "urgent"); // urgency dominates
        assert_eq!(order[2], "boring");
    }

    #[test]
    fn weights_change_ranking() {
        let evs = vec![ev("imminent", 3, 5)];
        let urgent = Scoreable {
            id: "urgent".into(),
            completion_ratio: 0.0,
            reward_value: 0,
            effort_minutes: 60,
            related_event_ids: vec!["imminent".into()],
        };
        let almost_done = Scoreable {
            id: "almost_done".into(),
            completion_ratio: 0.95,
            reward_value: 0,
            effort_minutes: 60,
            related_event_ids: vec![],
        };

        // Default weights: urgency dominates.
        let default_ranked = rank(
            vec![urgent.clone(), almost_done.clone()],
            &evs,
            &Weights::default(),
            now(),
        );
        assert_eq!(default_ranked[0].0.id, "urgent");

        // Heavy completion bias inverts the order.
        let completion_first = Weights {
            urgency: 0.0,
            completion: 1.0,
            reward: 0.0,
            effort: 0.0,
            urgency_window_minutes: 120,
        };
        let ranked = rank(
            vec![urgent, almost_done],
            &evs,
            &completion_first,
            now(),
        );
        assert_eq!(ranked[0].0.id, "almost_done");
    }
}

