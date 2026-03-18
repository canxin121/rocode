import { type Component, For, Show, createSignal } from "solid-js";
import { state } from "~/stores/app";
import { api } from "~/api/client";
import type { QuestionInteraction } from "~/api/types";
import styles from "./QuestionPanel.module.css";

export interface QuestionPanelProps {
  interaction: QuestionInteraction;
  onClose: () => void;
}

export const QuestionPanel: Component<QuestionPanelProps> = (props) => {
  const [answers, setAnswers] = createSignal<Record<string, string>>({});
  const [submitting, setSubmitting] = createSignal(false);

  const selectOption = (questionIdx: number, value: string) => {
    setAnswers((prev) => ({ ...prev, [String(questionIdx)]: value }));
  };

  const handleSubmit = async () => {
    setSubmitting(true);
    try {
      await api(`/question/${props.interaction.request_id}/reply`, {
        method: "POST",
        body: JSON.stringify({ answers: answers() }),
      });
      props.onClose();
    } catch (error) {
      console.error("Failed to submit answers:", error);
    } finally {
      setSubmitting(false);
    }
  };

  const handleReject = async () => {
    setSubmitting(true);
    try {
      await api(`/question/${props.interaction.request_id}/reject`, {
        method: "POST",
      });
      props.onClose();
    } catch (error) {
      console.error("Failed to reject question:", error);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div class={styles.overlay} onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class={styles.panel}>
        <div class={styles.header}>
          <span class={styles.title}>Question</span>
          <button class={styles.closeBtn} onClick={props.onClose}>✕</button>
        </div>
        <div class={styles.body}>
          <For each={props.interaction.questions}>
            {(question, idx) => (
              <div class={styles.questionGroup}>
                <div class={styles.questionText}>{question.question}</div>
                <Show when={question.options?.length}>
                  <div class={styles.optionList}>
                    <For each={question.options}>
                      {(option) => (
                        <button
                          class={styles.option}
                          classList={{
                            [styles.selected]: answers()[String(idx())] === option.value,
                          }}
                          onClick={() => selectOption(idx(), option.value)}
                        >
                          {option.label}
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
                <Show when={!question.options?.length}>
                  <input
                    class={styles.customInput}
                    type="text"
                    placeholder="Type your answer..."
                    onInput={(e) => selectOption(idx(), e.currentTarget.value)}
                  />
                </Show>
              </div>
            )}
          </For>
        </div>
        <div class={styles.footer}>
          <button
            class={styles.btnDanger}
            onClick={handleReject}
            disabled={submitting()}
          >
            Reject
          </button>
          <button
            class={styles.btnPrimary}
            onClick={handleSubmit}
            disabled={submitting()}
          >
            Submit
          </button>
        </div>
      </div>
    </div>
  );
};
