import { type Component, For, Show, createSignal, createResource } from "solid-js";
import { api } from "~/api/client";
import { state } from "~/stores/app";
import type { StageEvent } from "~/api/types";
import styles from "./StageInspector.module.css";

export const StageInspector: Component = () => {
  const [selectedStage, setSelectedStage] = createSignal<string | null>(null);
  const [stages, setStages] = createSignal<string[]>([]);
  const [events, setEvents] = createSignal<StageEvent[]>([]);

  const loadStages = async () => {
    if (!state.selectedSession) return;
    try {
      const response = await api(`/session/${state.selectedSession}/events/stages`);
      const data: string[] = await response.json();
      setStages(data);
      if (data.length > 0 && !selectedStage()) {
        setSelectedStage(data[0]);
        await loadEvents(data[0]);
      }
    } catch {
      // Silently ignore
    }
  };

  const loadEvents = async (stageId: string) => {
    if (!state.selectedSession) return;
    try {
      const response = await api(
        `/session/${state.selectedSession}/events?stage_id=${encodeURIComponent(stageId)}&limit=100`,
      );
      const data: StageEvent[] = await response.json();
      setEvents(data);
    } catch {
      setEvents([]);
    }
  };

  const handleSelectStage = async (stageId: string) => {
    setSelectedStage(stageId);
    await loadEvents(stageId);
  };

  return (
    <div class={styles.inspector}>
      <div class={styles.header}>
        <span class={styles.title}>Stage Inspector</span>
        <button class={styles.refreshBtn} onClick={loadStages}>↻</button>
      </div>
      <Show when={stages().length > 0}>
        <div class={styles.tabs}>
          <For each={stages()}>
            {(stageId) => (
              <button
                class={styles.tab}
                classList={{ [styles.active]: selectedStage() === stageId }}
                onClick={() => handleSelectStage(stageId)}
              >
                {stageId}
              </button>
            )}
          </For>
        </div>
      </Show>
      <div class={styles.eventTable}>
        <Show
          when={events().length > 0}
          fallback={<div class={styles.empty}>No events</div>}
        >
          <table class={styles.table}>
            <thead>
              <tr>
                <th>Time</th>
                <th>Type</th>
                <th>Execution</th>
              </tr>
            </thead>
            <tbody>
              <For each={events()}>
                {(event) => (
                  <tr>
                    <td>{event.time}</td>
                    <td>{event.event_type}</td>
                    <td>{event.execution_id ?? "--"}</td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </Show>
      </div>
    </div>
  );
};
