// src/pages/Settings/PrinterSection.tsx
// Phase 4: extracted printer section with serial + network transport support.
// Drop-in replacement for the printer block in Settings/index.tsx.
import { useCallback, useState } from "react";
import {
  Paper, Group, Text, Select, TextInput, NumberInput,
  Button, Stack, ActionIcon, Tooltip, Badge, Alert,
} from "@mantine/core";
import { IconPrinter, IconRefresh, IconNetwork, IconPlug, IconTestPipe } from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";
import * as cmd from "@/lib/commands";
import type { PrintTarget } from "@/lib/commands";

type Transport = "Serial" | "Network";

interface PrinterSectionProps {
  widthValue:    string;
  onWidthChange: (v: string) => void;
}

export function PrinterSection({ widthValue, onWidthChange }: PrinterSectionProps) {
  const [transport,    setTransport]    = useState<Transport>("Serial");
  const [serialPort,   setSerialPort]   = useState("");
  const [serialBaud,   setSerialBaud]   = useState(9600);
  const [netHost,      setNetHost]      = useState("");
  const [netPort,      setNetPort]      = useState(9100);
  const [ports,        setPorts]        = useState<cmd.PrinterPort[]>([]);
  const [loadingPorts, setLoadingPorts] = useState(false);
  const [pinging,      setPinging]      = useState(false);
  const [pingResult,   setPingResult]   = useState<boolean | null>(null);
  const [testing,      setTesting]      = useState(false);

  const loadPorts = useCallback(async () => {
    setLoadingPorts(true);
    try {
      const list = await cmd.listPrinters();
      setPorts(list);
      const thermal = list.find(p => p.likely_thermal);
      if (thermal && !serialPort) setSerialPort(thermal.port);
    } catch (e) {
      notifications.show({ color: "orange", message: `Ports: ${e}` });
    } finally { setLoadingPorts(false); }
  }, [serialPort]);

  const pingNetwork = async () => {
    if (!netHost) return;
    setPinging(true); setPingResult(null);
    try {
      const ok = await cmd.testNetworkPrinter(netHost, netPort);
      setPingResult(ok);
      if (!ok) notifications.show({ color: "red", message: `Imprimante ${netHost}:${netPort} inaccessible.` });
    } catch { setPingResult(false); }
    finally { setPinging(false); }
  };

  const buildTarget = (): PrintTarget | null => {
    if (transport === "Serial" && serialPort) {
      return { transport: "Serial", port: serialPort, baud: serialBaud };
    }
    if (transport === "Network" && netHost) {
      return { transport: "Network", host: netHost, port: netPort };
    }
    return null;
  };

  const handleTest = async () => {
    const target = buildTarget();
    if (!target) {
      notifications.show({ color: "orange", message: "Configurez d'abord l'imprimante." });
      return;
    }
    setTesting(true);
    try {
      await cmd.printTestPage(target);
      notifications.show({ color: "green", message: "Page de test envoyée !" });
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally { setTesting(false); }
  };

  return (
    <Paper p="lg" radius="md" withBorder>
      <Group gap="xs" mb="md" justify="space-between">
        <Group gap="xs">
          <IconPrinter size={18} color="var(--mantine-color-blue-6)" />
          <Text fw={700}>Imprimante thermique</Text>
        </Group>
        <Badge variant="light" color={transport === "Network" ? "blue" : "gray"} size="sm">
          {transport === "Network" ? "Réseau TCP/IP" : "USB / Série"}
        </Badge>
      </Group>

      <Stack gap="sm">
        {/* Width selector */}
        <Select
          label="Largeur du ticket"
          data={[
            { value: "58", label: "58 mm (32 chars/ligne)" },
            { value: "80", label: "80 mm (48 chars/ligne)" },
          ]}
          value={widthValue}
          onChange={v => onWidthChange(v ?? "80")}
        />

        {/* Transport switcher */}
        <Group gap="xs">
          <Button
            size="xs"
            variant={transport === "Serial" ? "filled" : "light"}
            leftSection={<IconPlug size={12} />}
            onClick={() => { setTransport("Serial"); setPingResult(null); }}
          >
            USB / Série
          </Button>
          <Button
            size="xs"
            variant={transport === "Network" ? "filled" : "light"}
            leftSection={<IconNetwork size={12} />}
            onClick={() => { setTransport("Network"); setPingResult(null); }}
          >
            Réseau TCP/IP
          </Button>
        </Group>

        {/* Transport-specific config */}
        {transport === "Serial" ? (
          <Group align="flex-end" gap="sm">
            <Select
              label="Port série"
              placeholder="Sélectionner…"
              data={ports.map(p => ({
                value: p.port,
                label: `${p.port}${p.description ? ` — ${p.description}` : ""}${p.likely_thermal ? " 🖨" : ""}`,
              }))}
              value={serialPort}
              onChange={v => setSerialPort(v ?? "")}
              style={{ flex: 1 }}
            />
            <Select
              label="Baud rate"
              data={["9600", "19200", "38400", "57600", "115200"].map(b => ({ value: b, label: b }))}
              value={String(serialBaud)}
              onChange={v => setSerialBaud(parseInt(v ?? "9600"))}
              style={{ width: 110 }}
            />
            <Tooltip label="Détecter les imprimantes" withArrow>
              <ActionIcon variant="light" size="lg" onClick={loadPorts} loading={loadingPorts}>
                <IconRefresh size={16} />
              </ActionIcon>
            </Tooltip>
          </Group>
        ) : (
          <Stack gap="xs">
            <Group align="flex-end" gap="sm">
              <TextInput
                label="Adresse IP / Hostname"
                placeholder="192.168.1.100"
                value={netHost}
                onChange={e => { setNetHost(e.target.value); setPingResult(null); }}
                style={{ flex: 1 }}
              />
              <NumberInput
                label="Port TCP"
                value={netPort}
                onChange={v => setNetPort(typeof v === "number" ? v : 9100)}
                min={1} max={65535} step={1}
                style={{ width: 90 }}
              />
              <Button
                size="sm"
                variant="light"
                color={pingResult === true ? "green" : pingResult === false ? "red" : "gray"}
                loading={pinging}
                onClick={pingNetwork}
                disabled={!netHost}
                style={{ marginBottom: 1 }}
              >
                {pingResult === true ? "OK ✓" : pingResult === false ? "KO ✕" : "Ping"}
              </Button>
            </Group>
            {pingResult === true && (
              <Alert color="green" radius="md" style={{ fontSize: 12 }}>
                Imprimante joignable sur {netHost}:{netPort} — prête à imprimer.
              </Alert>
            )}
          </Stack>
        )}

        {/* Test button */}
        <Button
          size="sm"
          variant="light"
          color="teal"
          leftSection={<IconTestPipe size={16} />}
          onClick={handleTest}
          loading={testing}
          disabled={transport === "Serial" ? !serialPort : !netHost}
        >
          Imprimer page de test
        </Button>
      </Stack>
    </Paper>
  );
}