// src/components/ui/FeatureGate.tsx
// Renders children only if the active license includes the required feature flag.
// Shows a professional "upgrade required" card otherwise.

import { Center, Stack, ThemeIcon, Text, Paper, Button } from "@mantine/core";
import { IconLock } from "@tabler/icons-react";
import { useNavigate } from "react-router-dom";
import { useLicenseStore } from "@/store/licenseStore";

interface FeatureGateProps {
  flag:      number;
  children:  React.ReactNode;
  fallback?: React.ReactNode;
}

export function FeatureGate({ flag, children, fallback }: FeatureGateProps) {
  const can = useLicenseStore((s) => s.can);

  if (can(flag)) return <>{children}</>;
  if (fallback)  return <>{fallback}</>;
  return <LockedFeaturePlaceholder />;
}

function LockedFeaturePlaceholder() {
  const navigate = useNavigate();

  return (
    <Center style={{ height: "100%", minHeight: 360 }}>
      <Paper
        withBorder
        p="xl"
        radius="lg"
        style={{ maxWidth: 420, textAlign: "center" }}
      >
        <Stack align="center" gap="md">
          <ThemeIcon size={64} radius="xl" color="violet" variant="light">
            <IconLock size={32} />
          </ThemeIcon>

          <div>
            <Text fw={700} size="xl" mb={4}>
              Fonctionnalité Premium
            </Text>
            <Text size="sm" c="dimmed" maw={300} mx="auto">
              Cette fonctionnalité n'est pas incluse dans votre licence actuelle.
              Contactez votre revendeur pour mettre à niveau.
            </Text>
          </div>

          <Button
            variant="light"
            color="violet"
            onClick={() => navigate("/license")}
          >
            Voir ma licence
          </Button>
        </Stack>
      </Paper>
    </Center>
  );
}