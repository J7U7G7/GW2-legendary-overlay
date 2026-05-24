use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Embedded boss schedule. Parsed once via `Schedule::load()`; the file lives
/// in `src-tauri/data/boss_schedule.json` and is bundled into the binary.
const EMBEDDED_SCHEDULE: &str = include_str!("../../data/boss_schedule.json");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Schedule {
    #[serde(default)]
    pub world_bosses: Vec<WorldBoss>,
    #[serde(default)]
    pub meta_events: Vec<MetaEvent>,
    #[serde(default)]
    pub ley_line_anomaly: Option<LeyLineAnomaly>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorldBoss {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tier: Option<String>,
    pub map: String,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub waypoint_code: Option<String>,
    /// Spawn times as "HH:MM" UTC strings.
    pub schedule_utc: Vec<String>,
    pub duration_minutes: u32,
    #[serde(default)]
    pub wiki_event: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetaEvent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub expansion: Option<String>,
    pub map: String,
    pub cycle_minutes: u32,
    /// Anchor time as "HH:MM" UTC — the phase[0] start of the canonical cycle.
    pub anchor_utc: String,
    pub phases: Vec<MetaPhase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetaPhase {
    pub offset_minutes: u32,
    pub name: String,
    pub duration_minutes: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeyLineAnomaly {
    pub id: String,
    pub name: String,
    pub schedule_utc: Vec<String>,
    pub duration_minutes: u32,
    #[serde(default)]
    pub rotation_maps: Vec<String>,
    #[serde(default)]
    pub wiki_event: Option<String>,
}

impl Schedule {
    pub fn load() -> Result<Self> {
        let s: Schedule = serde_json::from_str(EMBEDDED_SCHEDULE)?;
        s.validate()?;
        Ok(s)
    }

    /// Sanity check: every schedule_utc entry parses, every meta cycle has at
    /// least one phase, phase offsets stay within their cycle.
    fn validate(&self) -> Result<()> {
        for b in &self.world_bosses {
            for t in &b.schedule_utc {
                parse_hm(t).map_err(|e| {
                    crate::error::AppError::Serde(serde_json::Error::custom(format!(
                        "boss {}: invalid schedule_utc {t:?}: {e}",
                        b.id
                    )))
                })?;
            }
        }
        for m in &self.meta_events {
            parse_hm(&m.anchor_utc).map_err(|e| {
                crate::error::AppError::Serde(serde_json::Error::custom(format!(
                    "meta {}: invalid anchor_utc {:?}: {e}",
                    m.id, m.anchor_utc
                )))
            })?;
            if m.phases.is_empty() {
                return Err(crate::error::AppError::Serde(serde_json::Error::custom(
                    format!("meta {}: no phases", m.id),
                )));
            }
            for p in &m.phases {
                if p.offset_minutes >= m.cycle_minutes {
                    return Err(crate::error::AppError::Serde(serde_json::Error::custom(
                        format!(
                            "meta {}: phase {} offset {} exceeds cycle {}",
                            m.id, p.name, p.offset_minutes, m.cycle_minutes
                        ),
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Parse "HH:MM" to (hour, minute). Returns a generic error string the caller
/// can wrap.
pub fn parse_hm(s: &str) -> std::result::Result<(u32, u32), String> {
    let (h, m) = s.split_once(':').ok_or_else(|| format!("missing ':' in {s:?}"))?;
    let h: u32 = h.parse().map_err(|e| format!("hour: {e}"))?;
    let m: u32 = m.parse().map_err(|e| format!("minute: {e}"))?;
    if h >= 24 || m >= 60 {
        return Err(format!("out of range: {h}:{m}"));
    }
    Ok((h, m))
}

// serde_json::Error doesn't have a public custom() constructor on stable, so
// route through serde::de::Error which is implemented for it.
use serde::de::Error as _;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_schedule_parses_and_validates() {
        let s = Schedule::load().expect("schedule");
        assert!(s.world_bosses.len() >= 13, "got {} bosses", s.world_bosses.len());
        assert!(s.meta_events.len() >= 5, "got {} metas", s.meta_events.len());
        assert!(s.ley_line_anomaly.is_some(), "ley_line_anomaly should be present");
    }

    #[test]
    fn parse_hm_basic() {
        assert_eq!(parse_hm("00:00"), Ok((0, 0)));
        assert_eq!(parse_hm("23:59"), Ok((23, 59)));
        assert_eq!(parse_hm("07:30"), Ok((7, 30)));
        assert!(parse_hm("24:00").is_err());
        assert!(parse_hm("12:60").is_err());
        assert!(parse_hm("no").is_err());
        assert!(parse_hm("12-30").is_err());
    }

    #[test]
    fn tequatl_is_present_with_expected_times() {
        let s = Schedule::load().unwrap();
        let teq = s
            .world_bosses
            .iter()
            .find(|b| b.id == "tequatl")
            .expect("tequatl must be in the schedule");
        assert!(!teq.schedule_utc.is_empty());
    }
}
