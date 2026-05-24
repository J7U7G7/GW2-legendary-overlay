import { useCallback, useEffect, useRef, useState } from "react";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";

const COLLAPSED_HEIGHT = 32;

/**
 * Collapse the current window to a thin header-only strip and back. We
 * remember the user's chosen height before collapsing so a subsequent
 * expand restores it exactly. Width is left alone.
 */
export function useCollapse(): {
  collapsed: boolean;
  toggle: () => void;
} {
  const [collapsed, setCollapsed] = useState(false);
  const lastHeight = useRef<number | null>(null);

  const toggle = useCallback(() => {
    void (async () => {
      const w = getCurrentWindow();
      const current = await w.innerSize();
      if (!collapsed) {
        lastHeight.current = current.height;
        await w.setSize(new LogicalSize(current.width, COLLAPSED_HEIGHT));
        setCollapsed(true);
      } else {
        const target = lastHeight.current ?? 500;
        await w.setSize(new LogicalSize(current.width, target));
        setCollapsed(false);
      }
    })();
  }, [collapsed]);

  // If the user manually resizes a collapsed window above the strip
  // height, treat that as an expand.
  useEffect(() => {
    const w = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    void w
      .onResized(({ payload }) => {
        if (collapsed && payload.height > COLLAPSED_HEIGHT * 2) {
          setCollapsed(false);
          lastHeight.current = payload.height;
        }
      })
      .then((u) => {
        unlisten = u;
      });
    return () => {
      if (unlisten) unlisten();
    };
  }, [collapsed]);

  return { collapsed, toggle };
}
