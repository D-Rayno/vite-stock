// src/pages/POS/CartItemRow.tsx
import {
  Table,
  NumberInput,
  ActionIcon,
  Text,
  Group,
  Badge,
  Tooltip,
} from "@mantine/core";
import { IconTrash, IconAlertTriangle } from "@tabler/icons-react";
import { useCartStore } from "@/store/cartStore";
import { getExpiryStatus } from "@/types";
import type { CartItem } from "@/types";

const EXPIRY_COLOR: Record<ReturnType<typeof getExpiryStatus>, string | undefined> = {
  expired:  "red",
  critical: "orange",
  warning:  "yellow",
  ok:       undefined,
  none:     undefined,
};

interface Props {
  item:  CartItem;
  index: number;
}

export function CartItemRow({ item, index }: Props) {
  const { updateQty, updatePrice, updateDiscount, removeItem } = useCartStore();
  const pid    = item.product_id;
  const expiry = getExpiryStatus(item.days_until_expiry);
  const expiryColor = EXPIRY_COLOR[expiry];

  return (
    <Table.Tr
      style={{
        background: index % 2 === 0
          ? "var(--mantine-color-gray-0)"
          : "var(--mantine-color-white)",
        borderLeft: expiryColor
          ? `3px solid var(--mantine-color-${expiryColor}-5)`
          : "3px solid transparent",
        transition: "background 80ms",
      }}
    >
      {/* Product name + expiry badge */}
      <Table.Td style={{ maxWidth: 220 }}>
        <Text fw={600} size="sm" truncate="end" title={item.product_name}>
          {item.product_name}
        </Text>
        <Group gap={4} mt={2} wrap="nowrap">
          {item.product_gtin && (
            <Text size="xs" c="dimmed" ff="monospace">{item.product_gtin}</Text>
          )}
          {expiryColor && (
            <Tooltip
              label={
                expiry === "expired"
                  ? "Produit expiré !"
                  : `Expire le ${item.expiry_date} (dans ${item.days_until_expiry}j)`
              }
              withArrow
            >
              <Badge
                color={expiryColor}
                variant="light"
                size="xs"
                leftSection={<IconAlertTriangle size={10} />}
              >
                {expiry === "expired" ? "Expiré" : `${item.days_until_expiry}j`}
              </Badge>
            </Tooltip>
          )}
        </Group>
      </Table.Td>

      {/* Quantity */}
      <Table.Td style={{ width: 140 }}>
        <NumberInput
          value={item.quantity}
          onChange={(v) => updateQty(pid, Number(v) || 0)}
          min={0.01}
          step={1}
          decimalScale={2}
          size="xs"
          styles={{
            input: { textAlign: "center", fontWeight: 700, fontSize: 14 },
          }}
          rightSection={
            item.unit_label
              ? <Text size="xs" c="dimmed" pr={4}>{item.unit_label}</Text>
              : null
          }
          rightSectionWidth={item.unit_label ? 36 : undefined}
        />
      </Table.Td>

      {/* Unit price */}
      <Table.Td style={{ width: 120 }}>
        <NumberInput
          value={item.unit_price}
          onChange={(v) => updatePrice(pid, Number(v) || 0)}
          min={0}
          step={0.5}
          decimalScale={2}
          fixedDecimalScale
          size="xs"
          styles={{ input: { textAlign: "right", fontFamily: "monospace" } }}
        />
      </Table.Td>

      {/* Discount % */}
      <Table.Td style={{ width: 90 }}>
        <NumberInput
          value={item.discount_pct}
          onChange={(v) => updateDiscount(pid, Number(v) || 0)}
          min={0}
          max={100}
          step={1}
          decimalScale={1}
          suffix="%"
          size="xs"
          styles={{ input: { textAlign: "right" } }}
        />
      </Table.Td>

      {/* Line total */}
      <Table.Td style={{ width: 110, textAlign: "right" }}>
        <Text fw={700} size="sm" ff="monospace" c="blue.7">
          {item.line_total.toFixed(2)}
        </Text>
        <Text size="xs" c="dimmed">DZD</Text>
      </Table.Td>

      {/* Remove */}
      <Table.Td style={{ width: 44, textAlign: "center" }}>
        <Tooltip label="Supprimer (Del)" withArrow position="left">
          <ActionIcon color="red" variant="subtle" size="sm" onClick={() => removeItem(pid)}>
            <IconTrash size={14} />
          </ActionIcon>
        </Tooltip>
      </Table.Td>
    </Table.Tr>
  );
}