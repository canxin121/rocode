import { type Component, For, Show, createMemo } from "solid-js";
import { state, buildProjects, type Project } from "~/stores/app";
import { SessionItem } from "./SessionItem";
import styles from "./SessionList.module.css";

export interface SessionListProps {
  searchQuery: string;
  onSelectSession: (sessionId: string) => void;
}

export const SessionList: Component<SessionListProps> = (props) => {
  const projects = createMemo(() => {
    buildProjects(props.searchQuery);
    return state.projects;
  });

  return (
    <div class={styles.list}>
      <Show
        when={projects().length > 0}
        fallback={<div class={styles.empty}>No sessions yet</div>}
      >
        <For each={projects()}>
          {(project) => (
            <div class={styles.projectGroup}>
              <button
                class={styles.projectTitle}
                classList={{ [styles.active]: state.selectedProject === project.key }}
                onClick={() => {
                  // Select project and its first session
                  const firstSession = project.sessions[0];
                  if (firstSession) {
                    props.onSelectSession(firstSession.id);
                  }
                }}
              >
                <span class={styles.projectLabel}>{project.label}</span>
                <span class={styles.projectCount}>{project.sessions.length}</span>
              </button>

              <Show when={state.selectedProject === project.key}>
                <div class={styles.sessionItems}>
                  <For each={project.sessions}>
                    {(session) => (
                      <SessionItem
                        session={session}
                        active={state.selectedSession === session.id}
                        onClick={() => props.onSelectSession(session.id)}
                      />
                    )}
                  </For>
                </div>
              </Show>
            </div>
          )}
        </For>
      </Show>
    </div>
  );
};
