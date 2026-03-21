// src/hooks/useStartupChecks.ts
// Runs startup checks once on app mount:
//   1. Calls cmd_run_startup_checks → Rust fires native Toast notifications
//   2. Stores the results in local state for the dashboard banner
//   3. Schedules a daily re-check at 06:00 local time

import { useEffect, useState } from "react";
import { invoke }              from "@tauri-apps/api/core";

export interface ExpiryAlert {
  batch_id:          number;
  product_id:        number;
  product_name:      string;
  quantity:          number;
  expiry_date:       string;
  days_until_expiry: number;
  status:            "expired" | "critical" | "warning";
}

export interface LowStockAlert {
  product_id:   number;
  product_name: string;
  total_stock:  number;
  min_stock:    number;
  unit_label:   string | null;
}

export interface StartupCheckResult {
  expiry_alerts:    ExpiryAlert[];
  low_stock_alerts: LowStockAlert[];
  expired_count:    number;
  critical_count:   number;
  low_stock_count:  number;
}

export function useStartupChecks(warnDays = 30) {
  const [result,  setResult]  = useState<StartupCheckResult | null>(null);
  const [loading, setLoading] = useState(true);

  const runChecks = async () => {
    setLoading(true);
    try {
      const r = await invoke<StartupCheckResult>(
        "cmd_run_startup_checks",
        { warnDays },
      );
      setResult(r);
    } catch {
      // Startup checks are non-critical — silently fail
      setResult(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    runChecks();

    // Schedule daily re-check at next 06:00 local time
    const now      = new Date();
    const next6am  = new Date(now);
    next6am.setHours(6, 0, 0, 0);
    if (next6am <= now) next6am.setDate(next6am.getDate() + 1);
    const msUntil  = next6am.getTime() - now.getTime();

    const timer = setTimeout(() => {
      runChecks();
      // After first daily trigger, run every 24h
      const daily = setInterval(runChecks, 24 * 60 * 60 * 1000);
      return () => clearInterval(daily);
    }, msUntil);

    return () => clearTimeout(timer);
  }, [warnDays]);

  return { result, loading, refresh: runChecks };
}