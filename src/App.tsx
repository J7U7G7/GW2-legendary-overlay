import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { EventsWindow } from "./components/EventsWindow";
import { Overlay } from "./components/Overlay";

export default function App() {
  const [label, setLabel] = useState<string | null>(null);

  useEffect(() => {
    setLabel(getCurrentWindow().label);
  }, []);

  if (label === null) return null;
  if (label === "events") return <EventsWindow />;
  return <Overlay />;
}
