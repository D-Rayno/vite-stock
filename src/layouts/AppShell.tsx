// src/layouts/AppShell.tsx
import { useState } from "react";
import { NavLink as RouterNavLink, Outlet, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  AppShell as MantineShell,
  NavLink,
  Text,
  Group,
  Stack,
  Tooltip,
//   ActionIcon,
//   Badge,
//   Divider,
} from "@mantine/core";
import {
  IconShoppingCart,
  IconPackage,
  IconTag,
  IconBook,
  IconChartBar,
  IconSettings,
  IconKey,
  IconLanguage,
  IconChevronLeft,
  IconChevronRight,
} from "@tabler/icons-react";
import { useLicenseStore } from "@/store/licenseStore";
import { Features } from "@/types";
import { applyDirection } from "@/i18n";
import i18n from "@/i18n";

const NAV_ITEMS = [
  { path: "/",          labelKey: "nav.pos",        icon: <IconShoppingCart size={18} />,  flag: null },
  { path: "/inventory", labelKey: "nav.inventory",   icon: <IconPackage size={18} />,       flag: Features.INVENTORY_MGMT },
  { path: "/products",  labelKey: "nav.products",    icon: <IconTag size={18} />,           flag: null },
  { path: "/dain",      labelKey: "nav.dain",        icon: <IconBook size={18} />,          flag: Features.DAIN_LEDGER },
  { path: "/reports",   labelKey: "nav.reports",     icon: <IconChartBar size={18} />,      flag: null },
  { path: "/settings",  labelKey: "nav.settings",    icon: <IconSettings size={18} />,      flag: null },
] as const;

export function AppShell() {
  const { t } = useTranslation();
  const { pathname } = useLocation();
  const can          = useLicenseStore((s) => s.can);
  const license      = useLicenseStore((s) => s.license);
  const [collapsed,  setCollapsed] = useState(false);

  const navWidth = collapsed ? 60 : 220;

  const toggleLang = () => {
    const next = i18n.language === "fr" ? "ar" : "fr";
    i18n.changeLanguage(next);
    applyDirection(next);
  };

  return (
    <MantineShell
      navbar={{ width: navWidth, breakpoint: "sm", collapsed: { mobile: collapsed } }}
      style={{ height: "100vh" }}
    >
      {/* ── Navbar ────────────────────────────────────────────────────── */}
      <MantineShell.Navbar
        style={{
          background: "var(--mantine-color-dark-8)",
          border: "none",
          transition: "width 200ms ease",
          overflow: "hidden",
        }}
      >
        {/* Logo */}
        <Group
          px={collapsed ? "xs" : "md"}
          py="md"
          style={{ borderBottom: "1px solid var(--mantine-color-dark-6)", flexShrink: 0 }}
          justify={collapsed ? "center" : "flex-start"}
        >
          <Text size="xl">🛒</Text>
          {!collapsed && (
            <Text fw={800} c="white" size="md" style={{ letterSpacing: "0.03em" }}>
              SuperPOS
            </Text>
          )}
        </Group>

        {/* Nav items */}
        <Stack gap={2} p="xs" style={{ flex: 1, overflowY: "auto" }}>
          {NAV_ITEMS.map(({ path, labelKey, icon, flag }) => {
            const locked    = flag !== null && !can(flag);
            const isActive  = path === "/" ? pathname === "/" : pathname.startsWith(path);

            const item = (
              <NavLink
                key={path}
                component={RouterNavLink}
                to={locked ? "#" : path}
                label={collapsed ? undefined : t(labelKey)}
                leftSection={icon}
                active={isActive}
                disabled={locked}
                rightSection={
                  locked && !collapsed
                    ? <Text size="xs" c="dimmed">🔒</Text>
                    : undefined
                }
                styles={{
                  root: {
                    borderRadius: 8,
                    color: "var(--mantine-color-gray-4)",
                    "&[dataActive]": {
                      background: "var(--mantine-color-blue-8)",
                      color: "white",
                    },
                    "&:hover:not([dataDisabled])": {
                      background: "var(--mantine-color-dark-6)",
                      color: "white",
                    },
                    opacity: locked ? 0.4 : 1,
                    padding: collapsed ? "10px" : undefined,
                    justifyContent: collapsed ? "center" : undefined,
                  },
                  label: { color: "inherit" },
                }}
              />
            );

            return collapsed
              ? <Tooltip key={path} label={t(labelKey)} position="right" withArrow>{item}</Tooltip>
              : item;
          })}
        </Stack>

        {/* Footer */}
        <Stack gap={4} px="xs" py="sm" style={{ borderTop: "1px solid var(--mantine-color-dark-6)", flexShrink: 0 }}>
          {/* License status */}
          {!collapsed && (
            <NavLink
              component={RouterNavLink}
              to="/license"
              label={
                <Group gap="xs">
                  <Text size="xs" c={license.is_valid ? "green.4" : "orange.4"}>
                    {license.is_valid ? "✅" : "⚠️"}
                  </Text>
                  <Text size="xs" c="gray.4" tt="capitalize">
                    {license.tier || "Non licencié"}
                  </Text>
                </Group>
              }
              leftSection={<IconKey size={14} />}
              styles={{ root: { borderRadius: 8, color: "var(--mantine-color-gray-5)" } }}
            />
          )}

          {/* Language toggle */}
          <Tooltip label={i18n.language === "fr" ? "Switch to العربية" : "Passer en Français"} position="right">
            <NavLink
              label={collapsed ? undefined : (i18n.language === "fr" ? "العربية" : "Français")}
              leftSection={<IconLanguage size={14} />}
              onClick={toggleLang}
              styles={{ root: { borderRadius: 8, color: "var(--mantine-color-gray-5)", cursor: "pointer" } }}
            />
          </Tooltip>

          {/* Collapse toggle */}
          <Tooltip label={collapsed ? "Agrandir" : "Réduire"} position="right">
            <NavLink
              label={collapsed ? undefined : "Réduire"}
              leftSection={collapsed
                ? <IconChevronRight size={14} />
                : <IconChevronLeft size={14} />}
              onClick={() => setCollapsed((v) => !v)}
              styles={{ root: { borderRadius: 8, color: "var(--mantine-color-dark-3)", cursor: "pointer" } }}
            />
          </Tooltip>
        </Stack>
      </MantineShell.Navbar>

      {/* ── Main content ─────────────────────────────────────────────── */}
      <MantineShell.Main style={{ padding: 0, height: "100vh", overflow: "auto" }}>
        <Outlet />
      </MantineShell.Main>
    </MantineShell>
  );
}