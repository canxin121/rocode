import { type Component, Show } from "solid-js";
import { state } from "~/stores/app";
import { ExecutionPanel } from "./ExecutionPanel";
import { ChildSessionRail } from "./ChildSessionRail";
import styles from "./SchedulerDomain.module.css";

export const SchedulerDomain: Component = () => {
  return (
    <div class={styles.domain}>
      <Show
        when={state.executionTopology}
        fallback={
          <div class={styles.empty}>
            No active executions. Start a scheduler run to see execution topology.
          </div>
        }
      >
        <div class={styles.panels}>
          <div class={styles.mainPanel}>
            <ExecutionPanel />
          </div>
          <div class={styles.sidePanel}>
            <ChildSessionRail />
          </div>
        </div>
      </Show>
    </div>
  );
};
