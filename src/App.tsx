// src/App.tsx
// App bootstrap:
//  1. MantineProvider (theme + CSS variables) wraps everything.
//  2. Notifications portal is mounted once.
//  3. BarcodeProvider mounts the global keydown listener once.
//  4. On mount: load license state + settings from Rust.
//  5. Router renders the shell + lazy pages.

import { useEffect, useState, lazy, Suspense } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { LicenseGuard }      from "@/components/LicenseGuard";

import {
  MantineProvider,
  createTheme,
  LoadingOverlay,
  Box,
} from "@mantine/core";
import { Notifications } from "@mantine/notifications";
import { ModalsProvider } from "@mantine/modals";

import { BarcodeProvider } from "@/providers/BarcodeProvider";
import { useLicenseStore } from "@/store/licenseStore";
import { useSettingsStore } from "@/store/settingsStore";
import { useStartupChecks } from "@/hooks/useStartupChecks";
import { AppShell } from "@/layouts/AppShell";
import { applyDirection } from "@/i18n";
import * as cmd from "@/lib/commands";

// ─── Mantine theme ────────────────────────────────────────────────────────────

const theme = createTheme({
  primaryColor: "blue",
  fontFamily: '"IBM Plex Sans", "Segoe UI", system-ui, sans-serif',
  fontFamilyMonospace: '"JetBrains Mono", "Consolas", monospace',
  defaultRadius: "md",
  components: {
    Button:      { defaultProps: { radius: "md" } },
    Paper:       { defaultProps: { radius: "md" } },
    Modal:       { defaultProps: { radius: "lg" } },
    TextInput:   { defaultProps: { radius: "md" } },
    NumberInput: { defaultProps: { radius: "md" } },
    Select:      { defaultProps: { radius: "md" } },
  },
});

// ─── Lazy pages ───────────────────────────────────────────────────────────────

const POSPage       = lazy(() => import("@/pages/POS"));
const LicensePage   = lazy(() => import("@/pages/License"));
const ProductsPage  = lazy(() => import("@/pages/Products"));
const InventoryPage = lazy(() => import("@/pages/Inventory"));
const DainPage      = lazy(() => import("@/pages/Dain"));
const ReportsPage   = lazy(() => import("@/pages/Reports"));
const SettingsPage  = lazy(() => import("@/pages/Settings"));

// ─── Page loading fallback ────────────────────────────────────────────────────

function PageLoader() {
  return (
    <Box pos="relative" h="100%">
      <LoadingOverlay
        visible
        loaderProps={{ type: "dots", size: "lg" }}
        overlayProps={{ blur: 1 }}
      />
    </Box>
  );
}

// ─── App bootstrap ────────────────────────────────────────────────────────────

function AppBootstrap({ children }: { children: React.ReactNode }) {
  const { setLicense } = useLicenseStore();
  const { setSettings } = useSettingsStore();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    (async () => {
      // 1. Load license from Rust (validates against disk file + local HWID).
      try {
        const lic = await cmd.getLicenseState();
        setLicense(lic);
      } catch {
        // No license yet — app loads with locked premium features.
      }

      // 2. Load shop settings.
      //    Rust returns AppSettings (a serde newtype over HashMap<String,String>).
      //    Serde serialises newtypes as the inner value, so the JSON is a plain
      //    flat object: { "shop_name_fr": "...", "thermal_width": "80", ... }.
      try {
        const settings = await cmd.getSettings();
        if (settings) {
          setSettings(settings);
          const lang = settings.default_language ?? "fr";
          applyDirection(lang);
        }
      } catch {
        // Use defaults from settingsStore.
      }

      setReady(true);
    })();
  }, [setLicense, setSettings]);

  // Run startup checks (expiry + low stock) once data is loaded.
  useStartupChecks(30);

  if (!ready) return <PageLoader />;
  return <>{children}</>;
}

// ─── Root ─────────────────────────────────────────────────────────────────────

export default function App() {
  return (
    <MantineProvider theme={theme} defaultColorScheme="light">
      <Notifications position="top-right" zIndex={9999} autoClose={4000} />

      <ModalsProvider>
        <BarcodeProvider>
          <BrowserRouter>
            <AppBootstrap>
              <LicenseGuard>
                <Routes>
                  <Route element={<AppShell />}>
                    <Route path="/"          element={<Suspense fallback={<PageLoader />}><POSPage /></Suspense>} />
                    <Route path="/products"  element={<Suspense fallback={<PageLoader />}><ProductsPage /></Suspense>} />
                    <Route path="/inventory" element={<Suspense fallback={<PageLoader />}><InventoryPage /></Suspense>} />
                    <Route path="/dain"      element={<Suspense fallback={<PageLoader />}><DainPage /></Suspense>} />
                    <Route path="/reports"   element={<Suspense fallback={<PageLoader />}><ReportsPage /></Suspense>} />
                    <Route path="/settings"  element={<Suspense fallback={<PageLoader />}><SettingsPage /></Suspense>} />
                    <Route path="/license"   element={<Suspense fallback={<PageLoader />}><LicensePage /></Suspense>} />
                    <Route path="*"          element={<Navigate to="/" replace />} />
                  </Route>
                </Routes>
              </LicenseGuard>
            </AppBootstrap>
          </BrowserRouter>
        </BarcodeProvider>
      </ModalsProvider>
    </MantineProvider>
  );
}