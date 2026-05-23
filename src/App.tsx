function App() {
  return (
    <main
      className="h-screen w-screen flex flex-col text-[var(--text-color)]"
      style={{
        backgroundColor: 'rgba(0, 0, 0, var(--bg-opacity))',
      }}
    >
      <header
        data-tauri-drag-region
        className="px-3 py-2 text-xs font-semibold opacity-70 border-b border-white/10 cursor-grab"
      >
        GW2 Overlay
      </header>
      <section className="flex-1 overflow-y-auto px-3 py-2 text-xs">
        <p className="opacity-60">Scaffold OK. Implementation à venir.</p>
      </section>
    </main>
  );
}

export default App;
