import { type Component, For, Show } from "solid-js";
import { state } from "~/stores/app";
import type { ExecutionNode } from "~/api/types";
import styles from "./ExecutionPanel.module.css";

const statusClass = (status: string): string => {
  if (status === "running" || status === "cancelling") return styles.running;
  if (status === "done") return styles.done;
  if (status === "waiting") return styles.waiting;
  return styles.error;
};

const NodeItem: Component<{ node: ExecutionNode; depth?: number }> = (props) => {
  return (
    <>
      <div class={styles.node}>
        <span class={`${styles.statusDot} ${statusClass(props.node.status)}`} />
        <span class={styles.nodeKind}>{props.node.kind}</span>
        <span class={styles.nodeLabel}>{props.node.label || props.node.id}</span>
      </div>
      <Show when={props.node.children?.length}>
        <div class={styles.children}>
          <For each={props.node.children}>
            {(child) => <NodeItem node={child} depth={(props.depth ?? 0) + 1} />}
          </For>
        </div>
      </Show>
    </>
  );
};

export const ExecutionPanel: Component = () => {
  const topology = () => state.executionTopology;

  return (
    <div class={styles.panel}>
      <div class={styles.sectionTitle}>Execution Topology</div>
      <Show when={topology()}>
        <div class={styles.summary}>
          <span class={styles.summaryItem}>
            Active: {topology()!.active_count}
          </span>
          <span class={styles.summaryItem}>
            Running: {topology()!.running_count}
          </span>
          <span class={styles.summaryItem}>
            Waiting: {topology()!.waiting_count}
          </span>
        </div>
        <div class={styles.nodeList}>
          <For each={topology()!.nodes}>
            {(node) => <NodeItem node={node} />}
          </For>
        </div>
      </Show>
    </div>
  );
};
