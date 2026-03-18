import { type Component, createSignal, Show } from "solid-js";
import { state } from "~/stores/app";
import { SessionList } from "./SessionList";
import styles from "./NavRail.module.css";

export interface NavRailProps {
  onNewSession: () => void;
  onSelectSession: (sessionId: string) => void;
  onOpenSettings: () => void;
}

export const NavRail: Component<NavRailProps> = (props) => {
  const [collapsed, setCollapsed] = createSignal(false);
  const [searchQuery, setSearchQuery] = createSignal("");

  return (
    <nav
      class={styles.navRail}
      classList={{ [styles.collapsed]: collapsed() }}
    >
      <div class={styles.header}>
        <button
          class={styles.toggleBtn}
          title="Toggle Sidebar"
          onClick={() => setCollapsed((c) => !c)}
        >
          ≡
        </button>
        <span class={styles.brand}>ROCode</span>
      </div>

      <button
        class={styles.newBtn}
        title="New Session"
        onClick={props.onNewSession}
      >
        <span>+</span>
        <span>New Session</span>
      </button>

      <Show when={!collapsed()}>
        <div class={styles.sectionTitle}>Sessions</div>
      </Show>

      <div class={styles.sessionList}>
        <SessionList
          searchQuery={searchQuery()}
          onSelectSession={props.onSelectSession}
        />
      </div>

      <div class={styles.footer}>
        <button
          class={styles.settingsBtn}
          title="Settings"
          onClick={props.onOpenSettings}
        >
          <span>⚙</span>
          <Show when={!collapsed()}>
            <span>Settings</span>
          </Show>
        </button>
      </div>
    </nav>
  );
};
