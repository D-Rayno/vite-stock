// src/pages/POS/PaymentPanel.tsx
import { useState } from "react";
import {
  Modal, Stack, Group, Text, NumberInput, Button,
  Paper, Divider, Badge, Kbd, Alert, SimpleGrid,
} from "@mantine/core";
import {
  IconCash, IconCreditCard, IconBook,
  IconCheck, IconX, IconDiscount, IconAlertCircle,
} from "@tabler/icons-react";
import type { Cart, CartTotals, PaymentMethod } from "@/types";
import { useCartStore } from "@/store/cartStore";

interface Props {
  totals:         CartTotals;
  cart:           Cart;
  paymentOpen:    boolean;
  onOpenPayment:  () => void;
  onClosePayment: () => void;
  onCheckout:     (method: PaymentMethod, paid: number) => Promise<void>;
}

const QUICK_AMOUNTS = [500, 1000, 2000, 5000];
const PAYMENT_METHODS = [
  { value: "cash",    label: "Espèces",  icon: <IconCash size={16} /> },
  { value: "cib",     label: "CIB",      icon: <IconCreditCard size={16} /> },
  { value: "dahabia", label: "Dahabia",  icon: <IconCreditCard size={16} /> },
  { value: "dain",    label: "Dain",     icon: <IconBook size={16} /> },
] as const;

export function PaymentPanel({ totals, cart, paymentOpen, onOpenPayment, onClosePayment, onCheckout }: Props) {
  const { setCartDiscount, clearActiveCart } = useCartStore();
  const [method,  setMethod]  = useState<PaymentMethod>("cash");
  const [paid,    setPaid]    = useState<number | string>("");
  const [loading, setLoading] = useState(false);

  const paidNum = typeof paid === "number" ? paid : parseFloat(String(paid)) || 0;
  const change  = Math.max(0, paidNum - totals.total_ttc);
  const deficit = paidNum > 0 && paidNum < totals.total_ttc ? totals.total_ttc - paidNum : 0;
  const canConfirm = cart.items.length > 0 && !loading &&
    (method !== "cash" || paidNum >= totals.total_ttc || paidNum === 0);

  const handleConfirm = async () => {
    if (!canConfirm) return;
    setLoading(true);
    try {
      await onCheckout(method, paidNum || totals.total_ttc);
      setPaid("");
      setMethod("cash");
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      {/* ── Compact always-visible totals panel ──────────────────────── */}
      <Stack gap="sm" h="100%" p="md">
        <Paper withBorder p="md" radius="md" style={{ background: "var(--mantine-color-blue-0)" }}>
          <Stack gap={6}>
            <Group justify="space-between">
              <Text size="sm" c="dimmed">Sous-total HT</Text>
              <Text size="sm" ff="monospace">{totals.total_ht.toFixed(2)} DZD</Text>
            </Group>
            <Group justify="space-between">
              <Text size="sm" c="dimmed">TVA (19%)</Text>
              <Text size="sm" ff="monospace">{totals.total_vat.toFixed(2)} DZD</Text>
            </Group>
            {cart.discount_amount > 0 && (
              <Group justify="space-between">
                <Text size="sm" c="green.7">Remise</Text>
                <Text size="sm" ff="monospace" c="green.7">
                  − {cart.discount_amount.toFixed(2)} DZD
                </Text>
              </Group>
            )}
            <Divider my={4} />
            <Group justify="space-between" align="baseline">
              <Text fw={800} size="xl" c="blue.8">TOTAL TTC</Text>
              <Group gap={4} align="baseline">
                <Text fw={900} size="xl" ff="monospace" c="blue.8">
                  {totals.total_ttc.toFixed(2)}
                </Text>
                <Text size="sm" c="dimmed">DZD</Text>
              </Group>
            </Group>
          </Stack>
        </Paper>

        <Text size="xs" c="dimmed" ta="center">
          {totals.item_count} article{totals.item_count !== 1 ? "s" : ""}
        </Text>

        <NumberInput
          label="Remise globale (DZD)"
          value={cart.discount_amount || ""}
          onChange={(v) => setCartDiscount(Number(v) || 0)}
          min={0}
          step={10}
          decimalScale={2}
          placeholder="0.00"
          size="sm"
          leftSection={<IconDiscount size={14} />}
        />

        <div style={{ flex: 1 }} />

        <Stack gap="xs">
          <Button
            size="lg"
            fullWidth
            color="blue"
            disabled={cart.items.length === 0}
            onClick={onOpenPayment}
            rightSection={<Kbd size="xs">F12</Kbd>}
            style={{ height: 52, fontSize: 16 }}
          >
            Régler la vente
          </Button>
          <Button
            size="sm"
            fullWidth
            variant="subtle"
            color="red"
            leftSection={<IconX size={14} />}
            onClick={clearActiveCart}
            disabled={cart.items.length === 0}
          >
            Vider le panier
          </Button>
        </Stack>
      </Stack>

      {/* ── Payment modal ─────────────────────────────────────────────── */}
      <Modal
        opened={paymentOpen}
        onClose={onClosePayment}
        title={
          <Group gap="xs">
            <Text fw={700} size="lg">Règlement</Text>
            <Kbd size="sm">F12</Kbd>
          </Group>
        }
        size="md"
        radius="lg"
        centered
        overlayProps={{ backgroundOpacity: 0.45, blur: 3 }}
      >
        <Stack gap="lg">
          {/* Big total */}
          <Paper p="lg" radius="md" style={{
            background: "linear-gradient(135deg, var(--mantine-color-blue-7), var(--mantine-color-blue-9))",
            textAlign: "center",
          }}>
            <Text size="sm" c="blue.2" fw={500}>MONTANT À RÉGLER</Text>
            <Text size="md" fw={900} c="white" ff="monospace" mt={4} lh={1}>
              {totals.total_ttc.toFixed(2)}
            </Text>
            <Text size="lg" c="blue.2">DZD</Text>
            <Group justify="center" mt="sm" gap="xs">
              <Badge variant="light" color="blue" size="sm">HT: {totals.total_ht.toFixed(2)} DZD</Badge>
              <Badge variant="light" color="blue" size="sm">TVA: {totals.total_vat.toFixed(2)} DZD</Badge>
            </Group>
          </Paper>

          {/* Method selector */}
          <Stack gap={6}>
            <Text size="xs" fw={700} c="dimmed" tt="uppercase">Mode de paiement</Text>
            <SimpleGrid cols={4} spacing="xs">
              {PAYMENT_METHODS.map((m) => (
                <Paper
                  key={m.value}
                  withBorder
                  p="sm"
                  radius="md"
                  onClick={() => setMethod(m.value)}
                  style={{
                    cursor: "pointer",
                    textAlign: "center",
                    border: method === m.value
                      ? "2px solid var(--mantine-color-blue-5)"
                      : "2px solid var(--mantine-color-gray-3)",
                    background: method === m.value ? "var(--mantine-color-blue-0)" : undefined,
                    transition: "all 120ms",
                  }}
                >
                  <Stack gap={4} align="center">
                    <Text c={method === m.value ? "blue" : "dimmed"}>{m.icon}</Text>
                    <Text size="xs" fw={method === m.value ? 700 : 400}>{m.label}</Text>
                  </Stack>
                </Paper>
              ))}
            </SimpleGrid>
          </Stack>

          {/* Cash input */}
          {method === "cash" && (
            <Stack gap="sm">
              <NumberInput
                label="Montant remis par le client"
                placeholder={totals.total_ttc.toFixed(2)}
                value={paid}
                onChange={setPaid}
                min={0}
                step={50}
                decimalScale={2}
                fixedDecimalScale
                size="xl"
                leftSection={<IconCash size={18} />}
                styles={{ input: { fontSize: 28, fontWeight: 800, fontFamily: "monospace", textAlign: "right", height: 64 } }}
                autoFocus
              />
              <Group gap="xs">
                <Text size="xs" c="dimmed">Rapide :</Text>
                {QUICK_AMOUNTS.map((amt) => (
                  <Button key={amt} size="xs" variant="light" color="blue" onClick={() => setPaid(amt)}>
                    {amt.toLocaleString()} DZD
                  </Button>
                ))}
                <Button size="xs" variant="light" color="gray" onClick={() => setPaid(totals.total_ttc)}>
                  Exact
                </Button>
              </Group>
              {paidNum > 0 && (
                <Paper p="md" radius="md" style={{
                  background: change > 0 ? "var(--mantine-color-green-0)"
                    : deficit > 0 ? "var(--mantine-color-red-0)"
                    : "var(--mantine-color-blue-0)",
                  border: `1px solid ${change > 0 ? "var(--mantine-color-green-3)"
                    : deficit > 0 ? "var(--mantine-color-red-3)"
                    : "var(--mantine-color-blue-3)"}`,
                }}>
                  <Group justify="space-between">
                    <Text fw={700} c={change > 0 ? "green.7" : deficit > 0 ? "red.7" : "blue.7"}>
                      {change > 0 ? "💶 Monnaie à rendre" : deficit > 0 ? "⚠ Manque" : "✓ Montant exact"}
                    </Text>
                    <Text fw={900} size="xl" ff="monospace"
                      c={change > 0 ? "green.7" : deficit > 0 ? "red.7" : "blue.7"}
                    >
                      {(change > 0 ? change : deficit).toFixed(2)} DZD
                    </Text>
                  </Group>
                </Paper>
              )}
            </Stack>
          )}

          {method !== "cash" && (
            <Alert icon={<IconAlertCircle size={16} />} color="blue" variant="light" radius="md">
              Paiement par <strong>{PAYMENT_METHODS.find(m => m.value === method)?.label}</strong> — 
              confirmez après validation sur le terminal TPE.
            </Alert>
          )}

          <Group grow>
            <Button variant="subtle" color="gray" size="md" leftSection={<IconX size={16} />} onClick={onClosePayment}>
              Annuler <Kbd size="xs" ml={4}>Échap</Kbd>
            </Button>
            <Button
              size="md"
              color="green"
              leftSection={loading ? undefined : <IconCheck size={16} />}
              loading={loading}
              disabled={!canConfirm}
              onClick={handleConfirm}
              style={{ height: 48, fontSize: 16 }}
            >
              Confirmer <Kbd size="xs" ml={4}>F12</Kbd>
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}