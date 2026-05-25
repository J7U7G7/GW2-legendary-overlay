import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Phase = "idle" | "downloading" | "installing" | "error";

/**
 * Polls the configured updater endpoint on mount. When an update is
 * available, renders a non-modal banner above the main content with two
 * actions:
 *   - "Install now" → download + install + relaunch.
 *   - "Later"       → dismiss until next launch.
 *
 * Only mounts on the main window (label === "main") — the bosses /
 * achievements windows are passive and shouldn't compete for the
 * notification slot.
 */
export function UpdatePrompt() {
  const [pending, setPending] = useState<Update | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<{ downloaded: number; total: number | null }>({
    downloaded: 0,
    total: null,
  });
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    // Only run on the main window.
    if (getCurrentWindow().label !== "main") return;
    let cancelled = false;
    const run = async () => {
      try {
        const update = await check();
        if (cancelled) return;
        if (update) {
          setPending(update);
        }
      } catch (e) {
        // Network blip, endpoint down, etc. — silent. The user can
        // re-trigger via Settings → Check for updates (future).
        console.warn("update check failed:", e);
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, []);

  const onInstall = async () => {
    if (!pending) return;
    setError(null);
    setPhase("downloading");
    try {
      await pending.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            setProgress({ downloaded: 0, total: event.data.contentLength ?? null });
            break;
          case "Progress":
            setProgress((p) => ({
              downloaded: p.downloaded + event.data.chunkLength,
              total: p.total,
            }));
            break;
          case "Finished":
            setPhase("installing");
            break;
        }
      });
      // installMode "passive" runs the NSIS installer with progress UI;
      // when it returns we ask the OS to relaunch us on the new binary.
      await relaunch();
    } catch (e) {
      console.warn("update install failed:", e);
      setError(String(e));
      setPhase("error");
    }
  };

  if (!pending || dismissed) return null;

  const pct
    = progress.total && progress.total > 0
      ? Math.round((progress.downloaded / progress.total) * 100)
      : null;

  return (
    <div className="border-b border-[var(--accent-color)] bg-[var(--accent-color)]/15 px-3 py-1.5 shrink-0 flex items-center gap-2 text-[11px]">
      <span>⬆️</span>
      <span className="flex-1 truncate">
        <span className="font-semibold">v{pending.version}</span>{" "}
        <span className="opacity-70">available</span>
      </span>
      {phase === "idle" && (
        <>
          <button
            type="button"
            onClick={() => void onInstall()}
            className="px-2 py-0.5 text-[10px] rounded bg-[var(--accent-color)] text-black font-semibold"
          >
            Install now
          </button>
          <button
            type="button"
            onClick={() => setDismissed(true)}
            className="px-2 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20"
          >
            Later
          </button>
        </>
      )}
      {phase === "downloading" && (
        <span className="font-mono text-[10px] opacity-80">
          {pct != null ? `${pct}%` : "downloading…"}
        </span>
      )}
      {phase === "installing" && (
        <span className="font-mono text-[10px] opacity-80">installing…</span>
      )}
      {phase === "error" && (
        <>
          <span className="text-red-300 text-[10px] truncate">{error}</span>
          <button
            type="button"
            onClick={() => setDismissed(true)}
            className="px-2 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20"
          >
            Dismiss
          </button>
        </>
      )}
    </div>
  );
}
