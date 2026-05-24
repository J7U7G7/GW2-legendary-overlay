"""One-shot helper: replace the `meta_events` array in boss_schedule.json
with the wiki-verified set produced by the meta-event audit agent.
Run with `python scripts/update_meta_events.py` from the repo root.
"""

import json
from pathlib import Path

NEW_META_EVENTS = [
    {
        "id": "dragons_stand",
        "name": "Dragon's Stand",
        "expansion": "HoT",
        "map": "Dragon's Stand",
        "waypoint_code": "[&BBAIAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:30",
        "phases": [
            {"offset_minutes": 0, "name": "Advancing on the Blighting Towers", "duration_minutes": 120}
        ],
    },
    {
        "id": "verdant_brink",
        "name": "Night and the Enemy",
        "expansion": "HoT",
        "map": "Verdant Brink",
        "waypoint_code": "[&BAgIAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:30",
        "phases": [
            {"offset_minutes": 0,   "name": "Day: Securing Verdant Brink", "duration_minutes": 75},
            {"offset_minutes": 75,  "name": "Night: Night and the Enemy",  "duration_minutes": 25},
            {"offset_minutes": 100, "name": "Night Bosses",                "duration_minutes": 20},
        ],
    },
    {
        "id": "auric_basin",
        "name": "Battle in Tarir",
        "expansion": "HoT",
        "map": "Auric Basin",
        "waypoint_code": "[&BAIIAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:30",
        "phases": [
            {"offset_minutes": 0,   "name": "Pylons",     "duration_minutes": 75},
            {"offset_minutes": 75,  "name": "Challenges", "duration_minutes": 15},
            {"offset_minutes": 90,  "name": "Octovine",   "duration_minutes": 20},
            {"offset_minutes": 110, "name": "Reset",      "duration_minutes": 10},
        ],
    },
    {
        "id": "tangled_depths",
        "name": "Chak Gerent",
        "expansion": "HoT",
        "map": "Tangled Depths",
        "waypoint_code": "[&BPUHAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:50",
        "phases": [
            {"offset_minutes": 0,   "name": "Help the Outposts", "duration_minutes": 95},
            {"offset_minutes": 95,  "name": "Prep",              "duration_minutes": 5},
            {"offset_minutes": 100, "name": "Chak Gerent",       "duration_minutes": 20},
        ],
    },
    {
        "id": "crystal_oasis",
        "name": "Casino Blitz",
        "expansion": "PoF",
        "map": "Crystal Oasis",
        "waypoint_code": "[&BLsKAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:30",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",          "duration_minutes": 95},
            {"offset_minutes": 95,  "name": "Rounds 1 to 3", "duration_minutes": 16},
            {"offset_minutes": 111, "name": "Pinata/Reset",  "duration_minutes": 9},
        ],
    },
    {
        "id": "desert_highlands",
        "name": "Buried Treasure",
        "expansion": "PoF",
        "map": "Desert Highlands",
        "waypoint_code": "[&BGsKAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:20",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",            "duration_minutes": 100},
            {"offset_minutes": 100, "name": "Buried Treasure", "duration_minutes": 20},
        ],
    },
    {
        "id": "elon_riverlands",
        "name": "The Path to Ascension: Augury Rock",
        "expansion": "PoF",
        "map": "Elon Riverlands",
        "waypoint_code": "[&BFMKAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:15",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",         "duration_minutes": 75},
            {"offset_minutes": 75,  "name": "Augury Rock",  "duration_minutes": 25},
            {"offset_minutes": 100, "name": "Doppelganger", "duration_minutes": 20},
        ],
    },
    {
        "id": "desolation",
        "name": "Junundu Rising / Maws of Torment",
        "expansion": "PoF",
        "map": "The Desolation",
        "waypoint_code": "[&BMEKAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:00",
        "phases": [
            {"offset_minutes": 0,   "name": "Maws of Torment", "duration_minutes": 20},
            {"offset_minutes": 20,  "name": "Idle",            "duration_minutes": 10},
            {"offset_minutes": 30,  "name": "Junundu Rising",  "duration_minutes": 20},
            {"offset_minutes": 50,  "name": "Idle",            "duration_minutes": 40},
            {"offset_minutes": 90,  "name": "Junundu Rising",  "duration_minutes": 20},
            {"offset_minutes": 110, "name": "Idle",            "duration_minutes": 10},
        ],
    },
    {
        "id": "domain_of_vabbi",
        "name": "Forged with Fire / Serpents' Ire",
        "expansion": "PoF",
        "map": "Domain of Vabbi",
        "waypoint_code": "[&BHQKAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:30",
        "phases": [
            {"offset_minutes": 0,   "name": "Serpents' Ire",    "duration_minutes": 30},
            {"offset_minutes": 30,  "name": "Forged with Fire", "duration_minutes": 30},
            {"offset_minutes": 60,  "name": "Idle",             "duration_minutes": 30},
            {"offset_minutes": 90,  "name": "Forged with Fire", "duration_minutes": 30},
        ],
    },
    {
        "id": "domain_of_istan",
        "name": "Palawadan, Jewel of Istan",
        "expansion": "PoF",
        "map": "Domain of Istan",
        "waypoint_code": "[&BAkLAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:15",
        "phases": [
            {"offset_minutes": 0,  "name": "Idle",      "duration_minutes": 90},
            {"offset_minutes": 90, "name": "Palawadan", "duration_minutes": 30},
        ],
    },
    {
        "id": "jahai_bluffs",
        "name": "Death-Branded Shatterer",
        "expansion": "PoF",
        "map": "Jahai Bluffs",
        "waypoint_code": "[&BJMLAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:30",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",                    "duration_minutes": 90},
            {"offset_minutes": 90,  "name": "Escorts",                 "duration_minutes": 15},
            {"offset_minutes": 105, "name": "Death-Branded Shatterer", "duration_minutes": 15},
        ],
    },
    {
        "id": "thunderhead_peaks",
        "name": "Thunderhead Keep",
        "expansion": "PoF",
        "map": "Thunderhead Peaks",
        "waypoint_code": "[&BLsLAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:00",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",             "duration_minutes": 45},
            {"offset_minutes": 45,  "name": "Thunderhead Keep", "duration_minutes": 20},
            {"offset_minutes": 65,  "name": "Idle",             "duration_minutes": 40},
            {"offset_minutes": 105, "name": "The Oil Floes",    "duration_minutes": 15},
        ],
    },
    {
        "id": "grothmar_valley",
        "name": "Ceremony of the Sacred Flame (and more)",
        "expansion": "IBS",
        "map": "Grothmar Valley",
        "waypoint_code": "[&BA4MAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:10",
        "phases": [
            {"offset_minutes": 0,   "name": "Effigy",          "duration_minutes": 15},
            {"offset_minutes": 15,  "name": "Idle",            "duration_minutes": 13},
            {"offset_minutes": 28,  "name": "Doomlore Shrine", "duration_minutes": 22},
            {"offset_minutes": 50,  "name": "Idle",            "duration_minutes": 5},
            {"offset_minutes": 55,  "name": "Ooze Pits",       "duration_minutes": 20},
            {"offset_minutes": 75,  "name": "Idle",            "duration_minutes": 15},
            {"offset_minutes": 90,  "name": "Metal Concert",   "duration_minutes": 15},
            {"offset_minutes": 105, "name": "Idle",            "duration_minutes": 15},
        ],
    },
    {
        "id": "bjora_marches",
        "name": "Drakkar / Storms of Winter",
        "expansion": "IBS",
        "map": "Bjora Marches",
        "waypoint_code": "[&BDkMAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:20",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",                           "duration_minutes": 45},
            {"offset_minutes": 45,  "name": "Drakkar and Spirits of the Wild","duration_minutes": 35},
            {"offset_minutes": 80,  "name": "Idle",                           "duration_minutes": 5},
            {"offset_minutes": 85,  "name": "Defend Jora's Keep",             "duration_minutes": 15},
            {"offset_minutes": 100, "name": "Shards and Construct",           "duration_minutes": 5},
            {"offset_minutes": 105, "name": "Icebrood Champions",             "duration_minutes": 15},
        ],
    },
    {
        "id": "seitung_province",
        "name": "Aetherblade Assault",
        "expansion": "EoD",
        "map": "Seitung Province",
        "waypoint_code": "[&BGUNAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:30",
        "phases": [
            {"offset_minutes": 0,  "name": "Aetherblade Assault", "duration_minutes": 30},
            {"offset_minutes": 30, "name": "Idle",                "duration_minutes": 90},
        ],
    },
    {
        "id": "new_kaineng",
        "name": "Kaineng Blackout",
        "expansion": "EoD",
        "map": "New Kaineng City",
        "waypoint_code": "[&BBkNAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:00",
        "phases": [
            {"offset_minutes": 0,  "name": "Kaineng Blackout", "duration_minutes": 40},
            {"offset_minutes": 40, "name": "Idle",             "duration_minutes": 80},
        ],
    },
    {
        "id": "echovald_wilds",
        "name": "Gang War / Aspenwood",
        "expansion": "EoD",
        "map": "The Echovald Wilds",
        "waypoint_code": "[&BMwMAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:00",
        "phases": [
            {"offset_minutes": 0,   "name": "Idle",      "duration_minutes": 30},
            {"offset_minutes": 30,  "name": "Gang War",  "duration_minutes": 35},
            {"offset_minutes": 65,  "name": "Idle",      "duration_minutes": 35},
            {"offset_minutes": 100, "name": "Aspenwood", "duration_minutes": 20},
        ],
    },
    {
        "id": "dragons_end",
        "name": "The Battle for the Jade Sea",
        "expansion": "EoD",
        "map": "Dragon's End",
        "waypoint_code": "[&BKIMAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:00",
        "phases": [
            {"offset_minutes": 0,  "name": "Preparations",               "duration_minutes": 5},
            {"offset_minutes": 5,  "name": "Jade Maw",                   "duration_minutes": 8},
            {"offset_minutes": 13, "name": "Preparations",               "duration_minutes": 32},
            {"offset_minutes": 45, "name": "Jade Maw",                   "duration_minutes": 8},
            {"offset_minutes": 53, "name": "Preparations",               "duration_minutes": 7},
            {"offset_minutes": 60, "name": "The Battle for the Jade Sea","duration_minutes": 60},
        ],
    },
    {
        "id": "skywatch_archipelago",
        "name": "Unlocking the Wizard's Tower",
        "expansion": "SotO",
        "map": "Skywatch Archipelago",
        "waypoint_code": "[&BL4NAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:00",
        "phases": [
            {"offset_minutes": 0,  "name": "Unlocking the Wizard's Tower", "duration_minutes": 25},
            {"offset_minutes": 25, "name": "Idle",                         "duration_minutes": 95},
        ],
    },
    {
        "id": "wizards_tower",
        "name": "Skyscale Target Practice / Fly by Night",
        "expansion": "SotO",
        "map": "Wizard's Tower",
        "waypoint_code": "[&BB8OAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:00",
        "phases": [
            {"offset_minutes": 0,  "name": "Target Practice",                "duration_minutes": 40},
            {"offset_minutes": 40, "name": "Target Practice + Fly by Night", "duration_minutes": 15},
            {"offset_minutes": 55, "name": "Fly by Night",                   "duration_minutes": 25},
            {"offset_minutes": 80, "name": "Idle",                           "duration_minutes": 40},
        ],
    },
    {
        "id": "amnytas",
        "name": "Defense of Amnytas",
        "expansion": "SotO",
        "map": "Amnytas",
        "waypoint_code": "[&BDQOAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:00",
        "phases": [
            {"offset_minutes": 0,  "name": "Defense of Amnytas", "duration_minutes": 25},
            {"offset_minutes": 25, "name": "Idle",               "duration_minutes": 95},
        ],
    },
    {
        "id": "janthir_syntri",
        "name": "Of Mists and Monsters",
        "expansion": "JW",
        "map": "Janthir Syntri",
        "waypoint_code": "[&BCoPAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "00:30",
        "phases": [
            {"offset_minutes": 0,  "name": "Of Mists and Monsters", "duration_minutes": 25},
            {"offset_minutes": 25, "name": "Idle",                  "duration_minutes": 95},
        ],
    },
    {
        "id": "bava_nisos",
        "name": "A Titanic Voyage",
        "expansion": "JW",
        "map": "Bava Nisos",
        "waypoint_code": "[&BGEPAAA=]",
        "cycle_minutes": 120,
        "anchor_utc": "01:20",
        "phases": [
            {"offset_minutes": 0,  "name": "A Titanic Voyage", "duration_minutes": 25},
            {"offset_minutes": 25, "name": "Idle",             "duration_minutes": 95},
        ],
    },
]


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    path = root / "src-tauri" / "data" / "boss_schedule.json"
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    data["meta_events"] = NEW_META_EVENTS
    # Note: lowland_shore is intentionally dropped — the wiki documents it
    # as player-driven (no scheduled timer). Same for Inner Nayos and
    # Mistburned Barrens. Convergences (weekly) are out of scope for the
    # per-map meta feed.
    data.setdefault("_meta", {})["last_verified"] = "2026-05-24"
    with path.open("w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
    print(f"Wrote {len(NEW_META_EVENTS)} meta_events entries to {path}")


if __name__ == "__main__":
    main()
