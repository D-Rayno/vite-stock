// src/pages/Inventory/index.tsx
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Stack, Group, Title, Button, Paper, Badge, Text,
  TextInput, Alert, Table, Box, Loader, Center,
  SegmentedControl, Tooltip, ScrollArea,
} from "@mantine/core";
import {
  IconPlus, IconAlertTriangle, IconSearch,
  IconPackage, IconCalendar,
} from "@tabler/icons-react";
import { notifications }  from "@mantine/notifications";
import { FeatureGate }    from "@/components/ui/FeatureGate";
import { Features, type InventoryBatch, getExpiryStatus } from "@/types";
import type { AddBatchInput } from "@/lib/commands";
import * as cmd from "@/lib/commands";
import { AddBatchModal } from "./AddBatchModal";

// ─── Status config ────────────────────────────────────────────────────────────

const EXPIRY_COLOR: Record<ReturnType<typeof getExpiryStatus>, string> = {
  expired:  "red",
  critical: "orange",
  warning:  "yellow",
  ok:       "green",
  none:     "gray",
};

const EXPIRY_LABEL: Record<ReturnType<typeof getExpiryStatus>, string> = {
  expired:  "⛔ Expiré",
  critical: "🔴 Critique",
  warning:  "🟡 Attention",
  ok:       "🟢 OK",
  none:     "—",
};

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function InventoryPage() {
  const { t } = useTranslation();

  const [batches,  setBatches]  = useState<InventoryBatch[]>([]);
  const [alerts,   setAlerts]   = useState<InventoryBatch[]>([]);
  const [loading,  setLoading]  = useState(true);
  const [tab,      setTab]      = useState<"all" | "alerts">("all");
  const [addOpen,  setAddOpen]  = useState(false);
  const [search,   setSearch]   = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [all, warn] = await Promise.all([
        cmd.getInventoryBatches(),
        cmd.getExpiryAlerts(30),
      ]);
      setBatches(all);
      setAlerts(warn);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const displayed = (tab === "alerts" ? alerts : batches).filter((b) =>
    !search || b.product_name.toLowerCase().includes(search.toLowerCase()),
  );

  const handleAddBatch = async (input: AddBatchInput) => {
    try {
      await cmd.addInventoryBatch(input);
      notifications.show({ color: "green", title: "Stock ajouté", message: "Nouveau lot enregistré avec succès." });
      setAddOpen(false);
      load();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    }
  };

  // Expiry stats
  const expiredCount  = alerts.filter(b => getExpiryStatus(b.days_until_expiry) === "expired").length;
  const warningCount  = alerts.filter(b => ["critical","warning"].includes(getExpiryStatus(b.days_until_expiry))).length;

  return (
    <FeatureGate flag={Features.INVENTORY_MGMT}>
      <Stack gap="lg" p="lg" style={{ height: "100vh", overflow: "hidden" }}>

        {/* ── Header ───────────────────────────────────────────────────── */}
        <Group justify="space-between">
          <Title order={2}>{t("inventory.title")}</Title>
          <Button leftSection={<IconPlus size={16} />} onClick={() => setAddOpen(true)}>
            {t("inventory.add_batch")}
          </Button>
        </Group>

        {/* ── Alert strip ──────────────────────────────────────────────── */}
        {alerts.length > 0 && (
          <Alert
            icon={<IconAlertTriangle size={16} />}
            color="orange"
            radius="md"
            style={{ cursor: "pointer" }}
            onClick={() => setTab("alerts")}
          >
            <Group gap="xs" wrap="wrap">
              {expiredCount > 0 && (
                <Badge color="red" variant="filled" size="sm">
                  {expiredCount} expiré{expiredCount > 1 ? "s" : ""}
                </Badge>
              )}
              {warningCount > 0 && (
                <Badge color="orange" variant="filled" size="sm">
                  {warningCount} expirant dans 30 j.
                </Badge>
              )}
              <Text size="sm">Cliquez pour voir les alertes d'expiration.</Text>
            </Group>
          </Alert>
        )}

        {/* ── Filters row ──────────────────────────────────────────────── */}
        <Group>
          <SegmentedControl
            value={tab}
            onChange={(v) => setTab(v as "all" | "alerts")}
            data={[
              { label: `Tous les stocks (${batches.length})`, value: "all" },
              { label: `Alertes (${alerts.length})`, value: "alerts" },
            ]}
          />
          <TextInput
            placeholder="Filtrer par produit…"
            leftSection={<IconSearch size={16} />}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{ flex: 1, maxWidth: 300 }}
          />
        </Group>

        {/* ── Table ────────────────────────────────────────────────────── */}
        {loading ? (
          <Center style={{ flex: 1 }}>
            <Loader size="lg" />
          </Center>
        ) : (
          <Paper withBorder radius="md" style={{ flex: 1, overflow: "hidden" }}>
            <ScrollArea style={{ height: "100%" }}>
              <Table stickyHeader highlightOnHover withRowBorders>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Produit</Table.Th>
                    <Table.Th style={{ textAlign: "center" }}>{t("inventory.qty")}</Table.Th>
                    <Table.Th style={{ textAlign: "center" }}>{t("inventory.expiry")}</Table.Th>
                    <Table.Th style={{ textAlign: "center" }}>Statut</Table.Th>
                    <Table.Th>{t("inventory.supplier")}</Table.Th>
                    <Table.Th>{t("inventory.received")}</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {displayed.length === 0 ? (
                    <Table.Tr>
                      <Table.Td colSpan={6}>
                        <Center py="xl">
                          <Stack align="center" gap="xs" opacity={0.5}>
                            <IconPackage size={40} stroke={1} />
                            <Text c="dimmed">Aucun résultat</Text>
                          </Stack>
                        </Center>
                      </Table.Td>
                    </Table.Tr>
                  ) : (
                    displayed.map((b) => {
                      const status = getExpiryStatus(b.days_until_expiry);
                      return (
                        <Table.Tr
                          key={b.id}
                          style={{
                            background: (status === "expired" || status === "critical")
                              ? "var(--mantine-color-red-0)"
                              : undefined,
                          }}
                        >
                          <Table.Td>
                            <Text fw={600} size="sm">{b.product_name}</Text>
                          </Table.Td>

                          <Table.Td style={{ textAlign: "center" }}>
                            <Text ff="monospace" fw={700}>
                              {b.quantity % 1 === 0
                                ? b.quantity.toFixed(0)
                                : b.quantity.toFixed(2)}
                            </Text>
                          </Table.Td>

                          <Table.Td style={{ textAlign: "center" }}>
                            {b.expiry_date ? (
                              <Stack gap={2} align="center">
                                <Text size="xs" ff="monospace">{b.expiry_date}</Text>
                                {b.days_until_expiry !== null && (
                                  <Text size="xs" c="dimmed">
                                    {b.days_until_expiry >= 0
                                      ? `dans ${b.days_until_expiry} j`
                                      : `il y a ${Math.abs(b.days_until_expiry)} j`}
                                  </Text>
                                )}
                              </Stack>
                            ) : (
                              <Text c="dimmed" size="sm">—</Text>
                            )}
                          </Table.Td>

                          <Table.Td style={{ textAlign: "center" }}>
                            <Badge
                              color={EXPIRY_COLOR[status]}
                              variant="light"
                              size="sm"
                            >
                              {EXPIRY_LABEL[status]}
                            </Badge>
                          </Table.Td>

                          <Table.Td>
                            <Text size="sm" c="dimmed">
                              {b.supplier_ref ?? "—"}
                            </Text>
                          </Table.Td>

                          <Table.Td>
                            <Text size="xs" c="dimmed">
                              {b.received_at.slice(0, 10)}
                            </Text>
                          </Table.Td>
                        </Table.Tr>
                      );
                    })
                  )}
                </Table.Tbody>
              </Table>
            </ScrollArea>
          </Paper>
        )}
      </Stack>

      {/* ── Add batch modal ───────────────────────────────────────────── */}
      <AddBatchModal
        opened={addOpen}
        onSave={handleAddBatch}
        onClose={() => setAddOpen(false)}
      />
    </FeatureGate>
  );
}