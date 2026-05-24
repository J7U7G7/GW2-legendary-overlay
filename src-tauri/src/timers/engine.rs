use chrono::{DateTime, Duration, NaiveTime, TimeZone, Utc};
use serde::Serialize;

use crate::timers::schedule::{MetaEvent, Schedule, WorldBoss, parse_hm};

/// Compute the next spawn time for a world boss strictly after `now`.
/// The schedule_utc entries are local-to-day spawn times; if all of today's
/// spawns are past, we roll over to the first spawn tomorrow.
pub fn next_spawn(boss: &WorldBoss, now: DateTime<Utc>) -> DateTime<Utc> {
    let today_midnight = today_midnight_utc(now);
    let mut today: Vec<DateTime<Utc>> = boss
        .schedule_utc
        .iter()
        .filter_map(|s| parse_hm(s).ok())
        .map(|(h, m)| today_midnight + Duration::hours(h as i64) + Duration::minutes(m as i64))
        .collect();
    today.sort();

    match today.iter().find(|&&t| t > now).copied() {
        Some(t) => t,
        None => {
            // All today's spawns have passed; first spawn tomorrow.
            let tomorrow_midnight = today_midnight + Duration::days(1);
            let (h, m) = boss
                .schedule_utc
                .first()
                .and_then(|s| parse_hm(s).ok())
                .unwrap_or((0, 0));
            tomorrow_midnight + Duration::hours(h as i64) + Duration::minutes(m as i64)
        }
    }
}

/// For a meta event with cyclical phases, return the currently-active phase
/// (if any) and the next phase start, both anchored to UTC.
pub fn current_meta_phase(meta: &MetaEvent, now: DateTime<Utc>) -> MetaPhaseInstant {
    let anchor = anchor_datetime(meta, now);
    let cycle = Duration::minutes(meta.cycle_minutes as i64);

    // Bring `now` into the [anchor, anchor + cycle) window.
    let mut elapsed = now.signed_duration_since(anchor);
    while elapsed.num_minutes() < 0 {
        elapsed += cycle;
    }
    let within_cycle_minutes = (elapsed.num_minutes() % meta.cycle_minutes as i64) as u32;
    let cycle_origin = now - Duration::minutes(within_cycle_minutes as i64);

    let active = meta
        .phases
        .iter()
        .find(|p| {
            within_cycle_minutes >= p.offset_minutes
                && within_cycle_minutes < p.offset_minutes + p.duration_minutes
        })
        .map(|p| ActivePhase {
            name: p.name.clone(),
            started_at: cycle_origin + Duration::minutes(p.offset_minutes as i64),
            ends_at: cycle_origin
                + Duration::minutes((p.offset_minutes + p.duration_minutes) as i64),
        });

    // Next phase start (first phase whose offset > within_cycle, else first
    // phase of the next cycle).
    let next_phase = meta
        .phases
        .iter()
        .find(|p| p.offset_minutes > within_cycle_minutes)
        .map(|p| NextPhase {
            name: p.name.clone(),
            starts_at: cycle_origin + Duration::minutes(p.offset_minutes as i64),
        })
        .unwrap_or_else(|| {
            let first = &meta.phases[0];
            NextPhase {
                name: first.name.clone(),
                starts_at: cycle_origin + cycle + Duration::minutes(first.offset_minutes as i64),
            }
        });

    MetaPhaseInstant { active, next: next_phase }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MetaPhaseInstant {
    pub active: Option<ActivePhase>,
    pub next: NextPhase,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActivePhase {
    pub name: String,
    pub started_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NextPhase {
    pub name: String,
    pub starts_at: DateTime<Utc>,
}

/// Aggregate upcoming events (bosses + meta phases) within the next
/// `horizon_minutes`, sorted ascending by start time. Useful for the overlay's
/// urgency feed and for feeding the scorer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpcomingEvent {
    pub id: String,
    pub name: String,
    pub map: String,
    pub kind: UpcomingKind,
    pub start_at: DateTime<Utc>,
    pub duration_minutes: u32,
    pub waypoint_code: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpcomingKind {
    WorldBoss,
    MetaPhase,
}

pub fn all_upcoming(schedule: &Schedule, now: DateTime<Utc>, horizon_minutes: i64) -> Vec<UpcomingEvent> {
    let horizon = now + Duration::minutes(horizon_minutes);
    let mut out = Vec::new();

    for boss in &schedule.world_bosses {
        let t = next_spawn(boss, now);
        if t <= horizon {
            out.push(UpcomingEvent {
                id: boss.id.clone(),
                name: boss.name.clone(),
                map: boss.map.clone(),
                kind: UpcomingKind::WorldBoss,
                start_at: t,
                duration_minutes: boss.duration_minutes,
                waypoint_code: boss.waypoint_code.clone(),
            });
        }
    }
    for meta in &schedule.meta_events {
        let instant = current_meta_phase(meta, now);
        if instant.next.starts_at <= horizon {
            // duration of the *next* phase
            let next_phase_dur = meta
                .phases
                .iter()
                .find(|p| p.name == instant.next.name)
                .map(|p| p.duration_minutes)
                .unwrap_or(0);
            out.push(UpcomingEvent {
                id: meta.id.clone(),
                name: format!("{} — {}", meta.name, instant.next.name),
                map: meta.map.clone(),
                kind: UpcomingKind::MetaPhase,
                start_at: instant.next.starts_at,
                duration_minutes: next_phase_dur,
                waypoint_code: None,
            });
        }
    }

    out.sort_by_key(|e| e.start_at);
    out
}

fn today_midnight_utc(now: DateTime<Utc>) -> DateTime<Utc> {
    Utc.from_utc_datetime(&now.date_naive().and_time(NaiveTime::MIN))
}

/// Returns an anchor datetime in UTC for the meta event's canonical cycle
/// origin. We pick today's anchor; the modulo arithmetic in
/// `current_meta_phase` handles whether `now` is before or after it.
fn anchor_datetime(meta: &MetaEvent, now: DateTime<Utc>) -> DateTime<Utc> {
    let (h, m) = parse_hm(&meta.anchor_utc).unwrap_or((0, 0));
    today_midnight_utc(now) + Duration::hours(h as i64) + Duration::minutes(m as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timers::schedule::{MetaPhase, WorldBoss};

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    fn boss(times: &[&str]) -> WorldBoss {
        WorldBoss {
            id: "test".into(),
            name: "Test Boss".into(),
            tier: None,
            map: "Test Map".into(),
            area: None,
            waypoint_code: Some("[&AAAAAAAA=]".into()),
            expansion: "Core".into(),
            schedule_utc: times.iter().map(|s| (*s).to_string()).collect(),
            duration_minutes: 15,
            wiki_event: None,
        }
    }

    #[test]
    fn next_spawn_picks_first_future_today() {
        let b = boss(&["00:00", "06:00", "12:00", "18:00"]);
        let now = utc(2026, 5, 24, 7, 0);
        assert_eq!(next_spawn(&b, now), utc(2026, 5, 24, 12, 0));
    }

    #[test]
    fn next_spawn_rolls_over_to_tomorrow() {
        let b = boss(&["00:00", "06:00", "12:00", "18:00"]);
        let now = utc(2026, 5, 24, 23, 30);
        assert_eq!(next_spawn(&b, now), utc(2026, 5, 25, 0, 0));
    }

    #[test]
    fn next_spawn_exact_match_picks_next() {
        let b = boss(&["12:00", "18:00"]);
        let now = utc(2026, 5, 24, 12, 0);
        // Strictly greater: 18:00 is next, not 12:00.
        assert_eq!(next_spawn(&b, now), utc(2026, 5, 24, 18, 0));
    }

    fn meta() -> MetaEvent {
        MetaEvent {
            id: "ds".into(),
            name: "Dragon's Stand".into(),
            expansion: None,
            map: "Dragon's Stand".into(),
            cycle_minutes: 120,
            anchor_utc: "00:30".into(),
            phases: vec![
                MetaPhase { offset_minutes: 0, name: "Lanes".into(), duration_minutes: 90 },
                MetaPhase {
                    offset_minutes: 90,
                    name: "Mouth of Mordremoth".into(),
                    duration_minutes: 30,
                },
            ],
        }
    }

    #[test]
    fn meta_active_phase_inside_lanes() {
        let m = meta();
        let now = utc(2026, 5, 24, 1, 0); // 30 min into the cycle that started at 00:30
        let i = current_meta_phase(&m, now);
        let active = i.active.expect("should be active");
        assert_eq!(active.name, "Lanes");
        assert_eq!(active.started_at, utc(2026, 5, 24, 0, 30));
        assert_eq!(i.next.name, "Mouth of Mordremoth");
        assert_eq!(i.next.starts_at, utc(2026, 5, 24, 2, 0));
    }

    #[test]
    fn meta_active_phase_inside_mouth() {
        let m = meta();
        let now = utc(2026, 5, 24, 2, 15); // 1h45 into cycle (in Mouth phase)
        let i = current_meta_phase(&m, now);
        let active = i.active.expect("should be active");
        assert_eq!(active.name, "Mouth of Mordremoth");
        assert_eq!(active.started_at, utc(2026, 5, 24, 2, 0));
        // Next phase wraps to the next cycle's first phase
        assert_eq!(i.next.name, "Lanes");
        assert_eq!(i.next.starts_at, utc(2026, 5, 24, 2, 30));
    }

    #[test]
    fn meta_before_anchor_walks_back_a_cycle() {
        let m = meta();
        let now = utc(2026, 5, 24, 0, 15); // 15 min before today's anchor
        let i = current_meta_phase(&m, now);
        // We're inside the cycle that started yesterday 22:30 → Lanes (offset 0)
        // covers [22:30, 00:00); Mouth (offset 90) covers [00:00, 00:30).
        let active = i.active.expect("active");
        assert_eq!(active.name, "Mouth of Mordremoth");
        assert_eq!(i.next.name, "Lanes");
        assert_eq!(i.next.starts_at, utc(2026, 5, 24, 0, 30));
    }

    #[test]
    fn all_upcoming_is_sorted_and_respects_horizon() {
        let schedule = Schedule {
            world_bosses: vec![
                {
                    let mut b = boss(&["10:00", "20:00"]);
                    b.id = "early".into();
                    b
                },
                {
                    let mut b = boss(&["11:30"]);
                    b.id = "late".into();
                    b
                },
                {
                    let mut b = boss(&["23:00"]);
                    b.id = "outside".into();
                    b
                },
            ],
            meta_events: vec![],
            ley_line_anomaly: None,
        };
        let now = utc(2026, 5, 24, 9, 0);
        let upcoming = all_upcoming(&schedule, now, 180); // 3h window
        let ids: Vec<&str> = upcoming.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["early", "late"]);
    }
}
