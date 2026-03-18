import { type Component } from "solid-js";
import { short, formatTime } from "~/utils/format";
import type { NormalizedSession } from "~/stores/app";
import styles from "./SessionItem.module.css";

export interface SessionItemProps {
  session: NormalizedSession;
  active: boolean;
  onClick: () => void;
}

export const SessionItem: Component<SessionItemProps> = (props) => {
  return (
    <button
      class={styles.item}
      classList={{ [styles.active]: props.active }}
      onClick={props.onClick}
    >
      <span class={styles.title}>{short(props.session.title, 32)}</span>
      <span class={styles.time}>{formatTime(props.session.updated)}</span>
    </button>
  );
};
