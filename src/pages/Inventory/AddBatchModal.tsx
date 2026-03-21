// src/pages/Inventory/AddBatchModal.tsx
import { useEffect, useState } from "react";
import {
  Modal, Stack, Select, Group, NumberInput,
  TextInput, Button, Text,
} from "@mantine/core";
import { IconPackage } from "@tabler/icons-react";
import type { AddBatchInput } from "@/lib/commands";
import type { Product } from "@/types";
import * as cmd from "@/lib/commands";

interface Props {
  opened:  boolean;
  onSave:  (input: AddBatchInput) => Promise<void>;
  onClose: () => void;
}

export function AddBatchModal({ opened, onSave, onClose }: Props) {
  const [products,  setProducts]  = useState<Product[]>([]);
  const [productId, setProductId] = useState<string | null>(null);
  const [qty,       setQty]       = useState<number | string>(1);
  const [expiry,    setExpiry]    = useState("");
  const [supplier,  setSupplier]  = useState("");
  const [cost,      setCost]      = useState<number | string>("");
  const [saving,    setSaving]    = useState(false);

  useEffect(() => {
    if (opened) {
      cmd.getProducts().then(setProducts).catch(console.error);
      setProductId(null);
      setQty(1);
      setExpiry("");
      setSupplier("");
      setCost("");
    }
  }, [opened]);

  const handleSave = async () => {
    if (!productId || !qty || saving) return;
    setSaving(true);
    try {
      await onSave({
        product_id:   parseInt(productId),
        quantity:     typeof qty === "number" ? qty : parseFloat(String(qty)),
        expiry_date:  expiry || null,
        supplier_ref: supplier || null,
        cost_price:   cost
          ? (typeof cost === "number" ? cost : parseFloat(String(cost)))
          : null,
      });
    } finally {
      setSaving(false);
    }
  };

  const productData = products.map((p) => ({
    value: String(p.id),
    label: p.gtin ? `${p.name_fr} (${p.gtin})` : p.name_fr,
  }));

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconPackage size={18} />
          <Text fw={700}>Entrée de stock</Text>
        </Group>
      }
      size="md"
      radius="md"
      centered
      overlayProps={{ backgroundOpacity: 0.4, blur: 2 }}
    >
      <Stack gap="md">
        {/* Product */}
        <Select
          label="Produit"
          placeholder="Sélectionner un produit…"
          data={productData}
          value={productId}
          onChange={setProductId}
          searchable
          required
          autoFocus
        />

        {/* Quantity + Cost */}
        <Group grow>
          <NumberInput
            label="Quantité"
            value={qty}
            onChange={setQty}
            min={0.01}
            step={1}
            decimalScale={2}
            required
          />
          <NumberInput
            label="Prix d'achat (DZD)"
            placeholder="Optionnel"
            value={cost}
            onChange={setCost}
            min={0}
            step={10}
            decimalScale={2}
            rightSection={<Text size="xs" c="dimmed" pr={4}>DZD</Text>}
            rightSectionWidth={40}
          />
        </Group>

        {/* Expiry date — native date input styled like a TextInput */}
        <TextInput
          label="Date d'expiration"
          placeholder="aaaa-mm-jj"
          type="date"
          value={expiry}
          onChange={(e) => setExpiry(e.target.value)}
          min={new Date().toISOString().slice(0, 10)}
        />

        {/* Supplier ref */}
        <TextInput
          label="Référence fournisseur"
          placeholder="Optionnel"
          value={supplier}
          onChange={(e) => setSupplier(e.target.value)}
        />

        {/* Actions */}
        <Group justify="flex-end" pt="xs">
          <Button variant="subtle" color="gray" onClick={onClose}>
            Annuler
          </Button>
          <Button
            onClick={handleSave}
            loading={saving}
            disabled={!productId || !qty}
          >
            Enregistrer l'entrée
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}