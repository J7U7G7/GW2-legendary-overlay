/**
 * Strip GW2 in-game text formatting tags from API-returned strings.
 * The API often returns descriptions wrapped in `<c=@flavor>...</c>`,
 * `<c=@reminder>...</c>`, `<br>`, etc. — these render literally as text in
 * any non-game client.
 */
export function stripGw2Markup(s: string | null | undefined): string {
  if (!s) return "";
  return s
    .replace(/<c=@\w+>/g, "")
    .replace(/<\/c>/g, "")
    .replace(/<br\s*\/?>/gi, "\n")
    .trim();
}

/**
 * Many GW2 achievements have a literal double-space inside their
 * `requirement` text where the in-game UI inserts the tier count. Example
 * raw API string: `"Win  costume brawls."` Substitute the value with the
 * achievement's current tier target (`max`) to mirror the in-game text.
 */
export function fillRequirement(
  requirement: string | null | undefined,
  max: number | null | undefined,
): string {
  if (!requirement) return "";
  if (max == null) return requirement;
  return requirement.replace(/(\s)\s+/g, `$1${max} `);
}

/**
 * Build a wiki URL for a legendary collection / item. The English wiki
 * accepts plain names with spaces (or underscores).
 */
export function wikiUrl(name: string): string {
  return `https://wiki.guildwars2.com/wiki/${encodeURIComponent(name.replace(/ /g, "_"))}`;
}

function shortDuration(ms: number): string {
  const sec = Math.floor(ms / 1000);
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h}h${String(m).padStart(2, "0")}`;
  return `${m}m`;
}

export type EventStatus = "future" | "active" | "ended";

export type EventTimeLabel = {
  status: EventStatus;
  /** Short human label, e.g. "in 12m", "active · 8m left", "ended". */
  label: string;
  /** Minutes until the event starts (negative if already started). */
  minutesUntilStart: number;
  /** Minutes until the event ends (negative if already ended). */
  minutesUntilEnd: number;
};

export function eventTimeLabel(
  startIso: string,
  durationMinutes: number,
  now: number,
): EventTimeLabel {
  const startMs = new Date(startIso).getTime();
  const endMs = startMs + durationMinutes * 60_000;
  const diffStart = startMs - now;
  const diffEnd = endMs - now;
  const minutesUntilStart = Math.floor(diffStart / 60_000);
  const minutesUntilEnd = Math.floor(diffEnd / 60_000);
  if (diffStart > 0) {
    return {
      status: "future",
      label: `in ${shortDuration(diffStart)}`,
      minutesUntilStart,
      minutesUntilEnd,
    };
  }
  if (diffEnd > 0) {
    return {
      status: "active",
      label: `active · ${shortDuration(diffEnd)} left`,
      minutesUntilStart,
      minutesUntilEnd,
    };
  }
  return {
    status: "ended",
    label: "ended",
    minutesUntilStart,
    minutesUntilEnd,
  };
}
