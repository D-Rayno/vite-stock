// src/pages/Reports/index.tsx
// Full analytics Reports page.
// Uses Mantine components for layout and a lightweight inline SVG bar chart.

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Stack, Group, Text, Title, Button, Paper, Grid, Badge,
  Select, Loader, Center, Divider, Table, Tabs, Progress,
  ActionIcon, Tooltip, SegmentedControl,
} from "@mantine/core";
import {
  IconCalendar, IconDownload, IconRefresh,
  IconTrendingUp, IconShoppingCart, IconReceipt,
  IconCash, IconCreditCard, IconNotebook,
  IconChartBar, IconTable, IconPackage,
} from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";

interface DailyReport {
  date: string;
  total_sales: number; total_ht: number; total_vat: number;
  transaction_count: number; avg_basket: number;
  cash_total: number; cib_total: number; dain_total: number;
}

interface DailyBreakdown { date: string; sales: number; txns: number; }
interface ProductSalesRow {
  product_id: number; product_name: string; gtin: string | null;
  category: string | null; qty_sold: number;
  revenue_ht: number; revenue_ttc: number; txn_count: number;
}
interface HourlyRow { hour: number; sales: number; txns: number; }
interface PaymentRow { method: string; total: number; count: number; percent: number; }
interface FullReport {
  summary: {
    date_from: string; date_to: string;
    total_sales: number; total_ht: number; total_vat: number;
    transaction_count: number; avg_basket: number;
    daily_breakdown: DailyBreakdown[];
  };
  top_products: ProductSalesRow[];
  hourly_heatmap: HourlyRow[];
  payment_breakdown: PaymentRow[];
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

const fmt = (n: number) => n.toLocaleString("fr-DZ", { minimumFractionDigits: 2, maximumFractionDigits: 2 }) + " DZD";
const today = () => new Date().toISOString().slice(0, 10);
const daysAgo = (n: number) => {
  const d = new Date(); d.setDate(d.getDate() - n);
  return d.toISOString().slice(0, 10);
};

const PRESETS = [
  { label: "Aujourd'hui",    from: () => today(),     to: () => today() },
  { label: "7 derniers j.",  from: () => daysAgo(6),  to: () => today() },
  { label: "30 derniers j.", from: () => daysAgo(29), to: () => today() },
  { label: "Ce mois",        from: () => today().slice(0,7) + "-01", to: () => today() },
];

const METHOD_META: Record<string, { label: string; color: string; icon: React.ReactNode }> = {
  cash:     { label: "Espèces",  color: "green",  icon: <IconCash size={14} /> },
  cib:      { label: "CIB",      color: "blue",   icon: <IconCreditCard size={14} /> },
  dahabia:  { label: "Dahabia",  color: "indigo", icon: <IconCreditCard size={14} /> },
  dain:     { label: "Dain",     color: "orange", icon: <IconNotebook size={14} /> },
};

// ─── Inline SVG bar chart ──────────────────────────────────────────────────────

function BarChart({ data, maxVal }: { data: { label: string; value: number; color?: string }[]; maxVal: number }) {
  const H = 120, W = 420, BAR_W = Math.min(32, W / data.length - 4);
  const GAP = (W - data.length * BAR_W) / (data.length + 1);
  return (
    <svg viewBox={`0 0 ${W} ${H + 20}`} style={{ width: "100%", maxWidth: W }}>
      {data.map((d, i) => {
        const barH = maxVal > 0 ? (d.value / maxVal) * H : 0;
        const x    = GAP + i * (BAR_W + GAP);
        const y    = H - barH;
        return (
          <g key={i}>
            <rect
              x={x} y={y} width={BAR_W} height={barH}
              rx={3}
              fill={d.color ?? "var(--mantine-color-blue-5)"}
              opacity={0.85}
            />
            <text x={x + BAR_W / 2} y={H + 14} textAnchor="middle"
              style={{ fontSize: 9, fill: "var(--mantine-color-gray-6)" }}>
              {d.label}
            </text>
          </g>
        );
      })}
      {/* Y axis zero line */}
      <line x1={0} y1={H} x2={W} y2={H}
        stroke="var(--mantine-color-gray-3)" strokeWidth={1} />
    </svg>
  );
}

function HeatmapChart({ data }: { data: HourlyRow[] }) {
  const byHour = Array.from({ length: 24 }, (_, h) => {
    const row = data.find(r => r.hour === h);
    return { hour: h, sales: row?.sales ?? 0, txns: row?.txns ?? 0 };
  });
  const maxSales = Math.max(...byHour.map(r => r.sales), 1);

  return (
    <Group gap={3} wrap="nowrap">
      {byHour.map(({ hour, sales, txns }) => {
        const intensity = sales / maxSales;
        const alpha = 0.08 + intensity * 0.82;
        return (
          <Tooltip
            key={hour}
            label={`${String(hour).padStart(2,"0")}h — ${fmt(sales)} (${txns} txn)`}
            withArrow position="top"
          >
            <div style={{
              flex: 1,
              height: 36,
              borderRadius: 4,
              background: `rgba(66, 99, 235, ${alpha})`,
              display: "flex",
              alignItems: "flex-end",
              justifyContent: "center",
              paddingBottom: 2,
            }}>
              <Text size="xs" c="dimmed" style={{ fontSize: 9, lineHeight: 1 }}>
                {String(hour).padStart(2,"0")}
              </Text>
            </div>
          </Tooltip>
        );
      })}
    </Group>
  );
}

// ─── KPI Card ──────────────────────────────────────────────────────────────────

function KpiCard({ label, value, sub, icon, color = "blue" }: {
  label: string; value: string; sub?: string;
  icon: React.ReactNode; color?: string;
}) {
  return (
    <Paper p="md" radius="md" withBorder style={{ flex: 1, minWidth: 160 }}>
      <Group gap="sm" mb="xs">
        <ActionIcon variant="light" color={color} size="md" radius="md">
          {icon}
        </ActionIcon>
        <Text size="xs" c="dimmed" tt="uppercase" fw={600}>{label}</Text>
      </Group>
      <Text size="xl" fw={800} ff="monospace" c={`${color}.7`}>{value}</Text>
      {sub && <Text size="xs" c="dimmed" mt={2}>{sub}</Text>}
    </Paper>
  );
}

// ─── Main page ────────────────────────────────────────────────────────────────

export default function ReportsPage() {
  const [dateFrom, setDateFrom] = useState(today());
  const [dateTo,   setDateTo]   = useState(today());
  const [report,   setReport]   = useState<FullReport | null>(null);
  const [loading,  setLoading]  = useState(false);
  const [exporting, setExporting] = useState(false);

  const load = useCallback(async (from = dateFrom, to = dateTo) => {
    setLoading(true);
    try {
      const r = await invoke<FullReport>("cmd_get_full_report", {
        dateFrom: from, dateTo: to,
      });
      setReport(r);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setLoading(false);
    }
  }, [dateFrom, dateTo]);

  useEffect(() => { load(); }, []);

  const applyPreset = (from: string, to: string) => {
    setDateFrom(from); setDateTo(to); load(from, to);
  };

  const handleExport = async () => {
    setExporting(true);
    try {
      const result = await invoke<{ path: string; rows: number }>(
        "cmd_export_sales_excel",
        { request: { date_from: dateFrom, date_to: dateTo } },
      );
      notifications.show({
        color: "green",
        title: "Export réussi",
        message: `${result.rows} transactions → ${result.path.split(/[\\/]/).pop()}`,
      });
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setExporting(false);
    }
  };

  const s = report?.summary;

  return (
    <Stack gap="md" p="md" style={{ minHeight: "100vh" }}>

      {/* ── Header ─────────────────────────────────────────────────────── */}
      <Group justify="space-between" wrap="wrap" gap="xs">
        <Title order={2}>Rapports & Analytiques</Title>
        <Group gap="xs">
          <Tooltip label="Rafraîchir" withArrow>
            <ActionIcon variant="light" onClick={() => load()} loading={loading}>
              <IconRefresh size={16} />
            </ActionIcon>
          </Tooltip>
          <Button
            size="sm"
            variant="light"
            color="green"
            leftSection={<IconDownload size={16} />}
            onClick={handleExport}
            loading={exporting}
          >
            Export Excel
          </Button>
        </Group>
      </Group>

      {/* ── Date controls ──────────────────────────────────────────────── */}
      <Paper p="sm" radius="md" withBorder>
        <Group gap="md" wrap="wrap">
          {PRESETS.map(p => (
            <Button
              key={p.label}
              size="xs"
              variant={dateFrom === p.from() && dateTo === p.to() ? "filled" : "light"}
              onClick={() => applyPreset(p.from(), p.to())}
            >
              {p.label}
            </Button>
          ))}
          <Divider orientation="vertical" />
          <Group gap="xs">
            <Text size="sm" c="dimmed">Du :</Text>
            <input
              type="date"
              value={dateFrom}
              max={dateTo}
              onChange={e => setDateFrom(e.target.value)}
              style={{
                border: "1px solid var(--mantine-color-gray-3)",
                borderRadius: 6, padding: "4px 8px", fontSize: 13,
              }}
            />
            <Text size="sm" c="dimmed">Au :</Text>
            <input
              type="date"
              value={dateTo}
              min={dateFrom}
              max={today()}
              onChange={e => setDateTo(e.target.value)}
              style={{
                border: "1px solid var(--mantine-color-gray-3)",
                borderRadius: 6, padding: "4px 8px", fontSize: 13,
              }}
            />
            <Button size="xs" onClick={() => load()}>Appliquer</Button>
          </Group>
        </Group>
      </Paper>

      {loading && !report ? (
        <Center h={200}><Loader size="lg" /></Center>
      ) : !report ? null : (
        <Tabs defaultValue="overview" keepMounted={false}>
          <Tabs.List mb="md">
            <Tabs.Tab value="overview"  leftSection={<IconTrendingUp size={14} />}>Vue d'ensemble</Tabs.Tab>
            <Tabs.Tab value="products"  leftSection={<IconPackage size={14} />}>Top Produits</Tabs.Tab>
            <Tabs.Tab value="heatmap"   leftSection={<IconChartBar size={14} />}>Activité horaire</Tabs.Tab>
          </Tabs.List>

          {/* ── Overview tab ───────────────────────────────────────────── */}
          <Tabs.Panel value="overview">
            <Stack gap="md">

              {/* KPI cards */}
              <Group gap="sm" grow wrap="wrap">
                <KpiCard
                  label="Ventes TTC"
                  value={fmt(s!.total_sales)}
                  sub={`HT: ${fmt(s!.total_ht)}`}
                  icon={<IconTrendingUp size={16} />}
                  color="blue"
                />
                <KpiCard
                  label="Transactions"
                  value={s!.transaction_count.toString()}
                  sub={`Panier moyen: ${fmt(s!.avg_basket)}`}
                  icon={<IconReceipt size={16} />}
                  color="teal"
                />
                <KpiCard
                  label="TVA collectée"
                  value={fmt(s!.total_vat)}
                  sub={`${((s!.total_vat / Math.max(s!.total_sales, 1)) * 100).toFixed(1)}% du CA`}
                  icon={<IconShoppingCart size={16} />}
                  color="violet"
                />
              </Group>

              {/* Daily trend bar chart */}
              {s!.daily_breakdown.length > 1 && (
                <Paper p="md" radius="md" withBorder>
                  <Text size="sm" fw={600} mb="sm">Tendance journalière</Text>
                  <BarChart
                    maxVal={Math.max(...s!.daily_breakdown.map(d => d.sales))}
                    data={s!.daily_breakdown.map(d => ({
                      label: d.date.slice(5),  // MM-DD
                      value: d.sales,
                    }))}
                  />
                </Paper>
              )}

              {/* Payment breakdown */}
              <Paper p="md" radius="md" withBorder>
                <Text size="sm" fw={600} mb="md">Répartition par mode de paiement</Text>
                <Stack gap="sm">
                  {report.payment_breakdown.map(p => {
                    const meta = METHOD_META[p.method] ?? { label: p.method, color: "gray", icon: null };
                    return (
                      <div key={p.method}>
                        <Group justify="space-between" mb={4}>
                          <Group gap="xs">
                            <Badge color={meta.color} variant="light" size="sm" leftSection={meta.icon}>
                              {meta.label}
                            </Badge>
                            <Text size="xs" c="dimmed">{p.count} transaction{p.count > 1 ? "s" : ""}</Text>
                          </Group>
                          <Text size="sm" fw={600} ff="monospace">
                            {fmt(p.total)}
                            <Text span size="xs" c="dimmed" fw={400}> ({p.percent}%)</Text>
                          </Text>
                        </Group>
                        <Progress
                          value={p.percent}
                          color={meta.color}
                          size="sm"
                          radius="xl"
                          animated={loading}
                        />
                      </div>
                    );
                  })}
                </Stack>
              </Paper>
            </Stack>
          </Tabs.Panel>

          {/* ── Top Products tab ───────────────────────────────────────── */}
          <Tabs.Panel value="products">
            <Paper radius="md" withBorder style={{ overflow: "hidden" }}>
              <Table stickyHeader highlightOnHover>
                <Table.Thead style={{ background: "var(--mantine-color-dark-7)" }}>
                  <Table.Tr>
                    {["#", "Produit", "Catégorie", "Qté", "CA TTC", "Nb Txn"].map(h => (
                      <Table.Th key={h}
                        style={{ color: "var(--mantine-color-gray-4)", fontSize: 11, textTransform: "uppercase" }}
                      >{h}</Table.Th>
                    ))}
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {report.top_products.map((p, i) => (
                    <Table.Tr key={p.product_id}>
                      <Table.Td>
                        <Badge variant="light" size="xs" color={i < 3 ? "yellow" : "gray"}>
                          {i + 1}
                        </Badge>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm" fw={600}>{p.product_name}</Text>
                        {p.gtin && <Text size="xs" c="dimmed" ff="monospace">{p.gtin}</Text>}
                      </Table.Td>
                      <Table.Td>
                        <Text size="xs" c="dimmed">{p.category ?? "—"}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm" ff="monospace">{p.qty_sold.toFixed(p.qty_sold % 1 === 0 ? 0 : 2)}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm" fw={700} ff="monospace" c="blue.7">
                          {fmt(p.revenue_ttc)}
                        </Text>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm" c="dimmed">{p.txn_count}</Text>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                  {report.top_products.length === 0 && (
                    <Table.Tr>
                      <Table.Td colSpan={6}>
                        <Center py="xl"><Text c="dimmed">Aucune vente sur cette période</Text></Center>
                      </Table.Td>
                    </Table.Tr>
                  )}
                </Table.Tbody>
              </Table>
            </Paper>
          </Tabs.Panel>

          {/* ── Hourly heatmap tab ─────────────────────────────────────── */}
          <Tabs.Panel value="heatmap">
            <Stack gap="md">
              <Paper p="lg" radius="md" withBorder>
                <Text size="sm" fw={600} mb="xs">Activité par heure de la journée</Text>
                <Text size="xs" c="dimmed" mb="md">
                  Intensité = volume de ventes. Passez la souris sur chaque colonne pour le détail.
                </Text>
                <HeatmapChart data={report.hourly_heatmap} />
                <Group mt="sm" gap="md">
                  <Group gap={4}>
                    <div style={{ width: 12, height: 12, borderRadius: 2, background: "rgba(66,99,235,0.1)", border: "1px solid #eee" }} />
                    <Text size="xs" c="dimmed">Faible activité</Text>
                  </Group>
                  <Group gap={4}>
                    <div style={{ width: 12, height: 12, borderRadius: 2, background: "rgba(66,99,235,0.9)" }} />
                    <Text size="xs" c="dimmed">Forte activité</Text>
                  </Group>
                </Group>
              </Paper>

              {/* Peak hour callout */}
              {report.hourly_heatmap.length > 0 && (() => {
                const peak = [...report.hourly_heatmap].sort((a,b) => b.sales - a.sales)[0];
                return (
                  <Paper p="md" radius="md" withBorder style={{ background: "var(--mantine-color-blue-0)" }}>
                    <Group gap="sm">
                      <IconChartBar size={20} color="var(--mantine-color-blue-6)" />
                      <div>
                        <Text size="sm" fw={600}>
                          Heure de pointe : {String(peak.hour).padStart(2,"0")}h00–{String(peak.hour+1).padStart(2,"0")}h00
                        </Text>
                        <Text size="xs" c="dimmed">
                          {fmt(peak.sales)} de ventes · {peak.txns} transaction{peak.txns > 1 ? "s" : ""}
                        </Text>
                      </div>
                    </Group>
                  </Paper>
                );
              })()}
            </Stack>
          </Tabs.Panel>
        </Tabs>
      )}
    </Stack>
  );
}