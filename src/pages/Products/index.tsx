// src/pages/Products/index.tsx
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Stack, Group, Title, Button, TextInput, Paper, Badge,
  Text, Table, ActionIcon, Tooltip, Center, Loader,
  ScrollArea, Modal,
} from "@mantine/core";
import {
  IconPlus, IconSearch, IconPencil, IconTrash,
  IconTag, IconAlertTriangle,
} from "@tabler/icons-react";
import { notifications }  from "@mantine/notifications";
import type { Product, CreateProductInput } from "@/types";
import * as cmd from "@/lib/commands";
import { ProductFormModal } from "./ProductFormModal";

export default function ProductsPage() {
  const { t } = useTranslation();

  const [products,     setProducts]     = useState<Product[]>([]);
  const [filtered,     setFiltered]     = useState<Product[]>([]);
  const [search,       setSearch]       = useState("");
  const [loading,      setLoading]      = useState(true);
  const [formOpen,     setFormOpen]     = useState(false);
  const [editing,      setEditing]      = useState<Product | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Product | null>(null);

  // ── Load ──────────────────────────────────────────────────────────────────

  const loadProducts = useCallback(async () => {
    setLoading(true);
    try {
      const data = await cmd.getProducts();
      setProducts(data);
      setFiltered(data);
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadProducts(); }, [loadProducts]);

  // ── Search filter ─────────────────────────────────────────────────────────

  useEffect(() => {
    const q = search.toLowerCase();
    if (!q) { setFiltered(products); return; }
    setFiltered(
      products.filter((p) =>
        p.name_fr.toLowerCase().includes(q) ||
        p.name_ar.includes(q) ||
        (p.gtin ?? "").includes(q) ||
        (p.category_name_fr ?? "").toLowerCase().includes(q),
      ),
    );
  }, [search, products]);

  // ── CRUD ──────────────────────────────────────────────────────────────────

  const handleOpenCreate = () => { setEditing(null); setFormOpen(true); };
  const handleOpenEdit   = (p: Product) => { setEditing(p); setFormOpen(true); };

  const handleSave = async (input: CreateProductInput, id?: number) => {
    try {
      if (id && editing) {
        // cmd.updateProduct expects the full Product shape.
        // We preserve the read-only fields (category_name_fr, unit_label_fr,
        // total_stock, created_at) from the original editing record so that
        // the TypeScript Product type is fully satisfied.
        const updated: Product = {
          // Read-only / computed fields — keep original values
          id,
          is_active:        editing.is_active,
          total_stock:      editing.total_stock,
          created_at:       editing.created_at,
          category_name_fr: editing.category_name_fr,
          unit_label_fr:    editing.unit_label_fr,
          // Editable fields from the form
          gtin:             input.gtin,
          name_fr:          input.name_fr,
          name_ar:          input.name_ar,
          category_id:      input.category_id,
          unit_id:          input.unit_id,
          sell_price:       input.sell_price,
          buy_price:        input.buy_price,
          vat_rate:         input.vat_rate,
          min_stock_alert:  input.min_stock_alert,
        };
        await cmd.updateProduct(updated);
        notifications.show({ color: "green", message: "Produit mis à jour." });
      } else {
        await cmd.createProduct(input);
        notifications.show({ color: "green", message: "Produit créé avec succès." });
      }
      setFormOpen(false);
      setEditing(null);
      loadProducts();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await cmd.deleteProduct(deleteTarget.id);
      notifications.show({ color: "teal", message: "Produit désactivé." });
      setDeleteTarget(null);
      loadProducts();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    }
  };

  // ── Stock badge ───────────────────────────────────────────────────────────

  const stockBadge = (p: Product) => {
    if (p.total_stock <= 0)
      return <Badge color="red"    variant="light" size="xs">Rupture</Badge>;
    if (p.total_stock <= p.min_stock_alert)
      return <Badge color="orange" variant="light" size="xs">Bas</Badge>;
    return   <Badge color="green"  variant="light" size="xs">OK</Badge>;
  };

  // ── Stats ─────────────────────────────────────────────────────────────────

  const alertCount   = products.filter(p => p.total_stock > 0  && p.total_stock <= p.min_stock_alert).length;
  const ruptureCount = products.filter(p => p.total_stock <= 0 && p.is_active).length;

  return (
    <Stack gap="lg" p="lg" style={{ height: "100vh", overflow: "hidden" }}>

      {/* ── Header ───────────────────────────────────────────────────── */}
      <Group justify="space-between">
        <Title order={2}>{t("products.title")}</Title>
        <Button leftSection={<IconPlus size={16} />} onClick={handleOpenCreate}>
          {t("products.add")}
        </Button>
      </Group>

      {/* ── Search + stats ───────────────────────────────────────────── */}
      <Group justify="space-between">
        <TextInput
          placeholder={t("products.search")}
          leftSection={<IconSearch size={16} />}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          style={{ maxWidth: 360 }}
          autoFocus
        />
        <Group gap="xs">
          <Text size="xs" c="dimmed">{products.length} produits</Text>
          {alertCount > 0 && (
            <Badge
              color="orange"
              variant="light"
              size="sm"
              leftSection={<IconAlertTriangle size={10} />}
            >
              {alertCount} alerte{alertCount > 1 ? "s" : ""} stock
            </Badge>
          )}
          {ruptureCount > 0 && (
            <Badge color="red" variant="light" size="sm">
              {ruptureCount} rupture{ruptureCount > 1 ? "s" : ""}
            </Badge>
          )}
        </Group>
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
                  <Table.Th>{t("products.gtin")}</Table.Th>
                  <Table.Th>{t("products.category")}</Table.Th>
                  <Table.Th style={{ textAlign: "right" }}>{t("products.sell_price")}</Table.Th>
                  <Table.Th style={{ textAlign: "right" }}>{t("products.buy_price")}</Table.Th>
                  <Table.Th style={{ textAlign: "center" }}>{t("products.stock")}</Table.Th>
                  <Table.Th style={{ textAlign: "center" }}>Statut</Table.Th>
                  <Table.Th style={{ width: 80 }} />
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {filtered.length === 0 ? (
                  <Table.Tr>
                    <Table.Td colSpan={8}>
                      <Center py="xl">
                        <Stack align="center" gap="xs" opacity={0.5}>
                          <IconTag size={40} stroke={1} />
                          <Text c="dimmed">Aucun produit trouvé</Text>
                        </Stack>
                      </Center>
                    </Table.Td>
                  </Table.Tr>
                ) : (
                  filtered.map((p) => (
                    <Table.Tr
                      key={p.id}
                      style={{ opacity: p.is_active ? 1 : 0.45 }}
                    >
                      {/* Name */}
                      <Table.Td>
                        <Text fw={600} size="sm">{p.name_fr}</Text>
                        {p.name_ar && (
                          <Text
                            size="xs"
                            c="dimmed"
                            style={{ direction: "rtl", textAlign: "right" }}
                          >
                            {p.name_ar}
                          </Text>
                        )}
                      </Table.Td>

                      {/* GTIN */}
                      <Table.Td>
                        <Text size="xs" ff="monospace" c="dimmed">
                          {p.gtin ?? "—"}
                        </Text>
                      </Table.Td>

                      {/* Category */}
                      <Table.Td>
                        <Text size="sm" c="dimmed">{p.category_name_fr ?? "—"}</Text>
                      </Table.Td>

                      {/* Sell price */}
                      <Table.Td style={{ textAlign: "right" }}>
                        <Text fw={600} size="sm" ff="monospace">
                          {p.sell_price.toFixed(2)}
                          <Text span size="xs" c="dimmed" fw={400}> DZD</Text>
                        </Text>
                      </Table.Td>

                      {/* Buy price */}
                      <Table.Td style={{ textAlign: "right" }}>
                        <Text size="sm" ff="monospace" c="dimmed">
                          {p.buy_price.toFixed(2)}
                        </Text>
                      </Table.Td>

                      {/* Stock */}
                      <Table.Td style={{ textAlign: "center" }}>
                        <Text ff="monospace" size="sm">
                          {p.total_stock % 1 === 0
                            ? p.total_stock.toFixed(0)
                            : p.total_stock.toFixed(2)}
                          {p.unit_label_fr && (
                            <Text span size="xs" c="dimmed"> {p.unit_label_fr}</Text>
                          )}
                        </Text>
                      </Table.Td>

                      {/* Status */}
                      <Table.Td style={{ textAlign: "center" }}>
                        {stockBadge(p)}
                      </Table.Td>

                      {/* Actions */}
                      <Table.Td>
                        <Group gap={4} justify="flex-end">
                          <Tooltip label="Modifier" withArrow>
                            <ActionIcon
                              variant="subtle"
                              color="blue"
                              size="sm"
                              onClick={() => handleOpenEdit(p)}
                            >
                              <IconPencil size={14} />
                            </ActionIcon>
                          </Tooltip>
                          <Tooltip label="Désactiver" withArrow>
                            <ActionIcon
                              variant="subtle"
                              color="red"
                              size="sm"
                              onClick={() => setDeleteTarget(p)}
                              disabled={!p.is_active}
                            >
                              <IconTrash size={14} />
                            </ActionIcon>
                          </Tooltip>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  ))
                )}
              </Table.Tbody>
            </Table>
          </ScrollArea>
        </Paper>
      )}

      {/* ── Product form modal ───────────────────────────────────────── */}
      <ProductFormModal
        opened={formOpen}
        product={editing}
        onSave={handleSave}
        onClose={() => { setFormOpen(false); setEditing(null); }}
      />

      {/* ── Delete confirm modal ─────────────────────────────────────── */}
      <Modal
        opened={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        title={<Text fw={700}>Désactiver ce produit ?</Text>}
        size="sm"
        centered
        radius="md"
      >
        <Stack gap="md">
          <Text size="sm" c="dimmed">
            <Text span fw={600}>{deleteTarget?.name_fr}</Text> sera masqué de la caisse
            mais conservé dans l'historique des ventes.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" color="gray" onClick={() => setDeleteTarget(null)}>
              Annuler
            </Button>
            <Button color="red" onClick={handleDelete}>
              Désactiver
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}