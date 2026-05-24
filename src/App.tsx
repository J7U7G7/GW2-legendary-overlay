import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { AchievementsWindow } from "./components/AchievementsWindow";
import { BossesWindow } from "./components/BossesWindow";
import { Overlay } from "./components/Overlay";

export default function App() {
  const [label, setLabel] = useState<string | null>(null);

  useEffect(() => {
    setLabel(getCurrentWindow().label);
  }, []);

  if (label === null) return null;
  if (label === "bosses") return <BossesWindow />;
  if (label === "achievements") return <AchievementsWindow />;
  return <Overlay />;
}
