// src/pages/Activation/index.tsx
//
// The activation wall. Shown exclusively when no valid license is present.
// The user cannot navigate anywhere else until activation succeeds.
//
// Layout:
//   Left panel  — branding + what's included in each tier
//   Right panel — HWID display + license key input + activation button

import { useEffect, useRef, useState } from "react";
import {
  Box, Stack, Group, Text, Title, Paper, Textarea, Button,
  CopyButton, Tooltip, ActionIcon, Badge, Divider, Loader,
  ThemeIcon, List, Alert, Code, Tabs, Kbd,
} from "@mantine/core";
import {
  IconCheck, IconCopy, IconKey, IconShieldLock,
  IconDeviceDesktopAnalytics, IconPackage, IconNotebook,
  IconRefresh, IconAlertCircle, IconLock,
  IconBuildingStore, IconCpu,
} from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";
import { useLicenseStore } from "@/store/licenseStore";
import * as cmd from "@/lib/commands";
import type { HwidComponents } from "@/lib/commands";

// ─── Tier info cards ──────────────────────────────────────────────────────────

const TIERS = [
  {
    name:    "Basic",
    color:   "gray",
    icon:    <IconBuildingStore size={18} />,
    items:   ["Caisse (POS)", "Paiement espèces", "Catalogue produits"],
  },
  {
    name:    "Professional",
    color:   "blue",
    icon:    <IconPackage size={18} />,
    items:   ["Tout ce qui est Basic", "Gestion des stocks + alertes expiration", "Impression thermique (58mm / 80mm)"],
  },
  {
    name:    "Enterprise",
    color:   "violet",
    icon:    <IconNotebook size={18} />,
    items:   ["Tout ce qui est Professional", "Dain — Crédit client", "Multi-caisse (Attente)", "Rapports A4", "Analytiques avancés"],
  },
] as const;

// ─── Component ────────────────────────────────────────────────────────────────

export default function ActivationPage() {
  const { license, setLicense } = useLicenseStore();

  const [hwid,       setHwid]       = useState<string>("");
  const [components, setComponents] = useState<HwidComponents | null>(null);
  const [keyInput,   setKeyInput]   = useState("");
  const [activating, setActivating] = useState(false);
  const [error,      setError]      = useState<string | null>(null);
  const [loading,    setLoading]    = useState(true);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Load HWID on mount
  useEffect(() => {
    (async () => {
      try {
        const [h, c] = await Promise.all([
          cmd.getHwid(),
          cmd.getHwidComponents(),
        ]);
        setHwid(h);
        setComponents(c);
      } catch (e) {
        setHwid("ERREUR — " + String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  // Activate
  const handleActivate = async () => {
    const key = keyInput.trim();
    if (!key) {
      setError("Veuillez coller votre clé de licence.");
      return;
    }
    if (!key.startsWith("SUPERPOS-")) {
      setError("Format invalide. La clé doit commencer par « SUPERPOS- ».");
      return;
    }

    setActivating(true);
    setError(null);

    try {
      const newState = await cmd.verifyLicense(key);
      setLicense(newState);
      notifications.show({
        title:   "✅ Activation réussie",
        message: `Bienvenue ! Forfait ${newState.tier.toUpperCase()} activé.`,
        color:   "green",
        autoClose: 5000,
      });
      // LicenseGuard will automatically unmount this page
    } catch (err) {
      const msg = String(err);
      setError(msg);
      notifications.show({
        title:   "Activation échouée",
        message: msg,
        color:   "red",
      });
    } finally {
      setActivating(false);
    }
  };

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <Box
      style={{
        minHeight:  "100vh",
        background: "linear-gradient(135deg, #0f0a2e 0%, #1a1050 50%, #0f0a2e 100%)",
        display:    "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
      }}
    >
      <Box style={{ width: "100%", maxWidth: 1000 }}>

        {/* Header */}
        <Group justify="center" mb="xl" gap="sm">
          <ThemeIcon size="xl" radius="md" color="blue" variant="filled">
            <IconShieldLock size={24} />
          </ThemeIcon>
          <div>
            <Title order={1} c="white" style={{ fontSize: 28, lineHeight: 1 }}>
              SuperPOS
            </Title>
            <Text c="blue.3" size="sm">Activation du logiciel</Text>
          </div>
        </Group>

        <Group align="flex-start" gap="lg" wrap="nowrap"
          style={{ flexDirection: "row" }}
        >

          {/* ── LEFT: tier info ─────────────────────────────────────────── */}
          <Stack style={{ flex: "0 0 300px", minWidth: 260 }}>
            <Text c="gray.4" size="xs" tt="uppercase" fw={600}>
              Forfaits disponibles
            </Text>
            {TIERS.map((tier) => (
              <Paper
                key={tier.name}
                p="md"
                radius="md"
                style={{
                  background: "rgba(255,255,255,0.05)",
                  border:     `1px solid var(--mantine-color-${tier.color}-8)`,
                }}
              >
                <Group gap="xs" mb="xs">
                  <ThemeIcon
                    size="sm"
                    radius="sm"
                    color={tier.color}
                    variant="filled"
                  >
                    {tier.icon}
                  </ThemeIcon>
                  <Text fw={700} c="white" size="sm">{tier.name}</Text>
                </Group>
                <List size="xs" spacing={4} c="gray.4">
                  {tier.items.map((item) => (
                    <List.Item key={item}
                      icon={<IconCheck size={11} color="var(--mantine-color-green-4)" />}
                    >
                      {item}
                    </List.Item>
                  ))}
                </List>
              </Paper>
            ))}
          </Stack>

          {/* ── RIGHT: activation form ───────────────────────────────────── */}
          <Paper
            p="xl"
            radius="lg"
            style={{
              flex:       1,
              background: "rgba(255,255,255,0.04)",
              border:     "1px solid rgba(255,255,255,0.10)",
              backdropFilter: "blur(12px)",
            }}
          >
            <Tabs defaultValue="activate" color="blue">
              <Tabs.List mb="lg">
                <Tabs.Tab value="activate" leftSection={<IconKey size={14} />}>
                  Activer
                </Tabs.Tab>
                <Tabs.Tab value="hwid" leftSection={<IconCpu size={14} />}>
                  Identifiant matériel
                </Tabs.Tab>
              </Tabs.List>

              {/* ── Tab 1: Activation ────────────────────────────────────── */}
              <Tabs.Panel value="activate">
                <Stack gap="md">
                  <div>
                    <Title order={3} c="white" mb={4}>
                      Entrez votre clé de licence
                    </Title>
                    <Text c="gray.4" size="sm">
                      Copiez la clé fournie par votre revendeur SuperPOS et
                      collez-la ci-dessous.
                    </Text>
                  </div>

                  {/* Rejection reason from previous attempt */}
                  {license.rejection && !error && (
                    <Alert
                      icon={<IconAlertCircle size={16} />}
                      color="orange"
                      radius="md"
                      variant="light"
                    >
                      {license.rejection}
                    </Alert>
                  )}

                  {error && (
                    <Alert
                      icon={<IconAlertCircle size={16} />}
                      color="red"
                      radius="md"
                      variant="light"
                      onClose={() => setError(null)}
                      withCloseButton
                    >
                      {error}
                    </Alert>
                  )}

                  <Textarea
                    ref={textareaRef}
                    value={keyInput}
                    onChange={(e) => { setKeyInput(e.target.value); setError(null); }}
                    placeholder={`SUPERPOS-eyJleHBpcmVz…`}
                    minRows={4}
                    maxRows={8}
                    autosize
                    styles={{
                      input: {
                        fontFamily:  "monospace",
                        fontSize:    12,
                        background:  "rgba(0,0,0,0.3)",
                        border:      "1px solid rgba(255,255,255,0.15)",
                        color:       "white",
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

                  <Button
                    fullWidth
                    size="md"
                    leftSection={<IconShieldLock size={18} />}
                    onClick={handleActivate}
                    loading={activating}
                    style={{ fontWeight: 700, fontSize: 15 }}
                  >
                    Activer SuperPOS
                    <Kbd
                      ml={8}
                      size="xs"
                      style={{ background: "rgba(255,255,255,0.2)", color: "white", border: "none" }}
                    >
                      Ctrl+↵
                    </Kbd>
                  </Button>

                  <Text size="xs" c="gray.6" ta="center">
                    La clé est vérifiée entièrement hors-ligne. Aucune
                    connexion internet n'est requise.
                  </Text>
                </Stack>
              </Tabs.Panel>

              {/* ── Tab 2: HWID ──────────────────────────────────────────── */}
              <Tabs.Panel value="hwid">
                <Stack gap="md">
                  <div>
                    <Title order={3} c="white" mb={4}>
                      Identifiant matériel (HWID)
                    </Title>
                    <Text c="gray.4" size="sm">
                      Transmettez cet identifiant à votre revendeur pour
                      obtenir une clé de licence liée à cet appareil.
                    </Text>
                  </div>

                  {loading ? (
                    <Group justify="center" py="xl">
                      <Loader color="blue" />
                      <Text c="gray.4">Lecture des composants matériels…</Text>
                    </Group>
                  ) : (
                    <>
                      {/* Full HWID */}
                      <Paper
                        p="md"
                        radius="md"
                        style={{
                          background: "rgba(0,0,0,0.3)",
                          border:     "1px solid rgba(255,255,255,0.1)",
                        }}
                      >
                        <Group justify="space-between" mb={6}>
                          <Text size="xs" c="blue.4" fw={600} tt="uppercase">
                            HWID (SHA-256)
                          </Text>
                          <CopyButton value={hwid} timeout={2000}>
                            {({ copied, copy }) => (
                              <Tooltip label={copied ? "Copié !" : "Copier le HWID"} withArrow>
                                <ActionIcon
                                  size="sm"
                                  color={copied ? "teal" : "gray"}
                                  variant="subtle"
                                  onClick={copy}
                                >
                                  {copied ? <IconCheck size={14} /> : <IconCopy size={14} />}
                                </ActionIcon>
                              </Tooltip>
                            )}
                          </CopyButton>
                        </Group>
                        <Code
                          block
                          style={{
                            background:  "transparent",
                            color:       "var(--mantine-color-blue-3)",
                            fontSize:    12,
                            wordBreak:   "break-all",
                            userSelect:  "all",
                          }}
                        >
                          {hwid}
                        </Code>
                      </Paper>

                      {/* Component breakdown */}
                      {components && (
                        <Stack gap="xs">
                          <Text size="xs" c="gray.5" tt="uppercase" fw={600}>
                            Sources utilisées
                          </Text>
                          {[
                            { label: "Machine UID (OS)", value: components.machine_uid },
                            { label: "Processeur",        value: components.cpu_brand   },
                            { label: "Identifiant plateforme", value: components.platform_id },
                          ].map(({ label, value }) => (
                            <Group
                              key={label}
                              justify="space-between"
                              p="xs"
                              style={{
                                background: "rgba(0,0,0,0.2)",
                                borderRadius: 6,
                                border: "1px solid rgba(255,255,255,0.06)",
                              }}
                            >
                              <Text size="xs" c="gray.4">{label}</Text>
                              <Code
                                style={{
                                  background: "transparent",
                                  color:      "var(--mantine-color-gray-3)",
                                  fontSize:   11,
                                }}
                              >
                                {value}
                              </Code>
                            </Group>
                          ))}
                        </Stack>
                      )}

                      {/* Refresh button */}
                      <Button
                        variant="subtle"
                        color="gray"
                        size="xs"
                        leftSection={<IconRefresh size={14} />}
                        onClick={async () => {
                          setLoading(true);
                          try {
                            const [h, c] = await Promise.all([cmd.getHwid(), cmd.getHwidComponents()]);
                            setHwid(h);
                            setComponents(c);
                          } finally {
                            setLoading(false);
                          }
                        }}
                      >
                        Rafraîchir la lecture matérielle
                      </Button>
                    </>
                  )}

                  <Divider color="gray.8" />

                  <Alert
                    icon={<IconLock size={14} />}
                    color="gray"
                    radius="md"
                    variant="light"
                    style={{ fontSize: 12 }}
                  >
                    Le HWID est un hachage SHA-256 de trois identifiants
                    matériels indépendants. Il est impossible de l'inverser
                    pour récupérer vos informations matérielles.
                  </Alert>
                </Stack>
              </Tabs.Panel>
            </Tabs>
          </Paper>
        </Group>
      </Box>
    </Box>
  );
}