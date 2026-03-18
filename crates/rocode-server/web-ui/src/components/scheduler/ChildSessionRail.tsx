import { type Component, For, Show } from "solid-js";
import { state } from "~/stores/app";
import styles from "./ChildSessionRail.module.css";

export const ChildSessionRail: Component = () => {
  // Child sessions are derived from execution topology nodes
  const childSessions = () => {
    const topology = state.executionTopology;
    if (!topology) return [];
    const children: { id: string; label: string }[] = [];
    const walk = (nodes: typeof topology.nodes) => {
      for (const node of nodes) {
        if (node.kind === "agent_task" && node.id) {
          children.push({ id: node.id, label: node.label || node.id });
        }
        if (node.children) walk(node.children);
      }
    };
    walk(topology.nodes);
    return children;
  };

  return (
    <div class={styles.rail}>
      <div class={styles.title}>Child Sessions</div>
      <Show
        when={childSessions().length > 0}
        fallback={<div class={styles.empty}>No child sessions</div>}
      >
        <For each={childSessions()}>
          {(child) => (
            <button
              class={styles.card}
              classList={{
                [styles.focused]: state.focusedChildSessionId === child.id,
              }}
            >
              <div class={styles.cardLabel}>{child.label}</div>
              <div class={styles.cardMeta}>{child.id}</div>
            </button>
          )}
        </For>
      </Show>
    </div>
  );
};
