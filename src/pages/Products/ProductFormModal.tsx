// src/pages/Products/ProductFormModal.tsx
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Modal, Stack, Group, TextInput, NumberInput, Select,
  Button, Text, Divider, Paper, Badge,
} from "@mantine/core";
import { IconTag, IconAlertCircle } from "@tabler/icons-react";
import type { Product, CreateProductInput } from "@/types";

interface Props {
  opened:  boolean;
  product: Product | null;   // null = create mode
  onSave:  (input: CreateProductInput, id?: number) => Promise<void>;
  onClose: () => void;
}

const CATEGORY_DATA = [
  { value: "1", label: "Divers" },
  { value: "2", label: "Épicerie" },
  { value: "3", label: "Boulangerie" },
  { value: "4", label: "Boucherie / Deli" },
  { value: "5", label: "Produits laitiers" },
  { value: "6", label: "Boissons" },
  { value: "7", label: "Hygiène" },
  { value: "8", label: "Surgelés" },
  { value: "9", label: "Légumes & Fruits" },
];

const UNIT_DATA = [
  { value: "1", label: "Pièce" },
  { value: "2", label: "Kg" },
  { value: "3", label: "Litre" },
  { value: "4", label: "Carton" },
  { value: "5", label: "Gramme" },
  { value: "6", label: "Paquet" },
];

const VAT_DATA = [
  { value: "0",    label: "0% — Exonéré" },
  { value: "0.09", label: "9% — Taux réduit" },
  { value: "0.19", label: "19% — Taux normal" },
];

const BLANK: CreateProductInput = {
  gtin:            null,
  name_fr:         "",
  name_ar:         "",
  category_id:     1,
  unit_id:         1,
  sell_price:      0,
  buy_price:       0,
  vat_rate:        0.19,
  min_stock_alert: 5,
};

export function ProductFormModal({ opened, product, onSave, onClose }: Props) {
  const { t } = useTranslation();

  const [form,   setForm]   = useState<CreateProductInput>(BLANK);
  const [errors, setErrors] = useState<Partial<Record<keyof CreateProductInput, string>>>({});
  const [saving, setSaving] = useState(false);

  // Populate form from product prop
  useEffect(() => {
    if (opened) {
      setErrors({});
      setForm(product
        ? {
            gtin:            product.gtin,
            name_fr:         product.name_fr,
            name_ar:         product.name_ar,
            category_id:     product.category_id,
            unit_id:         product.unit_id,
            sell_price:      product.sell_price,
            buy_price:       product.buy_price,
            vat_rate:        product.vat_rate,
            min_stock_alert: product.min_stock_alert,
          }
        : BLANK
      );
    }
  }, [opened, product]);

  const set = <K extends keyof CreateProductInput>(key: K, val: CreateProductInput[K]) =>
    setForm((f) => ({ ...f, [key]: val }));

  const validate = (): boolean => {
    const e: typeof errors = {};
    if (!form.name_fr.trim()) e.name_fr    = "Nom (FR) requis";
    if (form.sell_price < 0)  e.sell_price = "Prix invalide";
    if (form.buy_price < 0)   e.buy_price  = "Prix d'achat invalide";
    setErrors(e);
    return Object.keys(e).length === 0;
  };

  const handleSubmit = async () => {
    if (!validate() || saving) return;
    setSaving(true);
    try {
      await onSave(form, product?.id);
    } finally {
      setSaving(false);
    }
  };

  // Margin calculation
  const margin     = form.sell_price > 0 && form.buy_price > 0
    ? form.sell_price - form.buy_price
    : null;
  const marginPct  = margin !== null && form.buy_price > 0
    ? ((margin / form.buy_price) * 100)
    : null;

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconTag size={18} />
          <Text fw={700}>
            {product ? t("products.edit") : t("products.add")}
          </Text>
        </Group>
      }
      size="lg"
      radius="md"
      centered
      scrollAreaComponent={Modal.NativeScrollArea}
      overlayProps={{ backgroundOpacity: 0.4, blur: 2 }}
    >
      <Stack gap="md">

        {/* ── Barcode ────────────────────────────────────────────────── */}
        <TextInput
          label={t("products.gtin")}
          placeholder="Ex: 6191234567890"
          value={form.gtin ?? ""}
          onChange={(e) => set("gtin", e.target.value || null)}
          ff="monospace"
        />

        {/* ── Names ──────────────────────────────────────────────────── */}
        <Group grow align="flex-start">
          <TextInput
            label={
              <Text size="sm" fw={500}>
                {t("products.name_fr")} <Text span c="red">*</Text>
              </Text>
            }
            placeholder="Nom en français"
            value={form.name_fr}
            onChange={(e) => set("name_fr", e.target.value)}
            error={errors.name_fr}
            autoFocus
          />
          <TextInput
            label={
              <Text size="sm" fw={500} style={{ direction: "rtl" }}>
                {t("products.name_ar")}
              </Text>
            }
            placeholder="الاسم بالعربية"
            value={form.name_ar}
            onChange={(e) => set("name_ar", e.target.value)}
            styles={{ input: { textAlign: "right", direction: "rtl" } }}
          />
        </Group>

        {/* ── Category + Unit ────────────────────────────────────────── */}
        <Group grow>
          <Select
            label={t("products.category")}
            data={CATEGORY_DATA}
            value={String(form.category_id ?? 1)}
            onChange={(v) => set("category_id", v ? parseInt(v) : null)}
            searchable
          />
          <Select
            label={t("products.unit")}
            data={UNIT_DATA}
            value={String(form.unit_id ?? 1)}
            onChange={(v) => set("unit_id", v ? parseInt(v) : null)}
          />
        </Group>

        {/* ── Prices + VAT ───────────────────────────────────────────── */}
        <Group grow align="flex-start">
          <NumberInput
            label={
              <Text size="sm" fw={500}>
                {t("products.sell_price")} <Text span c="red">*</Text>
              </Text>
            }
            value={form.sell_price}
            onChange={(v) => set("sell_price", typeof v === "number" ? v : 0)}
            min={0}
            step={10}
            decimalScale={2}
            fixedDecimalScale
            rightSection={<Text size="xs" c="dimmed" pr={4}>DZD</Text>}
            rightSectionWidth={40}
            error={errors.sell_price}
          />
          <NumberInput
            label={t("products.buy_price")}
            value={form.buy_price}
            onChange={(v) => set("buy_price", typeof v === "number" ? v : 0)}
            min={0}
            step={10}
            decimalScale={2}
            fixedDecimalScale
            rightSection={<Text size="xs" c="dimmed" pr={4}>DZD</Text>}
            rightSectionWidth={40}
            error={errors.buy_price}
          />
          <Select
            label={t("products.vat_rate")}
            data={VAT_DATA}
            value={String(form.vat_rate)}
            onChange={(v) => set("vat_rate", v ? parseFloat(v) : 0.19)}
          />
        </Group>

        {/* ── Margin indicator ───────────────────────────────────────── */}
        {margin !== null && (
          <Paper
            p="sm"
            radius="md"
            style={{ background: "var(--mantine-color-blue-0)", border: "1px solid var(--mantine-color-blue-2)" }}
          >
            <Group gap="sm">
              <Text size="sm" c="dimmed">Marge brute :</Text>
              <Badge
                color={margin > 0 ? "green" : "red"}
                variant="light"
                ff="monospace"
              >
                {margin.toFixed(2)} DZD
              </Badge>
              {marginPct !== null && (
                <Badge
                  color={marginPct > 0 ? "green" : "red"}
                  variant="light"
                >
                  {marginPct.toFixed(1)}%
                </Badge>
              )}
            </Group>
          </Paper>
        )}

        {/* ── Min stock alert ────────────────────────────────────────── */}
        <NumberInput
          label={t("products.min_stock")}
          description="Une alerte apparaît quand le stock descend sous ce seuil."
          value={form.min_stock_alert}
          onChange={(v) => set("min_stock_alert", typeof v === "number" ? v : 5)}
          min={0}
          step={1}
          style={{ maxWidth: 220 }}
        />

        {/* ── Actions ────────────────────────────────────────────────── */}
        <Divider />
        <Group justify="flex-end">
          <Button variant="subtle" color="gray" onClick={onClose}>
            {t("products.cancel")}
          </Button>
          <Button onClick={handleSubmit} loading={saving}>
            {t("products.save")}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}