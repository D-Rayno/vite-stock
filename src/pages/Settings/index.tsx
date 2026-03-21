// src/pages/Settings/index.tsx
import { useCallback, useEffect, useState } from "react";
import { useTranslation }   from "react-i18next";
import {
  Stack, Group, Text, Title, Button, Paper, TextInput,
  Select, Switch, NumberInput, Divider, Badge, Table,
  Loader, Center, ActionIcon, Tooltip, Alert, Code, Kbd,
} from "@mantine/core";
import {
  IconDeviceFloppy, IconPrinter, IconTestPipe,
  IconDatabase, IconAlertCircle, IconCheck,
  IconRefresh, IconBuildingStore, IconLanguage,
} from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";
import { invoke }         from "@tauri-apps/api/core";
import { useSettingsStore } from "@/store/settingsStore";
import { applyDirection }   from "@/i18n";
import i18n                 from "@/i18n";
import * as cmd             from "@/lib/commands";
import type { AppSettings } from "@/types";

interface PrinterPort { port: string; description: string; likely_thermal: boolean; }
interface BackupResult { path: string; size_kb: number; created_at: string; }

export default function SettingsPage() {
  const { t }  = useTranslation();
  const { settings, setSettings } = useSettingsStore();

  const [form,    setForm]    = useState<Partial<AppSettings>>({});
  const [saving,  setSaving]  = useState(false);

  // Printer state
  const [printers,      setPrinters]      = useState<PrinterPort[]>([]);
  const [selectedPort,  setSelectedPort]  = useState<string | null>(null);
  const [loadingPorts,  setLoadingPorts]  = useState(false);
  const [testPrinting,  setTestPrinting]  = useState(false);

  // Backup state
  const [backups,       setBackups]       = useState<BackupResult[]>([]);
  const [backing,       setBacking]       = useState(false);

  useEffect(() => { if (settings) setForm({ ...settings }); }, [settings]);

  const set = (key: keyof AppSettings, val: string) =>
    setForm(f => ({ ...f, [key]: val }));

  const S = (key: keyof AppSettings) => form[key] ?? "";

  // ── Save ────────────────────────────────────────────────────────────────
  const handleSave = async () => {
    setSaving(true);
    try {
      await cmd.updateSettings(form as AppSettings);
      setSettings(form as AppSettings);
      if (form.default_language) {
        i18n.changeLanguage(form.default_language);
        applyDirection(form.default_language);
      }
      notifications.show({ color: "green", title: "Sauvegardé", message: t("settings.saved") });
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally { setSaving(false); }
  };

  // ── Printers ────────────────────────────────────────────────────────────
  const loadPrinters = useCallback(async () => {
    setLoadingPorts(true);
    try {
      const list = await invoke<PrinterPort[]>("cmd_list_printers");
      setPrinters(list);
      if (!selectedPort && list.find(p => p.likely_thermal)) {
        setSelectedPort(list.find(p => p.likely_thermal)!.port);
      }
    } catch (e) {
      notifications.show({ color: "orange", message: `Ports: ${e}` });
    } finally { setLoadingPorts(false); }
  }, [selectedPort]);

  const handleTestPrint = async () => {
    if (!selectedPort) {
      notifications.show({ color: "orange", message: "Sélectionnez un port d'abord." });
      return;
    }
    setTestPrinting(true);
    try {
      await invoke("cmd_print_test_page", { port: selectedPort, baud: 9600 });
      notifications.show({ color: "green", message: "Page de test envoyée !" });
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally { setTestPrinting(false); }
  };

  // ── Backup ──────────────────────────────────────────────────────────────
  const loadBackups = useCallback(async () => {
    try {
      const list = await invoke<BackupResult[]>("cmd_list_backups");
      setBackups(list);
    } catch { setBackups([]); }
  }, []);

  const handleBackup = async () => {
    setBacking(true);
    try {
      const r = await invoke<BackupResult>("cmd_create_backup");
      notifications.show({
        color: "green",
        title: "Sauvegarde créée",
        message: `${r.path.split(/[\\/]/).pop()} — ${r.size_kb} Ko`,
      });
      loadBackups();
    } catch (e) {
      notifications.show({ color: "red", message: String(e) });
    } finally { setBacking(false); }
  };

  useEffect(() => { loadBackups(); }, []);

  // ── Render ──────────────────────────────────────────────────────────────
  return (
    <Stack gap="lg" p="lg" style={{ maxWidth: 720, margin: "0 auto" }}>
      <Group justify="space-between">
        <Title order={2}>Paramètres</Title>
        <Button
          leftSection={<IconDeviceFloppy size={16} />}
          onClick={handleSave}
          loading={saving}
        >
          Enregistrer <Kbd ml={6} size="xs">Ctrl+S</Kbd>
        </Button>
      </Group>

      {/* ── Shop info ──────────────────────────────────────────────────── */}
      <Paper p="lg" radius="md" withBorder>
        <Group gap="xs" mb="md">
          <IconBuildingStore size={18} color="var(--mantine-color-blue-6)" />
          <Text fw={700}>Informations du magasin</Text>
        </Group>
        <Stack gap="sm">
          <Group grow>
            <TextInput label="Nom (Français)" value={S("shop_name_fr")}
              onChange={e => set("shop_name_fr", e.target.value)} />
            <TextInput label="الاسم (Arabe)" value={S("shop_name_ar")}
              onChange={e => set("shop_name_ar", e.target.value)}
              styles={{ input: { textAlign: "right", direction: "rtl" } }} />
          </Group>
          <TextInput label="Adresse" value={S("shop_address")}
            onChange={e => set("shop_address", e.target.value)} />
          <Group grow>
            <TextInput label="Téléphone" value={S("shop_phone")}
              onChange={e => set("shop_phone", e.target.value)} placeholder="05x xxx xxxx" />
            <TextInput label="NIF" value={S("shop_nif")} ff="monospace"
              onChange={e => set("shop_nif", e.target.value)} />
          </Group>
          <Group grow>
            <TextInput label="NIS" value={S("shop_nis")} ff="monospace"
              onChange={e => set("shop_nis", e.target.value)} />
            <TextInput label="Registre de commerce" value={S("shop_rc")} ff="monospace"
              onChange={e => set("shop_rc", e.target.value)} />
          </Group>
        </Stack>
      </Paper>

      {/* ── Language + Currency ────────────────────────────────────────── */}
      <Paper p="lg" radius="md" withBorder>
        <Group gap="xs" mb="md">
          <IconLanguage size={18} color="var(--mantine-color-blue-6)" />
          <Text fw={700}>Langue & Affichage</Text>
        </Group>
        <Group grow wrap="wrap">
          <Select
            label="Langue par défaut"
            data={[
              { value: "fr", label: "🇫🇷 Français" },
              { value: "ar", label: "🇩🇿 العربية" },
            ]}
            value={S("default_language") || "fr"}
            onChange={v => set("default_language", v ?? "fr")}
          />
          <Switch
            label="Afficher le détail TVA sur les tickets"
            checked={S("vat_display") === "1"}
            onChange={e => set("vat_display", e.currentTarget.checked ? "1" : "0")}
            mt="lg"
          />
          <NumberInput
            label="Seuil alerte expiration (jours)"
            value={parseInt(S("expiry_warn_days")) || 30}
            onChange={v => set("expiry_warn_days", String(v))}
            min={1} max={365} step={1}
          />
        </Group>
      </Paper>

      {/* ── Printer ────────────────────────────────────────────────────── */}
      <Paper p="lg" radius="md" withBorder>
        <Group gap="xs" mb="md" justify="space-between">
          <Group gap="xs">
            <IconPrinter size={18} color="var(--mantine-color-blue-6)" />
            <Text fw={700}>Imprimante thermique</Text>
          </Group>
          <Tooltip label="Actualiser la liste des ports" withArrow>
            <ActionIcon variant="light" onClick={loadPrinters} loading={loadingPorts}>
              <IconRefresh size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>

        <Stack gap="sm">
          <Select
            label="Largeur du ticket"
            data={[
              { value: "58", label: "58 mm (32 chars/ligne)" },
              { value: "80", label: "80 mm (48 chars/ligne)" },
            ]}
            value={S("thermal_width") || "80"}
            onChange={v => set("thermal_width", v ?? "80")}
          />

          {printers.length === 0 ? (
            <Button
              variant="light"
              leftSection={<IconPrinter size={16} />}
              onClick={loadPrinters}
              loading={loadingPorts}
            >
              Détecter les imprimantes
            </Button>
          ) : (
            <>
              <Select
                label="Port de l'imprimante"
                data={printers.map(p => ({
                  value: p.port,
                  label: `${p.port}${p.description ? ` — ${p.description}` : ""}${p.likely_thermal ? " 🖨" : ""}`,
                }))}
                value={selectedPort}
                onChange={setSelectedPort}
                placeholder="Sélectionner un port…"
              />
              <Group>
                <Button
                  size="sm"
                  variant="light"
                  color="teal"
                  leftSection={<IconTestPipe size={16} />}
                  onClick={handleTestPrint}
                  loading={testPrinting}
                  disabled={!selectedPort}
                >
                  Page de test
                </Button>
                <Text size="xs" c="dimmed">
                  Imprime une page de test pour vérifier la connexion.
                </Text>
              </Group>
            </>
          )}
        </Stack>
      </Paper>

      {/* ── Backups ────────────────────────────────────────────────────── */}
      <Paper p="lg" radius="md" withBorder>
        <Group gap="xs" mb="md" justify="space-between">
          <Group gap="xs">
            <IconDatabase size={18} color="var(--mantine-color-blue-6)" />
            <Text fw={700}>Sauvegardes</Text>
          </Group>
          <Button
            size="sm"
            variant="light"
            color="blue"
            leftSection={<IconDatabase size={16} />}
            onClick={handleBackup}
            loading={backing}
          >
            Créer une sauvegarde
          </Button>
        </Group>

        <Text size="xs" c="dimmed" mb="sm">
          Sauvegardes automatiques stockées dans le répertoire de données de l'application.
          Les 30 dernières sont conservées.
        </Text>

        {backups.length === 0 ? (
          <Text size="sm" c="dimmed">Aucune sauvegarde trouvée.</Text>
        ) : (
          <Table striped withTableBorder>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Fichier</Table.Th>
                <Table.Th style={{ textAlign: "right" }}>Taille</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {backups.slice(0, 5).map((b, i) => (
                <Table.Tr key={i}>
                  <Table.Td>
                    <Text size="xs" ff="monospace">{b.path.split(/[\\/]/).pop()}</Text>
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right" }}>
                    <Text size="xs" c="dimmed">{b.size_kb} Ko</Text>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        )}
      </Paper>
    </Stack>
  );
}