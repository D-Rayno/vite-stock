// src/pages/POS/CartTabBar.tsx
import { ActionIcon, Badge, Button, Group, Text, Tooltip } from "@mantine/core";
import { IconClock, IconPlus, IconShoppingCart, IconX } from "@tabler/icons-react";
import { computeTotals } from "@/store/cartStore";
import type { Cart } from "@/types";

interface Props {
  carts:    Cart[];
  activeId: string;
  onSelect: (id: string) => void;
  onAdd:    () => void;
  onRemove: (id: string) => void;
}

export function CartTabBar({ carts, activeId, onSelect, onAdd, onRemove }: Props) {
  return (
    <Group
      gap={0}
      px="xs"
      py={4}
      style={{
        background: "var(--mantine-color-dark-8)",
        borderBottom: "1px solid var(--mantine-color-dark-6)",
        overflowX: "auto",
        flexWrap: "nowrap",
        minHeight: 44,
      }}
    >
      {carts.map((cart) => {
        const t        = computeTotals(cart);
        const isActive = cart.id === activeId;
        const isWait   = cart.label.startsWith("Attente");

        return (
          <Group key={cart.id} gap={0} style={{ position: "relative", flexShrink: 0 }}>
            <Button
              size="xs"
              variant={isActive ? "filled" : "subtle"}
              color={isActive ? "blue" : "gray"}
              leftSection={isWait ? <IconClock size={13}/> : <IconShoppingCart size={13}/>}
              onClick={() => onSelect(cart.id)}
              style={{
                borderRadius: 6,
                fontWeight: isActive ? 700 : 500,
                color: isActive ? undefined : "var(--mantine-color-gray-4)",
                paddingInline: 10,
                height: 32,
              }}
            >
              <Group gap={6} wrap="nowrap">
                <Text size="xs" truncate style={{ maxWidth: 90 }}>{cart.label}</Text>
                {t.item_count > 0 && (
                  <Badge size="xs" circle
                    color={isActive ? "white" : "blue"}
                    style={{ color: isActive ? "var(--mantine-color-blue-7)" : undefined }}
                  >
                    {t.item_count}
                  </Badge>
                )}
                {t.total_ttc > 0 && (
                  <Text size="xs" ff="monospace" c={isActive ? "blue.2" : "dimmed"} style={{ whiteSpace: "nowrap" }}>
                    {t.total_ttc.toFixed(0)} DZD
                  </Text>
                )}
              </Group>
            </Button>
            {carts.length > 1 && (
              <ActionIcon size={16} color="red" variant="transparent"
                onClick={(e) => { e.stopPropagation(); onRemove(cart.id); }}
                style={{ position: "absolute", top: -5, right: -4 }}
              >
                <IconX size={10}/>
              </ActionIcon>
            )}
          </Group>
        );
      })}

      <Tooltip label="Nouvelle attente (F5)" withArrow>
        <ActionIcon variant="subtle" color="gray" size="md" ml={4}
          onClick={onAdd}
          style={{ color: "var(--mantine-color-gray-5)", flexShrink: 0 }}
        >
          <IconPlus size={16}/>
        </ActionIcon>
      </Tooltip>

      <Text size="xs" c="dark.3" ml="auto" style={{ whiteSpace: "nowrap", paddingRight: 8, flexShrink: 0 }}>
        F5 · Tab · Shift+F5
      </Text>
    </Group>
  );
}