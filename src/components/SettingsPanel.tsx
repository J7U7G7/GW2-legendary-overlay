import { useEffect, useState } from "react";

import { api } from "../lib/tauri";
import { useSettingsStore } from "../store/settings";

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const appearance = useSettingsStore((s) => s.appearance);
  const update = useSettingsStore((s) => s.update);
  const reset = useSettingsStore((s) => s.reset);
  const [notifLead, setNotifLead] = useState<number>(2);
  const [testStatus, setTestStatus] = useState<"" | "sent" | "error">("");

  useEffect(() => {
    void api.getNotificationLead().then(setNotifLead).catch(() => {
      /* keep default */
    });
  }, []);

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

      <p className="opacity-50 text-[10px] mt-auto">
        Changes are applied live and persisted on close.
      </p>
    </div>
  );
}
