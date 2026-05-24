import { useEffect, useState } from "react";

import { api } from "../lib/tauri";
import type { TodoView } from "../types/gw2";

type Period = "daily" | "weekly";

function TodoSection({
  period,
  title,
  resetHint,
}: {
  period: Period;
  title: string;
  resetHint: string;
}) {
  const [todos, setTodos] = useState<TodoView[]>([]);
  const [draft, setDraft] = useState("");

  const load = async () => {
    try {
      setTodos(await api.listTodos(period));
    } catch (e) {
      console.warn(`listTodos(${period}) failed:`, e);
    }
  };

  useEffect(() => {
    void load();
    // Re-check periodically so a daily todo flips back to unchecked at
    // 00:00 UTC without a manual refresh.
    const id = window.setInterval(() => void load(), 60_000);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [period]);

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    const text = draft.trim();
    if (!text) return;
    try {
      await api.addTodo(text, period);
      setDraft("");
      void load();
    } catch (err) {
      console.warn("addTodo failed:", err);
    }
  };

  const toggle = async (id: number) => {
    await api.toggleTodo(id);
    void load();
  };

  const del = async (id: number) => {
    await api.deleteTodo(id);
    void load();
  };

  const remaining = todos.filter((t) => !t.completed).length;

  return (
    <section className="border-b border-white/10">
      <header className="px-3 py-1.5 flex items-center justify-between">
        <span className="text-xs font-semibold">{title}</span>
        <span className="text-[10px] opacity-60 font-mono">
          {remaining}/{todos.length} left
        </span>
      </header>
      <p className="px-3 pb-1 text-[10px] opacity-50">{resetHint}</p>
      <ul>
        {todos.map((t) => (
          <li
            key={t.id}
            className="px-3 py-1 flex items-center gap-2 text-xs border-t border-white/5"
          >
            <button
              type="button"
              onClick={() => void toggle(t.id)}
              className={
                t.completed
                  ? "text-[var(--accent-color)]"
                  : "opacity-50 hover:opacity-100"
              }
              title={t.completed ? "Mark not done" : "Mark done"}
            >
              {t.completed ? "✓" : "○"}
            </button>
            <span
              className={`flex-1 ${t.completed ? "opacity-40 line-through" : "opacity-95"}`}
            >
              {t.text}
            </span>
            <button
              type="button"
              onClick={() => void del(t.id)}
              className="opacity-30 hover:opacity-100 text-[10px]"
              title="Delete"
            >
              ✕
            </button>
          </li>
        ))}
      </ul>
      <form onSubmit={(e) => void add(e)} className="px-3 py-1.5 flex items-center gap-2">
        <input
          type="text"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={`Add ${period} todo…`}
          className="flex-1 font-mono text-xs px-2 py-1 bg-white/5 border border-white/10 rounded focus:outline-none focus:border-[var(--accent-color)]"
        />
        <button
          type="submit"
          disabled={draft.trim().length === 0}
          className="px-2 py-0.5 text-[10px] bg-white/10 hover:bg-white/20 rounded disabled:opacity-40"
        >
          +
        </button>
      </form>
    </section>
  );
}

export function TodosView() {
  return (
    <div className="flex-1 overflow-y-auto">
      <TodoSection
        period="daily"
        title="Daily"
        resetHint="Resets at 00:00 UTC every day."
      />
      <TodoSection
        period="weekly"
        title="Weekly"
        resetHint="Resets Mondays at 07:30 UTC."
      />
    </div>
  );
}
