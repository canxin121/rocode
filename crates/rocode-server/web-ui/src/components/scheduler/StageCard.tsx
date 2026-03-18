import { type Component, Show, For } from "solid-js";
import styles from "./StageCard.module.css";

export interface StageCardProps {
  name: string;
  status: string;
  profile?: string;
  index?: number;
  total?: number;
  step?: string;
  focus?: string;
  activity?: string;
}

const statusLabel = (status: string) => {
  switch (status) {
    case "waiting": return "? waiting";
    case "running": return "@ running";
    case "done": return "+ done";
    case "blocked": return "! blocked";
    case "cancelled": return "x cancelled";
    default: return status;
  }
};

const statusTone = (status: string) => {
  if (status === "done") return "success";
  if (status === "blocked" || status === "cancelled") return "error";
  return "warning";
};

export const StageCard: Component<StageCardProps> = (props) => {
  return (
    <article class={styles.card}>
      <div class={styles.header}>
        <Show when={props.profile}>
          <span class={styles.profile}>{props.profile}</span>
          <span class={styles.dot}>·</span>
        </Show>
        <span class={styles.name}>{props.name}</span>
      </div>
      <div class={styles.chips}>
        <span class={`${styles.chip} ${styles[statusTone(props.status)]}`}>
          {statusLabel(props.status)}
        </span>
        <Show when={props.index != null && props.total != null}>
          <span class={styles.chip}>{props.index}/{props.total}</span>
        </Show>
        <Show when={props.step}>
          <span class={styles.chip}>{props.step}</span>
        </Show>
      </div>
      <Show when={props.focus}>
        <div class={styles.field}>
          <span class={styles.fieldLabel}>Focus</span>
          <span>{props.focus}</span>
        </div>
      </Show>
      <Show when={props.activity}>
        <pre class={styles.activity}>{props.activity}</pre>
      </Show>
    </article>
  );
};
