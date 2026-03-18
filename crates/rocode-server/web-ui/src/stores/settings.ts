// ── Settings Store ─────────────────────────────────────────────────────────

import { createStore, produce } from "solid-js/store";
import { api } from "~/api/client";

export interface SettingsState {
  activeTab: string;
  configSnapshot: Record<string, unknown> | null;
  knownProviders: unknown[];
  providerSelection: string | null;
  modelSelection: string | null;
  schedulerConfigSnapshot: Record<string, unknown> | null;
  mcpSelection: string | null;
  pluginSelection: string | null;
  mcpStatusSnapshot: Record<string, unknown>;
  pluginAuthSnapshot: unknown[];
  lspStatusSnapshot: { servers: string[] };
  formatterStatusSnapshot: { formatters: string[] };
  inlineActions: {
    provider: string | null;
    model: string | null;
    mcp: string | null;
    plugin: string | null;
  };
}

const [settings, setSettings] = createStore<SettingsState>({
  activeTab: "general",
  configSnapshot: null,
  knownProviders: [],
  providerSelection: null,
  modelSelection: null,
  schedulerConfigSnapshot: null,
  mcpSelection: null,
  pluginSelection: null,
  mcpStatusSnapshot: {},
  pluginAuthSnapshot: [],
  lspStatusSnapshot: { servers: [] },
  formatterStatusSnapshot: { formatters: [] },
  inlineActions: {
    provider: null,
    model: null,
    mcp: null,
    plugin: null,
  },
});

export { settings, setSettings };

export function setSettingsTab(tab: string) {
  setSettings("activeTab", tab);
}

export async function loadSettingsConfig(): Promise<Record<string, unknown>> {
  const response = await api("/config");
  const config = await response.json();
  setSettings("configSnapshot", config);
  return config;
}

export async function saveConfig(patch: Record<string, unknown>): Promise<void> {
  await api("/config", {
    method: "PATCH",
    body: JSON.stringify(patch),
  });
}
