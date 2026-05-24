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
