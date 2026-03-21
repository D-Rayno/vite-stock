// src/pages/License/index.tsx
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Stack, Group, Title, Text, Paper, Badge, Textarea,
  Button, CopyButton, Tooltip, ActionIcon, Code,
  Divider, SimpleGrid, ThemeIcon, Alert, Box,
} from "@mantine/core";
import {
  IconCheck, IconCopy, IconKey, IconShieldCheck,
  IconShieldOff, IconShieldLock, IconCpu, IconRefresh,
  IconAlertCircle,
} from "@tabler/icons-react";
import { notifications }   from "@mantine/notifications";
import { useLicenseStore } from "@/store/licenseStore";
import { Features }        from "@/types";
import * as cmd            from "@/lib/commands";

// ─── Feature flag registry ────────────────────────────────────────────────────

const FLAG_LABELS: [number, string][] = [
  [Features.POS_BASIC,          "Caisse (POS)"],
  [Features.INVENTORY_MGMT,     "Gestion des stocks"],
  [Features.THERMAL_PRINT,      "Impression thermique"],
  [Features.DAIN_LEDGER,        "Dain — Crédit client"],
  [Features.A4_REPORTS,         "Rapports A4"],
  [Features.MULTI_CART,         "Multi-caisse (Attente)"],
  [Features.ADVANCED_ANALYTICS, "Analytiques avancés"],
];

const TIER_COLOR: Record<string, string> = {
  basic:        "gray",
  professional: "blue",
  enterprise:   "violet",
};

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function LicensePage() {
  const { t } = useTranslation();
  const { license, setLicense } = useLicenseStore();

  const [hwid,    setHwid]    = useState("Chargement…");
  const [blob,    setBlob]    = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    cmd.getHwid()
      .then(setHwid)
      .catch((e) => setHwid(`Erreur: ${e}`));
  }, []);

  const handleActivate = async () => {
    const key = blob.trim();
    if (!key) {
      notifications.show({ color: "orange", message: "Entrez une clé de licence." });
      return;
    }
    setLoading(true);
    try {
      const state = await cmd.verifyLicense(key);
      setLicense(state);
      notifications.show({
        color:   "green",
        title:   "✅ Licence activée",
        message: `Forfait ${state.tier.toUpperCase()} activé avec succès.`,
      });
      setBlob("");
    } catch (e) {
      notifications.show({
        color:   "red",
        title:   "Activation échouée",
        message: String(e),
      });
    } finally {
      setLoading(false);
    }
  };

  return (
    <Stack gap="lg" p="lg" maw={720} mx="auto">
      <Title order={2}>{t("license.title")}</Title>

      {/* ── Current status card ───────────────────────────────────────── */}
      <Paper
        withBorder
        p="lg"
        radius="md"
        style={{
          borderColor: license.is_valid
            ? "var(--mantine-color-green-3)"
            : "var(--mantine-color-orange-3)",
          background: license.is_valid
            ? "var(--mantine-color-green-0)"
            : "var(--mantine-color-orange-0)",
        }}
      >
        <Group justify="space-between" align="flex-start">
          <Group gap="sm">
            <ThemeIcon
              size="lg"
              radius="md"
              color={license.is_valid ? "green" : "orange"}
              variant="light"
            >
              {license.is_valid
                ? <IconShieldCheck size={22} />
                : <IconShieldOff size={22} />}
            </ThemeIcon>
            <Box>
              <Text fw={700} size="lg">
                {license.is_valid ? t("license.valid") : t("license.invalid")}
              </Text>
              {license.is_valid && (
                <Group gap="xs" mt={4}>
                  <Badge
                    color={TIER_COLOR[license.tier] ?? "gray"}
                    variant="filled"
                    tt="uppercase"
                  >
                    {license.tier}
                  </Badge>
                  <Text size="xs" c="dimmed">
                    {license.expires_at
                      ? t("license.expires", { date: license.expires_at })
                      : t("license.perpetual")}
                  </Text>
                </Group>
              )}
            </Box>
          </Group>
        </Group>

        {/* Feature flags grid */}
        {license.is_valid && (
          <>
            <Divider my="md" />
            <SimpleGrid cols={2} spacing="xs">
              {FLAG_LABELS.map(([flag, label]) => {
                const active = (license.features & flag) !== 0;
                return (
                  <Group
                    key={flag}
                    gap="xs"
                    p="xs"
                    style={{
                      borderRadius: 6,
                      background: active
                        ? "var(--mantine-color-green-1)"
                        : "var(--mantine-color-gray-1)",
                    }}
                  >
                    <Text c={active ? "green.7" : "gray.5"} size="sm">
                      {active ? "✓" : "○"}
                    </Text>
                    <Text size="sm" c={active ? "green.8" : "gray.5"}>
                      {label}
                    </Text>
                  </Group>
                );
              })}
            </SimpleGrid>
          </>
        )}
      </Paper>

      {/* ── HWID card ─────────────────────────────────────────────────── */}
      <Paper withBorder p="lg" radius="md">
        <Group gap="xs" mb="md">
          <IconCpu size={18} color="var(--mantine-color-blue-6)" />
          <Text fw={700}>{t("license.hwid_label")}</Text>
        </Group>

        <Group align="flex-start" gap="sm">
          <Code
            block
            style={{
              flex: 1,
              wordBreak: "break-all",
              fontSize: 12,
              userSelect: "all",
            }}
          >
            {hwid}
          </Code>
          <Group gap="xs">
            <CopyButton value={hwid} timeout={2000}>
              {({ copied, copy }) => (
                <Tooltip label={copied ? "Copié !" : t("license.copy_hwid")} withArrow>
                  <ActionIcon
                    size="lg"
                    variant={copied ? "filled" : "light"}
                    color={copied ? "teal" : "blue"}
                    onClick={copy}
                  >
                    {copied ? <IconCheck size={18} /> : <IconCopy size={18} />}
                  </ActionIcon>
                </Tooltip>
              )}
            </CopyButton>
            <Tooltip label="Relire les composants matériels" withArrow>
              <ActionIcon
                size="lg"
                variant="light"
                color="gray"
                onClick={() => {
                  setHwid("Chargement…");
                  cmd.getHwid().then(setHwid).catch((e) => setHwid(`Erreur: ${e}`));
                }}
              >
                <IconRefresh size={18} />
              </ActionIcon>
            </Tooltip>
          </Group>
        </Group>

        <Alert
          mt="sm"
          icon={<IconAlertCircle size={14} />}
          color="blue"
          variant="light"
          radius="md"
          style={{ fontSize: 12 }}
        >
          Transmettez ce code à votre revendeur pour obtenir une clé de licence
          liée à cet appareil. Il est impossible de l'inverser.
        </Alert>
      </Paper>

      {/* ── Activation card ───────────────────────────────────────────── */}
      <Paper withBorder p="lg" radius="md">
        <Group gap="xs" mb="md">
          <IconShieldLock size={18} color="var(--mantine-color-blue-6)" />
          <Text fw={700}>{t("license.enter_key")}</Text>
        </Group>

        <Textarea
          value={blob}
          onChange={(e) => setBlob(e.target.value)}
          placeholder="SUPERPOS-eyJleHBpcmVz…"
          minRows={4}
          maxRows={8}
          autosize
          styles={{
            input: {
              fontFamily:  "monospace",
              fontSize:    12,
              wordBreak:   "break-all",
            },
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              handleActivate();
            }
          }}
        />

        <Group justify="space-between" align="center" mt="md">
          <Text size="xs" c="dimmed">
            Vérification entièrement hors-ligne · Ctrl+↵ pour activer
          </Text>
          <Button
            leftSection={<IconKey size={16} />}
            onClick={handleActivate}
            loading={loading}
          >
            {t("license.activate")}
          </Button>
        </Group>
      </Paper>
    </Stack>
  );
}