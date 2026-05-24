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
