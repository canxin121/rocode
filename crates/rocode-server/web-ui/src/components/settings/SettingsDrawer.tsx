import { type Component, For, Show, Switch, Match } from "solid-js";
import { settings, setSettingsTab } from "~/stores/settings";
import { GeneralSection } from "./GeneralSection";
import { ProviderSection } from "./ProviderSection";
import styles from "./SettingsDrawer.module.css";

const TABS = [
  { id: "general", label: "General" },
  { id: "providers", label: "Providers" },
  { id: "scheduler", label: "Scheduler" },
  { id: "mcp", label: "MCP" },
  { id: "plugins", label: "Plugins" },
  { id: "lsp", label: "LSP" },
];

export interface SettingsDrawerProps {
  onClose: () => void;
}

export const SettingsDrawer: Component<SettingsDrawerProps> = (props) => {
  return (
    <div class={styles.overlay} onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class={styles.drawer}>
        <div class={styles.header}>
          <span class={styles.title}>Settings</span>
          <button class={styles.closeBtn} onClick={props.onClose}>✕</button>
        </div>
        <div class={styles.tabs}>
          <For each={TABS}>
            {(tab) => (
              <button
                class={styles.tab}
                classList={{ [styles.active]: settings.activeTab === tab.id }}
                onClick={() => setSettingsTab(tab.id)}
              >
                {tab.label}
              </button>
            )}
          </For>
        </div>
        <div class={styles.content}>
          <Switch>
            <Match when={settings.activeTab === "general"}>
              <GeneralSection />
            </Match>
            <Match when={settings.activeTab === "providers"}>
              <ProviderSection />
            </Match>
            <Match when={settings.activeTab === "scheduler"}>
              <div class={styles.empty}>Scheduler settings — configure via scheduler config file</div>
            </Match>
            <Match when={settings.activeTab === "mcp"}>
              <div class={styles.empty}>MCP server configuration</div>
            </Match>
            <Match when={settings.activeTab === "plugins"}>
              <div class={styles.empty}>Plugin management</div>
            </Match>
            <Match when={settings.activeTab === "lsp"}>
              <div class={styles.empty}>LSP server status</div>
            </Match>
          </Switch>
        </div>
      </div>
    </div>
  );
};
