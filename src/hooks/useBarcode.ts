// src/hooks/useBarcode.ts
// Detects barcode scanner input: a sequence of characters arriving within
// SCAN_THRESHOLD_MS followed by an Enter keypress.
//
// Usage:
//   useBarcode((code) => { handleScan(code); });

import { useEffect, useRef } from "react";

const SCAN_THRESHOLD_MS = 50;   // chars faster than this = scanner
const MIN_BARCODE_LEN   = 4;    // ignore very short bursts

export function useBarcode(
  onScan: (barcode: string) => void,
  enabled = true,
) {
  const bufRef       = useRef<string[]>([]);
  const lastKeyTime  = useRef<number>(0);

  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if focus is on an input / textarea / select
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      const now = Date.now();
      const delta = now - lastKeyTime.current;
      lastKeyTime.current = now;

      if (e.key === "Enter") {
        const code = bufRef.current.join("").trim();
        bufRef.current = [];
        if (code.length >= MIN_BARCODE_LEN) {
          onScan(code);
        }
        return;
      }

      if (e.key.length === 1) {
        if (delta > SCAN_THRESHOLD_MS * 3 && bufRef.current.length > 0) {
          // Too slow — looks like manual typing; discard previous buffer
          // but don't discard if we've already collected something significant
          if (bufRef.current.length < MIN_BARCODE_LEN) {
            bufRef.current = [];
          }
        }
        bufRef.current.push(e.key);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onScan, enabled]);
}