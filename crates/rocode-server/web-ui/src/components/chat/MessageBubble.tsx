import { type Component, Show, createMemo } from "solid-js";
import { renderMarkdownToHtml } from "~/utils/markdown";
import type { FeedMessage } from "./MessageFeed";
import styles from "./MessageBubble.module.css";

export interface MessageBubbleProps {
  message: FeedMessage;
}

export const MessageBubble: Component<MessageBubbleProps> = (props) => {
  const isStatus = () => props.message.kind === "status";
  const isUser = () => props.message.role === "user";
  const isAssistant = () =>
    props.message.role === "assistant" || props.message.kind === "message";

  const bubbleClass = createMemo(() => {
    if (isStatus()) {
      const tone = props.message.tone ?? "normal";
      return `${styles.bubble} ${styles.status} ${styles[tone] ?? ""}`;
    }
    if (isUser()) return `${styles.bubble} ${styles.user}`;
    if (isAssistant()) return `${styles.bubble} ${styles.assistant}`;
    return `${styles.bubble} ${styles.system}`;
  });

  const renderedHtml = createMemo(() => {
    if (isStatus()) return props.message.text;
    return renderMarkdownToHtml(props.message.text);
  });

  return (
    <article class={bubbleClass()}>
      <Show when={props.message.title && !isStatus()}>
        <div class={styles.title}>{props.message.title}</div>
      </Show>
      <div class={styles.content} innerHTML={renderedHtml()} />
    </article>
  );
};
