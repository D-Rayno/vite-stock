// src/pages/Dain/index.tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Stack, Group, Title, Text, TextInput, Button, Paper,
  Badge, Divider, Textarea, NumberInput, ActionIcon,
  Alert, ScrollArea, Box, Loader, Center,
} from "@mantine/core";
import {
  IconSearch, IconPlus, IconMinus, IconAlertCircle,
  IconPhone, IconCurrencyDollar, IconHistory,
  IconCheck, IconX,
} from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";
import { FeatureGate }   from "@/components/ui/FeatureGate";
import { Features, type CustomerDainSummary, type DainEntry } from "@/types";
import * as cmd from "@/lib/commands";

export default function DainPage() {
  const { t } = useTranslation();

  const [phone,    setPhone]    = useState("");
  const [customer, setCustomer] = useState<CustomerDainSummary | null>(null);
  const [history,  setHistory]  = useState<DainEntry[]>([]);
  const [loading,  setLoading]  = useState(false);
  const [notFound, setNotFound] = useState(false);

  // Entry form
  const [mode,   setMode]   = useState<"debt" | "repayment" | null>(null);
  const [amount, setAmount] = useState<number | string>("");
  const [notes,  setNotes]  = useState("");
  const [saving, setSaving] = useState(false);

  const search = async () => {
    if (!phone.trim()) return;
    setLoading(true);
    setNotFound(false);
    setCustomer(null);
    setHistory([]);
    setMode(null);
    try {
      const c = await cmd.getCustomerDain(phone.trim());
      setCustomer(c);
      const h = await cmd.getDainHistory(c.customer_id);
      setHistory(h);
    } catch {
      setNotFound(true);
    } finally {
      setLoading(false);
    }
  };

  const handleEntry = async () => {
    if (!customer || !mode || !amount || saving) return;
    const amountNum = typeof amount === "number" ? amount : parseFloat(String(amount));
    if (!amountNum || amountNum <= 0) return;
    setSaving(true);
    try {
      if (mode === "debt") {
        await cmd.addDainEntry(customer.customer_id, null, amountNum, notes || null);
        notifications.show({ color: "red", title: "Dette enregistrée", message: `${amountNum.toFixed(2)} DZD ajouté au solde.` });
      } else {
        await cmd.repayDain(customer.customer_id, amountNum, notes || null);
        notifications.show({ color: "green", title: "Remboursement enregistré", message: `${amountNum.toFixed(2)} DZD déduit du solde.` });
      }
      setMode(null);
      setAmount("");
      setNotes("");
      const [c, h] = await Promise.all([
        cmd.getCustomerDain(phone.trim()),
        cmd.getDainHistory(customer.customer_id),
      ]);
      setCustomer(c);
      setHistory(h);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <FeatureGate flag={Features.DAIN_LEDGER}>
      <Stack gap="lg" p="lg" maw={800} mx="auto">

        {/* ── Header ────────────────────────────────────────────────────── */}
        <Title order={2}>{t("dain.title")}</Title>

        {/* ── Search ────────────────────────────────────────────────────── */}
        <Group gap="sm">
          <TextInput
            style={{ flex: 1 }}
            placeholder={t("dain.search_phone")}
            leftSection={<IconPhone size={16} />}
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && search()}
            autoFocus
          />
          <Button
            leftSection={<IconSearch size={16} />}
            onClick={search}
            loading={loading}
          >
            Rechercher
          </Button>
        </Group>

        {/* ── Not found ─────────────────────────────────────────────────── */}
        {notFound && (
          <Alert icon={<IconAlertCircle size={16} />} color="orange" radius="md">
            {t("dain.no_customer")}
          </Alert>
        )}

        {/* ── Loading ───────────────────────────────────────────────────── */}
        {loading && (
          <Center py="xl">
            <Loader />
          </Center>
        )}

        {/* ── Customer card ─────────────────────────────────────────────── */}
        {customer && (
          <Stack gap="md">
            <Paper withBorder p="lg" radius="md">
              <Group justify="space-between" align="flex-start">
                {/* Customer info */}
                <Stack gap={4}>
                  <Text fw={700} size="xl">{customer.name}</Text>
                  <Group gap="xs">
                    <IconPhone size={14} color="var(--mantine-color-dimmed)" />
                    <Text size="sm" c="dimmed">{customer.phone}</Text>
                  </Group>
                </Stack>

                {/* Balance badge */}
                <Paper
                  p="md"
                  radius="md"
                  style={{
                    background: customer.balance > 0
                      ? "var(--mantine-color-red-0)"
                      : "var(--mantine-color-green-0)",
                    border: `1px solid ${customer.balance > 0
                      ? "var(--mantine-color-red-3)"
                      : "var(--mantine-color-green-3)"}`,
                    textAlign: "right",
                  }}
                >
                  <Text size="xs" c="dimmed" fw={600} tt="uppercase">
                    {t("dain.balance")}
                  </Text>
                  <Text
                    fw={900}
                    size="xl"
                    ff="monospace"
                    c={customer.balance > 0 ? "red.7" : "green.7"}
                  >
                    {customer.balance.toFixed(2)} DZD
                  </Text>
                </Paper>
              </Group>

              {/* Action buttons */}
              <Divider my="md" />
              <Group>
                <Button
                  color="red"
                  variant={mode === "debt" ? "filled" : "light"}
                  leftSection={<IconPlus size={16} />}
                  onClick={() => setMode(mode === "debt" ? null : "debt")}
                  style={{ flex: 1 }}
                >
                  {t("dain.add_debt")}
                </Button>
                <Button
                  color="green"
                  variant={mode === "repayment" ? "filled" : "light"}
                  leftSection={<IconMinus size={16} />}
                  onClick={() => setMode(mode === "repayment" ? null : "repayment")}
                  style={{ flex: 1 }}
                >
                  {t("dain.add_repay")}
                </Button>
              </Group>

              {/* Inline entry form */}
              {mode && (
                <Paper
                  mt="md"
                  p="md"
                  radius="md"
                  style={{
                    background: mode === "debt"
                      ? "var(--mantine-color-red-0)"
                      : "var(--mantine-color-green-0)",
                    border: `1px solid ${mode === "debt"
                      ? "var(--mantine-color-red-2)"
                      : "var(--mantine-color-green-2)"}`,
                  }}
                >
                  <Text fw={600} size="sm" mb="sm">
                    {mode === "debt" ? "Nouvelle dette" : "Remboursement"}
                  </Text>
                  <Group align="flex-end" gap="sm">
                    <NumberInput
                      label={t("dain.amount")}
                      placeholder="0.00"
                      value={amount}
                      onChange={setAmount}
                      min={0.01}
                      step={100}
                      decimalScale={2}
                      fixedDecimalScale
                      leftSection={<IconCurrencyDollar size={14} />}
                      rightSection={<Text size="xs" c="dimmed" pr={4}>DZD</Text>}
                      rightSectionWidth={40}
                      style={{ flex: 1 }}
                      autoFocus
                    />
                    <TextInput
                      label={t("dain.notes")}
                      placeholder="Optionnel"
                      value={notes}
                      onChange={(e) => setNotes(e.target.value)}
                      style={{ flex: 1 }}
                    />
                    <Group gap="xs" pb={1}>
                      <Button
                        color={mode === "debt" ? "red" : "green"}
                        leftSection={<IconCheck size={14} />}
                        onClick={handleEntry}
                        loading={saving}
                        disabled={!amount}
                      >
                        Confirmer
                      </Button>
                      <Button
                        variant="subtle"
                        color="gray"
                        leftSection={<IconX size={14} />}
                        onClick={() => { setMode(null); setAmount(""); setNotes(""); }}
                      >
                        Annuler
                      </Button>
                    </Group>
                  </Group>
                </Paper>
              )}
            </Paper>

            {/* ── History ───────────────────────────────────────────────── */}
            <Paper withBorder p="lg" radius="md">
              <Group gap="xs" mb="md">
                <IconHistory size={18} />
                <Text fw={700}>{t("dain.history")}</Text>
                <Badge size="sm" variant="light">{history.length}</Badge>
              </Group>

              {history.length === 0 ? (
                <Center py="xl">
                  <Text c="dimmed" size="sm">Aucune transaction</Text>
                </Center>
              ) : (
                <ScrollArea mah={400}>
                  <Stack gap="xs">
                    {history.map((entry) => (
                      <Group
                        key={entry.id}
                        justify="space-between"
                        p="sm"
                        style={{
                          borderRadius: 8,
                          background: entry.entry_type === "debt"
                            ? "var(--mantine-color-red-0)"
                            : "var(--mantine-color-green-0)",
                          border: `1px solid ${entry.entry_type === "debt"
                            ? "var(--mantine-color-red-2)"
                            : "var(--mantine-color-green-2)"}`,
                        }}
                      >
                        <Box>
                          <Group gap="xs">
                            <Badge
                              color={entry.entry_type === "debt" ? "red" : "green"}
                              variant="light"
                              size="sm"
                            >
                              {entry.entry_type === "debt"
                                ? `＋ ${t("dain.debt")}`
                                : `－ ${t("dain.repayment")}`}
                            </Badge>
                            {entry.notes && (
                              <Text size="xs" c="dimmed">— {entry.notes}</Text>
                            )}
                          </Group>
                          <Text size="xs" c="dimmed" mt={2}>
                            {entry.created_at.slice(0, 16).replace("T", " ")}
                          </Text>
                        </Box>
                        <Text
                          fw={700}
                          ff="monospace"
                          c={entry.entry_type === "debt" ? "red.7" : "green.7"}
                        >
                          {entry.entry_type === "debt" ? "+" : "−"}
                          {entry.amount.toFixed(2)} DZD
                        </Text>
                      </Group>
                    ))}
                  </Stack>
                </ScrollArea>
              )}
            </Paper>
          </Stack>
        )}
      </Stack>
    </FeatureGate>
  );
}