// src/pages/POS/ProductSearchModal.tsx
// Manual "Search by Name" fallback using Mantine's Modal + TextInput.
// Triggered by F3 or the "Recherche manuelle" button.

import { useEffect, useRef, useState } from "react";
import {
  Modal,
  TextInput,
  Stack,
  Group,
  Text,
  Badge,
  ScrollArea,
  Loader,
  Center,
  Kbd,
  ActionIcon,
  Tooltip,
} from "@mantine/core";
import { IconSearch, IconShoppingCartPlus, IconAlertTriangle } from "@tabler/icons-react";
import type { Product } from "@/types";
import * as cmd from "@/lib/commands";

interface Props {
  opened:  boolean;
  onClose: () => void;
  onSelect:(product: Product) => void;
}

export function ProductSearchModal({ opened, onClose, onSelect }: Props) {
  const [query,   setQuery]   = useState("");
  const [results, setResults] = useState<Product[]>([]);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Auto-focus input when modal opens
  useEffect(() => {
    if (opened) {
      setQuery("");
      setResults([]);
      setTimeout(() => inputRef.current?.focus(), 80);
    }
  }, [opened]);

  // Debounced search
  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (!query.trim()) { setResults([]); return; }
    setLoading(true);
    timerRef.current = setTimeout(async () => {
      try {
        const r = await cmd.searchProducts(query);
        setResults(r);
      } catch {
        setResults([]);
      } finally {
        setLoading(false);
      }
    }, 200);
  }, [query]);

  // Keyboard navigation: Enter on first result
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && results.length > 0) {
      e.preventDefault();
      onSelect(results[0]);
      onClose();
    }
    if (e.key === "Escape") {
      onClose();
    }
  };

  const stockBadge = (p: Product) => {
    if (p.total_stock <= 0)
      return <Badge color="red" variant="light" size="xs">Rupture</Badge>;
    if (p.total_stock <= p.min_stock_alert)
      return <Badge color="orange" variant="light" size="xs">Stock bas</Badge>;
    return null;
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconSearch size={18} />
          <Text fw={600}>Recherche produit</Text>
          <Kbd size="xs">F3</Kbd>
        </Group>
      }
      size="lg"
      radius="md"
      overlayProps={{ backgroundOpacity: 0.4, blur: 2 }}
    >
      <Stack gap="sm">
        <TextInput
          ref={inputRef}
          placeholder="Nom du produit, code-barres, catégorie…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          leftSection={<IconSearch size={16} />}
          rightSection={loading ? <Loader size="xs" /> : null}
          size="md"
          autoComplete="off"
        />

        <ScrollArea h={400} type="scroll">
          {results.length === 0 && query.trim() && !loading && (
            <Center h={120}>
              <Stack align="center" gap="xs">
                <IconAlertTriangle size={32} color="var(--mantine-color-orange-5)" />
                <Text size="sm" c="dimmed">Aucun produit trouvé pour « {query} »</Text>
              </Stack>
            </Center>
          )}

          {results.length === 0 && !query.trim() && (
            <Center h={120}>
              <Text size="sm" c="dimmed">Tapez pour rechercher…</Text>
            </Center>
          )}

          <Stack gap={4}>
            {results.map((p, idx) => (
              <Group
                key={p.id}
                p="sm"
                style={{
                  borderRadius: 8,
                  cursor: "pointer",
                  border: "1px solid var(--mantine-color-gray-2)",
                  background: idx === 0
                    ? "var(--mantine-color-blue-0)"
                    : "var(--mantine-color-white)",
                  transition: "background 100ms",
                }}
                justify="space-between"
                onClick={() => { onSelect(p); onClose(); }}
                onMouseEnter={(e) => {
                  (e.currentTarget as HTMLElement).style.background =
                    "var(--mantine-color-blue-0)";
                }}
                onMouseLeave={(e) => {
                  if (idx !== 0)
                    (e.currentTarget as HTMLElement).style.background =
                      "var(--mantine-color-white)";
                }}
              >
                <Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
                  <Group gap="xs">
                    <Text fw={600} size="sm" truncate>
                      {p.name_fr}
                    </Text>
                    {idx === 0 && (
                      <Badge size="xs" color="blue" variant="light">
                        ↵ Entrée
                      </Badge>
                    )}
                    {stockBadge(p)}
                  </Group>
                  <Group gap="xs">
                    {p.gtin && (
                      <Text size="xs" c="dimmed" ff="monospace">{p.gtin}</Text>
                    )}
                    {p.category_name_fr && (
                      <Text size="xs" c="dimmed">· {p.category_name_fr}</Text>
                    )}
                    <Text size="xs" c="dimmed">
                      · Stock: {p.total_stock % 1 === 0
                        ? p.total_stock.toFixed(0)
                        : p.total_stock.toFixed(2)}
                      {p.unit_label_fr ? ` ${p.unit_label_fr}` : ""}
                    </Text>
                  </Group>
                </Stack>

                <Group gap="md" wrap="nowrap">
                  <Text fw={700} size="md" c="blue.7" ff="monospace">
                    {p.sell_price.toFixed(2)}{" "}
                    <Text span size="xs" c="dimmed" fw={400}>DZD</Text>
                  </Text>
                  <Tooltip label="Ajouter au panier">
                    <ActionIcon
                      variant="light"
                      color="blue"
                      size="md"
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelect(p);
                        onClose();
                      }}
                    >
                      <IconShoppingCartPlus size={16} />
                    </ActionIcon>
                  </Tooltip>
                </Group>
              </Group>
            ))}
          </Stack>
        </ScrollArea>

        {results.length > 0 && (
          <Text size="xs" c="dimmed" ta="right">
            {results.length} résultat{results.length > 1 ? "s" : ""} — <Kbd size="xs">↵</Kbd> pour le premier
          </Text>
        )}
      </Stack>
    </Modal>
  );
}