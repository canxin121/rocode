import { type Component } from "solid-js";
import { MessageFeed } from "./MessageFeed";
import { Composer } from "./Composer";
import styles from "./ChatDomain.module.css";

export interface ChatDomainProps {
  onSend: (content: string) => void;
}

export const ChatDomain: Component<ChatDomainProps> = (props) => {
  return (
    <div class={styles.domain}>
      <MessageFeed />
      <div class={styles.composerWrap}>
        <Composer onSend={props.onSend} />
      </div>
    </div>
  );
};
