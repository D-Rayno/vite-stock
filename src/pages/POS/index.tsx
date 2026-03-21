// src/pages/POS/index.tsx
// Smart POS Checkout — keyboard-first, scanner-driven, Mantine UI.
//
// Layout:
//   TOP     — scanner indicator + F-key legend + cart tab bar
//   LEFT    — cart items table (fills remaining height)
//   RIGHT   — totals + payment panel (fixed 320px)

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Box,
  Center,
  Divider,
  Group,
  Kbd,
  Notification,
  Paper,
  ScrollArea,
  Stack,
  Table,
  Text,
  TextInput,
  Tooltip,
  Badge,
  Button,
  Loader,
  ActionIcon,
} from "@mantine/core";
import {
  IconBarcode,
  IconSearch,
  IconShoppingCartOff,
  IconAlertTriangle,
  IconX,
} from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";

import { useBarcodeScanner } from "@/providers/BarcodeProvider";
import { useCartStore } from "@/store/cartStore";
import { useLicenseStore } from "@/store/licenseStore";
import { useHotkeys } from "@/hooks/useHotkeys";
import { Features, type Product, type ProductLookupResult } from "@/types";
import * as cmd from "@/lib/commands";

import { ScannerIndicator } from "./ScannerIndicator";
import { CartItemRow } from "./CartItemRow";
import { CartTabBar } from "./CartTabBar";
import { PaymentPanel } from "./PaymentPanel";
import { ProductSearchModal } from "./ProductSearchModal";

// ─────────────────────────────────────────────────────────────────────────────

export default function POSPage() {
  const { t } = useTranslation();
  const can = useLicenseStore((s) => s.can);

  const {
    carts, activeId, activeCart, totals,
    addFromLookup, addCart, removeCart, setActive,
    nextCart, prevCart,
  } = useCartStore();

  const [paymentOpen,  setPaymentOpen]  = useState(false);
  const [searchOpen,   setSearchOpen]   = useState(false);
  const [lastScanInfo, setLastScanInfo] = useState<{
    code: string; status: "ok" | "err"; name?: string;
  } | null>(null);

  const cart   = activeCart();
  const totals_ = totals();

  // ── Barcode scan handler ─────────────────────────────────────────────────

  const handleScan = useCallback(async (barcode: string) => {
    try {
      const result = await cmd.lookupProduct(barcode);

      if (!result) {
        setLastScanInfo({ code: barcode, status: "err" });
        notifications.show({
          title: "Code inconnu",
          message: `Aucun produit trouvé pour : ${barcode}`,
          color: "red",
          icon: <IconAlertTriangle size={16} />,
          autoClose: 3000,
        });
        return;
      }

      // Warn on near-expiry batches
      if (result.days_until_expiry !== null && result.days_until_expiry <= 0) {
        notifications.show({
          title: "⚠ Produit expiré",
          message: `${result.name_fr} — expiré depuis ${Math.abs(result.days_until_expiry)} jour(s)`,
          color: "red",
          autoClose: 5000,
        });
      } else if (result.days_until_expiry !== null && result.days_until_expiry <= 7) {
        notifications.show({
          title: "⚠ Expiration proche",
          message: `${result.name_fr} — expire dans ${result.days_until_expiry} jour(s)`,
          color: "orange",
          autoClose: 4000,
        });
      }

      addFromLookup(result);
      setLastScanInfo({ code: barcode, status: "ok", name: result.name_fr });

    } catch (e) {
      notifications.show({
        title: "Erreur scanner",
        message: String(e),
        color: "red",
        autoClose: 4000,
      });
    }
  }, [addFromLookup]);

  useBarcodeScanner(handleScan, !paymentOpen && !searchOpen);

  // ── Manual search fallback — add product by name ─────────────────────────

  const handleManualSelect = useCallback(async (product: Product) => {
    // Do a FEFO lookup using the product's GTIN, or build a minimal result
    if (product.gtin) {
      const result = await cmd.lookupProduct(product.gtin).catch(() => null);
      if (result) { addFromLookup(result); return; }
    }
    // Fallback: build a minimal ProductLookupResult from the Product row
    const fallback: ProductLookupResult = {
      id:                product.id,
      gtin:              product.gtin,
      name_fr:           product.name_fr,
      name_ar:           product.name_ar,
      sell_price:        product.sell_price,
      vat_rate:          product.vat_rate,
      unit_label_fr:     product.unit_label_fr,
      total_stock:       product.total_stock,
      batch_id:          null,
      batch_qty:         null,
      expiry_date:       null,
      days_until_expiry: null,
    };
    addFromLookup(fallback);
  }, [addFromLookup]);

  // ── Checkout ──────────────────────────────────────────────────────────────

  const handleCheckout = useCallback(async (
    method: import("@/types").PaymentMethod,
    paid: number,
  ) => {
    if (cart.items.length === 0) return;
    try {
      const result = await cmd.createTransaction({
        customer_id:     cart.customer_id,
        items:           cart.items.map((i) => ({
          product_id:   i.product_id,
          batch_id:     i.batch_id,
          quantity:     i.quantity,
          unit_price:   i.unit_price,
          vat_rate:     i.vat_rate,
          discount_pct: i.discount_pct,
        })),
        discount_amount: cart.discount_amount,
        payment_method:  method,
        amount_paid:     paid,
        cashier_name:    "Admin",
        notes:           null,
      });

      useCartStore.getState().clearActiveCart();
      setPaymentOpen(false);
      setLastScanInfo(null);

      notifications.show({
        title: "✓ Vente enregistrée",
        message: `${result.ref_number} — Rendu: ${result.change_given.toFixed(2)} DZD`,
        color: "green",
        autoClose: 5000,
      });
    } catch (e) {
      notifications.show({
        title: "Erreur de transaction",
        message: String(e),
        color: "red",
        autoClose: 0,
      });
    }
  }, [cart]);

  // ── F-key hotkeys ─────────────────────────────────────────────────────────

  useHotkeys({
    "F12": () => { if (cart.items.length > 0) setPaymentOpen(true); },
    "F3":  () => setSearchOpen(true),
    "F5":  () => {
      if (can(Features.MULTI_CART)) {
        addCart();
      } else {
        notifications.show({
          message: "Multi-caisse non disponible sur cette licence.",
          color: "orange",
        });
      }
    },
    "Shift+F5": () => prevCart(),
    "Tab":      () => { if (!paymentOpen && !searchOpen) nextCart(); },
    "Escape":   () => {
      if (paymentOpen) setPaymentOpen(false);
      else if (searchOpen) setSearchOpen(false);
    },
  });

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <Box style={{ display: "flex", flexDirection: "column", height: "100vh", overflow: "hidden" }}>

      {/* ── Top bar: scanner status + hotkey legend ──────────────────── */}
      <Group
        px="md"
        py="xs"
        style={{
          background: "var(--mantine-color-dark-7)",
          borderBottom: "1px solid var(--mantine-color-dark-5)",
          flexShrink: 0,
          gap: 12,
        }}
      >
        <ScannerIndicator />

        <Divider orientation="vertical" color="dark.5" />

        {/* Last scan mini-notification */}
        {lastScanInfo && (
          <Group gap={6}>
            {lastScanInfo.status === "ok" ? (
              <Badge color="green" variant="light" size="sm">
                ✓ {lastScanInfo.name}
              </Badge>
            ) : (
              <Badge color="red" variant="light" size="sm">
                ✕ Inconnu : {lastScanInfo.code}
              </Badge>
            )}
            <ActionIcon
              size="xs"
              variant="transparent"
              c="dimmed"
              onClick={() => setLastScanInfo(null)}
            >
              <IconX size={10} />
            </ActionIcon>
          </Group>
        )}

        {/* Spacer */}
        <div style={{ flex: 1 }} />

        {/* Hotkey legend */}
        <Group gap="xs" style={{ opacity: 0.6 }}>
          {[
            ["F3", "Recherche"],
            ["F5", "Attente"],
            ["F12", "Régler"],
            ["Tab", "Changer caisse"],
          ].map(([key, label]) => (
            <Group key={key} gap={4}>
              <Kbd size="xs">{key}</Kbd>
              <Text size="xs" c="dimmed">{label}</Text>
            </Group>
          ))}
        </Group>
      </Group>

      {/* ── Cart tab bar (multi-cart) ─────────────────────────────────── */}
      {can(Features.MULTI_CART) && (
        <CartTabBar
          carts={carts}
          activeId={activeId}
          onSelect={setActive}
          onAdd={addCart}
          onRemove={removeCart}
        />
      )}

      {/* ── Main body: cart table + payment panel ─────────────────────── */}
      <Box style={{ display: "flex", flex: 1, overflow: "hidden" }}>

        {/* LEFT: cart items ───────────────────────────────────────────── */}
        <Box style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>

          {/* Manual search bar */}
          <Group
            px="md"
            py="xs"
            style={{
              borderBottom: "1px solid var(--mantine-color-gray-2)",
              background: "var(--mantine-color-white)",
              flexShrink: 0,
            }}
          >
            <TextInput
              placeholder="F3 — Recherche manuelle par nom ou code-barres…"
              leftSection={<IconSearch size={16} />}
              rightSection={<Kbd size="xs">F3</Kbd>}
              style={{ flex: 1 }}
              readOnly
              onClick={() => setSearchOpen(true)}
              onFocus={() => setSearchOpen(true)}
              styles={{ input: { cursor: "pointer" } }}
            />
            <Button
              variant="light"
              leftSection={<IconSearch size={14} />}
              onClick={() => setSearchOpen(true)}
              size="sm"
            >
              Recherche <Kbd size="xs" ml={4}>F3</Kbd>
            </Button>
          </Group>

          {/* Items table */}
          {cart.items.length === 0 ? (
            <Center style={{ flex: 1 }}>
              <Stack align="center" gap="md" opacity={0.5}>
                <IconShoppingCartOff size={64} stroke={1} />
                <Text size="lg" fw={500}>Panier vide</Text>
                <Text size="sm" c="dimmed" ta="center">
                  Scannez un article ou appuyez sur{" "}
                  <Kbd size="xs">F3</Kbd> pour rechercher.
                </Text>
              </Stack>
            </Center>
          ) : (
            <ScrollArea style={{ flex: 1 }}>
              <Table
                stickyHeader
                withRowBorders
                highlightOnHover
                styles={{
                  th: {
                    background: "var(--mantine-color-gray-1)",
                    fontSize: 11,
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                    color: "var(--mantine-color-gray-6)",
                    padding: "8px 12px",
                  },
                  td: { padding: "6px 12px", verticalAlign: "middle" },
                }}
              >
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Produit</Table.Th>
                    <Table.Th style={{ width: 140 }}>Quantité</Table.Th>
                    <Table.Th style={{ width: 120, textAlign: "right" }}>Prix U.</Table.Th>
                    <Table.Th style={{ width: 90, textAlign: "right" }}>Remise</Table.Th>
                    <Table.Th style={{ width: 110, textAlign: "right" }}>Total TTC</Table.Th>
                    <Table.Th style={{ width: 44 }} />
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {cart.items.map((item, idx) => (
                    <CartItemRow key={item.product_id} item={item} index={idx} />
                  ))}
                </Table.Tbody>
              </Table>
            </ScrollArea>
          )}

          {/* Bottom status bar */}
          <Group
            px="md"
            py="xs"
            style={{
              borderTop: "1px solid var(--mantine-color-gray-2)",
              background: "var(--mantine-color-gray-0)",
              flexShrink: 0,
            }}
            justify="space-between"
          >
            <Text size="xs" c="dimmed">
              {cart.items.length} ligne{cart.items.length !== 1 ? "s" : ""} ·{" "}
              {totals_.item_count} article{totals_.item_count !== 1 ? "s" : ""}
            </Text>
            {cart.items.some(i =>
              i.days_until_expiry !== null && i.days_until_expiry <= 30
            ) && (
              <Badge
                color="orange"
                variant="light"
                size="xs"
                leftSection={<IconAlertTriangle size={10} />}
              >
                Articles avec expiration proche
              </Badge>
            )}
            <Text size="xs" c="dimmed">
              Scanner actif — appuyez sur <Kbd size="xs">F3</Kbd> pour recherche manuelle
            </Text>
          </Group>
        </Box>

        {/* RIGHT: payment panel ───────────────────────────────────────── */}
        <Box
          style={{
            width: 320,
            flexShrink: 0,
            borderLeft: "1px solid var(--mantine-color-gray-3)",
            background: "var(--mantine-color-white)",
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
          }}
        >
          <PaymentPanel
            totals={totals_}
            cart={cart}
            paymentOpen={paymentOpen}
            onOpenPayment={() => setPaymentOpen(true)}
            onClosePayment={() => setPaymentOpen(false)}
            onCheckout={handleCheckout}
          />
        </Box>
      </Box>

      {/* ── Product search modal ──────────────────────────────────────── */}
      <ProductSearchModal
        opened={searchOpen}
        onClose={() => setSearchOpen(false)}
        onSelect={handleManualSelect}
      />
    </Box>
  );
}