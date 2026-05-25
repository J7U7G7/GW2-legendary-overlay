import { useEffect, useRef, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";

import { api } from "../lib/tauri";
import { useSettingsStore } from "../store/settings";
import type { HotkeyConfig } from "../types/gw2";

/**
 * Captures a single hotkey combo via a focused input that listens for
 * keydown events and produces the Tauri-accelerator string
 * ("CmdOrCtrl+Shift+G"). Escape cancels. The combo must include at least
 * one modifier (Ctrl/Cmd/Alt/Shift) plus a key — bare letters would clash
 * with normal typing.
 */
function HotkeyCapture({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const [draft, setDraft] = useState<string>(value);
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  useEffect(() => {
    if (!capturing) return;
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(false);
        setDraft(value);
        return;
      }
      const mods: string[] = [];
      if (e.ctrlKey || e.metaKey) mods.push("CmdOrCtrl");
      if (e.altKey) mods.push("Alt");
      if (e.shiftKey) mods.push("Shift");
      // Ignore pure modifier presses — wait until a real key joins.
      const isModifierKey =
        e.key === "Control"
        || e.key === "Meta"
        || e.key === "Alt"
        || e.key === "Shift";
      if (isModifierKey) return;
      if (mods.length === 0) return; // require a modifier
      // Tauri expects a single-character or named key. Uppercase letters.
      const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
      const combo = [...mods, key].join("+");
      setDraft(combo);
      onChange(combo);
      setCapturing(false);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [capturing, value, onChange]);

  return (
    <label className="flex items-center justify-between gap-2">
      <span className="opacity-70 flex-1">{label}</span>
      <div
        ref={ref}
        role="button"
        tabIndex={0}
        onClick={() => setCapturing(true)}
        onKeyDown={(e) => {
          if (e.key === " " || e.key === "Enter") {
            e.preventDefault();
            setCapturing(true);
          }
        }}
        className={
          capturing
            ? "px-2 py-1 rounded font-mono text-[10px] bg-[var(--accent-color)] text-black cursor-pointer min-w-[120px] text-center"
            : "px-2 py-1 rounded font-mono text-[10px] bg-white/10 hover:bg-white/20 cursor-pointer min-w-[120px] text-center"
        }
        title={capturing ? "Press the new combo… Esc to cancel" : "Click to remap"}
      >
        {capturing ? "press keys…" : draft}
      </div>
    </label>
  );
}

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const appearance = useSettingsStore((s) => s.appearance);
  const update = useSettingsStore((s) => s.update);
  const reset = useSettingsStore((s) => s.reset);
  const hotkeys = useSettingsStore((s) => s.hotkeys);
  const setHotkeys = useSettingsStore((s) => s.setHotkeys);
  const resetHotkeys = useSettingsStore((s) => s.resetHotkeys);
  const [notifLead, setNotifLead] = useState<number>(2);
  const [testStatus, setTestStatus] = useState<"" | "sent" | "error">("");

  const updateHotkey = (field: keyof HotkeyConfig, value: string) => {
    void setHotkeys({ ...hotkeys, [field]: value });
  };

  // Two-step reset: first click arms, second within 3 s commits. Avoids the
  // user nuking their data with a single fat-finger.
  const [resetArmed, setResetArmed] = useState(false);
  const [resetStatus, setResetStatus] = useState<"" | "wiping" | "done" | "error">("");
  useEffect(() => {
    if (!resetArmed) return;
    const t = window.setTimeout(() => setResetArmed(false), 3000);
    return () => window.clearTimeout(t);
  }, [resetArmed]);
  const onReset = async () => {
    if (!resetArmed) {
      setResetArmed(true);
      return;
    }
    setResetArmed(false);
    setResetStatus("wiping");
    try {
      await api.resetDatabase();
      setResetStatus("done");
      window.setTimeout(() => setResetStatus(""), 2500);
    } catch (e) {
      console.warn("resetDatabase failed:", e);
      setResetStatus("error");
    }
  };

  useEffect(() => {
    void api.getNotificationLead().then(setNotifLead).catch(() => {
      /* keep default */
    });
  }, []);

  // ---- Diagnostics + feedback (logs + GitHub issue pre-fills) ----
  const [appVersion, setAppVersion] = useState<string>("");
  useEffect(() => {
    void api
      .appVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion(""));
  }, []);
  const [copyStatus, setCopyStatus] = useState<"" | "copied" | "error">("");
  const onCopyLogs = async () => {
    try {
      const txt = await api.recentLogs(200);
      await navigator.clipboard.writeText(txt);
      setCopyStatus("copied");
      window.setTimeout(() => setCopyStatus(""), 2000);
    } catch (e) {
      console.warn("copy logs failed:", e);
      setCopyStatus("error");
    }
  };
  const onOpenLogs = () => {
    void api.openLogsFolder().catch((e) => console.warn("open logs failed:", e));
  };
  const onReportBug = () => {
    const ua = navigator.userAgent;
    const body = [
      `<!-- Auto-filled by the app. Edit freely. -->`,
      ``,
      `**App version:** ${appVersion || "(unknown)"}`,
      `**User agent:** ${ua}`,
      ``,
      `### What happened?`,
      `(describe)`,
      ``,
      `### Recent logs`,
      `In the app: Settings → Diagnostics → Copy last logs → paste here.`,
      ``,
    ].join("\n");
    const url
      = "https://github.com/J7U7G7/GW2-legendary-overlay/issues/new"
        + "?template=bug_report.yml"
        + `&version=${encodeURIComponent(appVersion)}`
        + `&body=${encodeURIComponent(body)}`;
    window.open(url, "_blank", "noopener");
  };
  const onFeatureRequest = () => {
    const url
      = "https://github.com/J7U7G7/GW2-legendary-overlay/issues/new"
        + "?template=feature_request.yml"
        + `&version=${encodeURIComponent(appVersion)}`;
    window.open(url, "_blank", "noopener");
  };

  const onChangeLead = (minutes: number) => {
    setNotifLead(minutes);
    void api.setNotificationLead(minutes).catch((e) => console.warn(e));
  };

  const onTestNotif = async () => {
    setTestStatus("");
    try {
      await api.testNotification();
      setTestStatus("sent");
    } catch (e) {
      console.warn("test notification failed:", e);
      setTestStatus("error");
    }
    window.setTimeout(() => setTestStatus(""), 2500);
  };

  return (
    <div className="flex flex-col h-full overflow-y-auto px-3 py-3 gap-3 text-xs">
      <div className="flex items-center justify-between">
        <h2 className="font-semibold">Appearance</h2>
        <button
          type="button"
          onClick={onClose}
          className="opacity-60 hover:opacity-100 text-xs"
          title="Close settings"
        >
          ✕
        </button>
      </div>

      <label className="flex flex-col gap-1">
        <span className="opacity-70">Background opacity</span>
        <input
          type="range"
          min={0.3}
          max={1}
          step={0.05}
          value={appearance.opacity}
          onChange={(e) => void update({ opacity: Number(e.target.value) })}
        />
        <span className="opacity-50 font-mono">{Math.round(appearance.opacity * 100)}%</span>
      </label>

      <label className="flex items-center justify-between gap-2">
        <span className="opacity-70">Accent color</span>
        <input
          type="color"
          value={appearance.accent_color}
          onChange={(e) => void update({ accent_color: e.target.value })}
          className="h-6 w-12 bg-transparent border-0 rounded cursor-pointer"
        />
      </label>

      <label className="flex items-center justify-between gap-2">
        <span className="opacity-70">Text color</span>
        <input
          type="color"
          value={appearance.text_color}
          onChange={(e) => void update({ text_color: e.target.value })}
          className="h-6 w-12 bg-transparent border-0 rounded cursor-pointer"
        />
      </label>

      <label className="flex items-center justify-between gap-2">
        <span className="opacity-70">Background color</span>
        <input
          type="color"
          value={appearance.background_color}
          onChange={(e) => void update({ background_color: e.target.value })}
          className="h-6 w-12 bg-transparent border-0 rounded cursor-pointer"
        />
      </label>

      <label className="flex flex-col gap-1">
        <span className="opacity-70">Font size ({appearance.font_size}px)</span>
        <input
          type="range"
          min={10}
          max={18}
          step={1}
          value={appearance.font_size}
          onChange={(e) => void update({ font_size: Number(e.target.value) })}
        />
      </label>

      <button
        type="button"
        onClick={() => void reset()}
        className="self-start px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
      >
        Reset to defaults
      </button>

      <div className="border-t border-white/10 pt-3 flex flex-col gap-2">
        <h2 className="font-semibold">Notifications</h2>
        <label className="flex flex-col gap-1">
          <span className="opacity-70">
            Alert when a pinned boss spawns in ({notifLead} min)
          </span>
          <input
            type="range"
            min={1}
            max={15}
            step={1}
            value={notifLead}
            onChange={(e) => onChangeLead(Number(e.target.value))}
          />
        </label>
        <button
          type="button"
          onClick={() => void onTestNotif()}
          className="self-start px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
        >
          Send a test notification
        </button>
        {testStatus === "sent" && (
          <p className="text-[10px] text-[var(--accent-color)]">
            ✓ Sent — check your Windows action center.
          </p>
        )}
        {testStatus === "error" && (
          <p className="text-[10px] text-red-300">
            Could not send. Open the Tauri logs (RUST_LOG=info) to inspect.
          </p>
        )}
      </div>

      <div className="border-t border-white/10 pt-3 flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <h2 className="font-semibold">Hotkeys</h2>
          <button
            type="button"
            onClick={() => void resetHotkeys()}
            className="px-2 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded"
            title="Restore Ctrl+Shift+G/H/B/P"
          >
            Reset
          </button>
        </div>
        <HotkeyCapture
          label="Toggle main overlay"
          value={hotkeys.toggle_visibility}
          onChange={(v) => updateHotkey("toggle_visibility", v)}
        />
        <HotkeyCapture
          label="Toggle click-through"
          value={hotkeys.toggle_clickthrough}
          onChange={(v) => updateHotkey("toggle_clickthrough", v)}
        />
        <HotkeyCapture
          label="Toggle Bosses window"
          value={hotkeys.toggle_bosses}
          onChange={(v) => updateHotkey("toggle_bosses", v)}
        />
        <HotkeyCapture
          label="Toggle Achievements window"
          value={hotkeys.toggle_achievements}
          onChange={(v) => updateHotkey("toggle_achievements", v)}
        />
        <p className="text-[10px] opacity-50 italic">
          Click a combo, press the new keys. Need at least one modifier
          (Ctrl/Alt/Shift). Esc cancels. Re-binds immediately.
        </p>
      </div>

      <div className="border-t border-white/10 pt-3 flex flex-col gap-2">
        <h2 className="font-semibold">Diagnostics &amp; feedback</h2>
        <p className="text-[10px] opacity-70">
          Logs are written daily to your AppData. The 'Copy last logs' button
          grabs the last 200 lines for pasting into a bug report.
        </p>
        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            onClick={onOpenLogs}
            className="px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
          >
            📂 Open logs folder
          </button>
          <button
            type="button"
            onClick={() => void onCopyLogs()}
            className={
              copyStatus === "copied"
                ? "px-2 py-1 text-[10px] bg-[var(--accent-color)] text-black rounded"
                : "px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
            }
          >
            {copyStatus === "copied" ? "✓ Copied" : "📋 Copy last logs"}
          </button>
          <button
            type="button"
            onClick={onReportBug}
            className="px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
          >
            🐛 Report bug
          </button>
          <button
            type="button"
            onClick={onFeatureRequest}
            className="px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
          >
            💡 Feature request
          </button>
        </div>
        {copyStatus === "error" && (
          <p className="text-[10px] text-red-300">
            Could not copy. Use 'Open logs folder' instead.
          </p>
        )}
        {appVersion && (
          <p className="text-[10px] opacity-50 font-mono">
            Version: {appVersion}
          </p>
        )}
      </div>

      <div className="border-t border-white/10 pt-3 flex flex-col gap-2">
        <h2 className="font-semibold">Window layout</h2>
        <p className="text-[10px] opacity-70">
          If a window ends up off-screen, maximized, or otherwise stuck,
          this clears the saved layout and relaunches with the defaults
          from <code>tauri.conf.json</code> (centered, 380×600).
        </p>
        <button
          type="button"
          onClick={async () => {
            try {
              await api.resetWindowLayout();
              await relaunch();
            } catch (e) {
              console.warn("reset layout failed:", e);
            }
          }}
          className="self-start px-2 py-1 text-[10px] bg-white/10 hover:bg-white/20 rounded"
        >
          🔄 Reset window layout &amp; restart
        </button>
      </div>

      <div className="border-t border-red-400/30 pt-3 flex flex-col gap-2">
        <h2 className="font-semibold text-red-300">Danger zone</h2>
        <p className="text-[10px] opacity-70">
          Wipe every cached achievement, item, todo, wallet currency, and
          pinned entry. Keeps your API key + appearance settings. Useful when
          a sync goes sideways or after a spec change. Triggers a fresh
          re-sync on the next periodic tick.
        </p>
        <button
          type="button"
          onClick={() => void onReset()}
          disabled={resetStatus === "wiping"}
          className={
            resetArmed
              ? "self-start px-2 py-1 text-[10px] bg-red-500 text-white rounded"
              : "self-start px-2 py-1 text-[10px] bg-red-400/20 hover:bg-red-400/30 text-red-200 rounded disabled:opacity-40"
          }
        >
          {resetStatus === "wiping"
            ? "Wiping…"
            : resetArmed
              ? "Click again to confirm wipe"
              : "🗑 Reset database"}
        </button>
        {resetStatus === "done" && (
          <p className="text-[10px] text-[var(--accent-color)]">
            ✓ Wiped. Hit Sync (↻) in the header to refill.
          </p>
        )}
        {resetStatus === "error" && (
          <p className="text-[10px] text-red-300">Reset failed; check logs.</p>
        )}
      </div>

      <p className="opacity-50 text-[10px] mt-auto">
        Changes are applied live and persisted on close.
      </p>
    </div>
  );
}
