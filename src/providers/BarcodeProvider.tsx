// src/providers/BarcodeProvider.tsx
//
// Global barcode scanner engine.
//
// How HID barcode scanners work:
//   The scanner presents itself as a USB keyboard to the OS.
//   It "types" the barcode digits in rapid succession (< 30ms/char)
//   then fires a synthetic Enter keystroke.
//
// This provider distinguishes scanner input from human typing by:
//   1. Measuring inter-key intervals.  If consecutive keys arrive within
//      SCANNER_THRESHOLD_MS we treat them as scanner output.
//   2. Requiring at least MIN_BARCODE_LEN characters before Enter.
//   3. Requiring that the *total* input burst takes less than BURST_MAX_MS.
//
// Consumer API:
//   const { scanStatus, lastBarcode } = useBarcodeContext();
//   useBarcodeScanner((barcode) => handleScan(barcode));  // registers a callback
//
// The provider is mounted once at the app root so the listener is always active,
// even when a modal is open.

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import type { ScanStatus } from "@/types";

// ─── Tuning constants ─────────────────────────────────────────────────────────

/** Maximum ms between characters to be considered scanner input */
const SCANNER_THRESHOLD_MS = 50;

/** Minimum barcode length (EAN-8 = 8, EAN-13 = 13; allow 4 for short codes) */
const MIN_BARCODE_LEN = 4;

/** Maximum total duration of a valid scan burst (safety ceiling) */
const BURST_MAX_MS = 500;

/** How long (ms) to keep the "success" / "error" badge visible */
const STATUS_RESET_MS = 2000;

// ─── Context shape ────────────────────────────────────────────────────────────

interface BarcodeContextValue {
  scanStatus:  ScanStatus;
  lastBarcode: string | null;
  /** Register a callback — returned cleanup unregisters it */
  subscribe:   (cb: ScanCallback) => () => void;
}

type ScanCallback = (barcode: string) => void | Promise<void>;

const BarcodeContext = createContext<BarcodeContextValue | null>(null);

// ─── Provider ─────────────────────────────────────────────────────────────────

export function BarcodeProvider({ children }: { children: React.ReactNode }) {
  const [scanStatus,  setScanStatus]  = useState<ScanStatus>("idle");
  const [lastBarcode, setLastBarcode] = useState<string | null>(null);

  // Mutable refs — never trigger re-renders, but survive across keystrokes
  const bufferRef     = useRef<string[]>([]);      // accumulated characters
  const lastKeyTime   = useRef<number>(0);         // timestamp of last keydown
  const burstStart    = useRef<number>(0);         // timestamp of first char in burst
  const statusTimer   = useRef<ReturnType<typeof setTimeout> | null>(null);
  const subscribers   = useRef<Set<ScanCallback>>(new Set());

  // ── Subscriber registration ─────────────────────────────────────────────

  const subscribe = useCallback((cb: ScanCallback) => {
    subscribers.current.add(cb);
    return () => { subscribers.current.delete(cb); };
  }, []);

  // ── Scan dispatch ───────────────────────────────────────────────────────

  const dispatchScan = useCallback((barcode: string) => {
    setLastBarcode(barcode);
    setScanStatus("scanning");

    // Call all registered callbacks
    const callbacks = [...subscribers.current];
    const results   = callbacks.map((cb) => cb(barcode));

    // If any callback returns a Promise, track async completion
    const promises = results.filter((r): r is Promise<void> => r instanceof Promise);
    if (promises.length > 0) {
      Promise.allSettled(promises).then((settled) => {
        const anyFailed = settled.some((r) => r.status === "rejected");
        setScanStatus(anyFailed ? "error" : "success");
        resetStatusAfterDelay();
      });
    } else {
      setScanStatus("success");
      resetStatusAfterDelay();
    }
  }, []);

  const resetStatusAfterDelay = () => {
    if (statusTimer.current) clearTimeout(statusTimer.current);
    statusTimer.current = setTimeout(() => setScanStatus("idle"), STATUS_RESET_MS);
  };

  // ── Global keydown listener ─────────────────────────────────────────────

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const now   = Date.now();
      const delta = now - lastKeyTime.current;

      // ── Enter key: evaluate buffer ──────────────────────────────────────
      if (e.key === "Enter") {
        const code        = bufferRef.current.join("").trim();
        const burstLength = now - burstStart.current;

        bufferRef.current = [];
        lastKeyTime.current = 0;

        if (
          code.length >= MIN_BARCODE_LEN &&
          burstLength <= BURST_MAX_MS
        ) {
          // Prevent Enter from submitting forms / activating focused buttons
          // only if this looks like a genuine scan (not human-typed + Enter)
          e.preventDefault();
          dispatchScan(code);
        }
        return;
      }

      // ── Printable characters only ───────────────────────────────────────
      if (e.key.length !== 1) return;

      // ── Detect if we're inside a text input — skip accumulation ─────────
      // We still want scanners to work inside the POS search field,
      // but we DON'T want to prevent the input from receiving the character.
      // So we accumulate in both cases but only preventDefault on non-inputs.
      const tag = (e.target as HTMLElement).tagName;
      const isTextField = tag === "INPUT" || tag === "TEXTAREA";

      // If too much time has passed since last key, start a fresh burst
      if (delta > SCANNER_THRESHOLD_MS * 3 && bufferRef.current.length > 0) {
        // Slow typing — discard unless buffer is large (already mid-scan)
        if (bufferRef.current.length < MIN_BARCODE_LEN) {
          bufferRef.current = [];
        }
      }

      if (bufferRef.current.length === 0) {
        burstStart.current = now;   // record burst start
      }

      bufferRef.current.push(e.key);
      lastKeyTime.current = now;

      // For fast scanner input: prevent bubbling to focused elements
      // so we don't accidentally type into an unrelated field.
      if (!isTextField && delta <= SCANNER_THRESHOLD_MS) {
        e.preventDefault();
      }
    };

    window.addEventListener("keydown", onKeyDown, { capture: true });
    return () => window.removeEventListener("keydown", onKeyDown, { capture: true });
  }, [dispatchScan]);

  return (
    <BarcodeContext.Provider value={{ scanStatus, lastBarcode, subscribe }}>
      {children}
    </BarcodeContext.Provider>
  );
}

// ─── Consumer hooks ───────────────────────────────────────────────────────────

/** Access scanner status and last scanned barcode. */
export function useBarcodeContext(): BarcodeContextValue {
  const ctx = useContext(BarcodeContext);
  if (!ctx) throw new Error("useBarcodeContext must be used inside BarcodeProvider");
  return ctx;
}

/**
 * Register a scan callback.  Automatically unregisters on unmount.
 *
 * @param callback  Called with the decoded barcode string after each valid scan.
 * @param enabled   Optionally disable scanning (e.g. when a modal is open).
 */
export function useBarcodeScanner(
  callback: ScanCallback,
  enabled = true,
): void {
  const { subscribe } = useBarcodeContext();
  const cbRef = useRef<ScanCallback>(callback);
  cbRef.current = callback;    // always latest without re-subscribing

  useEffect(() => {
    if (!enabled) return;
    // Wrap so the ref stays stable
    const wrapper: ScanCallback = (bc) => cbRef.current(bc);
    return subscribe(wrapper);
  }, [subscribe, enabled]);
}