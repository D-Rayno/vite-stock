// src/hooks/useHotkeys.ts
// Register global keyboard hotkey handlers (F-keys, Ctrl combos, etc.).
// The map is re-evaluated on every render via a stable ref so stale closures
// are never an issue.

import { useEffect, useRef } from "react";

type HotkeyMap = Record<string, (e: KeyboardEvent) => void>;

export function useHotkeys(map: HotkeyMap, enabled = true) {
  const mapRef = useRef<HotkeyMap>(map);
  mapRef.current = map;   // always latest without re-subscribing

  useEffect(() => {
    if (!enabled) return;

    const handler = (e: KeyboardEvent) => {
      // Build canonical combo: [Ctrl+][Shift+][Alt+]Key
      const parts: string[] = [];
      if (e.ctrlKey)  parts.push("Ctrl");
      if (e.shiftKey) parts.push("Shift");
      if (e.altKey)   parts.push("Alt");
      parts.push(e.key);
      const combo = parts.join("+");

      const fn = mapRef.current[combo];
      if (!fn) return;

      // F-keys always intercept regardless of focus
      const isFKey = /^F\d{1,2}$/.test(e.key);
      const tag    = (e.target as HTMLElement).tagName;
      const isText = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";

      if (isFKey || !isText) {
        e.preventDefault();
        fn(e);
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [enabled]);
}