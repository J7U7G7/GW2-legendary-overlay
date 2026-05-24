//! Static catalog of meta builds, embedded at compile time from
//! `src-tauri/data/builds.json`. No DB tables involved — this is
//! read-only curated data shipped with the binary.

use serde::{Deserialize, Serialize};

use crate::error::Result;

const EMBEDDED_BUILDS: &str = include_str!("../data/builds.json");

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Build {
    pub id: String,
    pub profession: String,
    #[serde(default)]
    pub elite_spec: Option<String>,
    pub role: String,
    pub name: String,
    pub source: String,
    pub source_url: String,
    pub chat_code: String,
    #[serde(default)]
    pub gear_summary: Option<String>,
    #[serde(default)]
    pub weapons: Option<String>,
    #[serde(default)]
    pub difficulty: Option<u8>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    builds: Vec<Build>,
}

pub fn load_all() -> Result<Vec<Build>> {
    let cat: Catalog = serde_json::from_str(EMBEDDED_BUILDS)?;
    Ok(cat.builds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_builds_parse() {
        let builds = load_all().expect("builds.json should parse");
        assert!(!builds.is_empty());
        for b in &builds {
            assert!(b.chat_code.starts_with("[&"), "{} chat code not in [&...] format", b.id);
        }
    }
}
