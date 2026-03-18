import { type Component, For } from "solid-js";
import {
  state,
  THEMES,
  setTheme,
  setSelectedMode,
  setSelectedModel,
  selectedModeLabel,
} from "~/stores/app";
import styles from "./SettingsDrawer.module.css";

export const GeneralSection: Component = () => {
  return (
    <div>
      <div class={styles.section}>
        <div class={styles.sectionTitle}>Appearance</div>
        <div class={styles.field}>
          <label class={styles.fieldLabel}>Theme</label>
          <select
            class={styles.fieldSelect}
            value={state.selectedTheme}
            onChange={(e) => setTheme(e.currentTarget.value)}
          >
            <For each={THEMES}>
              {(theme) => (
                <option value={theme.id}>{theme.label}</option>
              )}
            </For>
          </select>
        </div>
      </div>

      <div class={styles.section}>
        <div class={styles.sectionTitle}>Execution</div>
        <div class={styles.field}>
          <label class={styles.fieldLabel}>Mode</label>
          <select
            class={styles.fieldSelect}
            value={state.selectedModeKey ?? ""}
            onChange={(e) => {
              const value = e.currentTarget.value;
              setSelectedMode(value || null);
            }}
          >
            <option value="">auto</option>
            <For each={state.modes}>
              {(mode) => (
                <option value={`${mode.kind}:${mode.id}`}>
                  {mode.kind === "agent" ? mode.name : `${mode.kind}:${mode.name}`}
                </option>
              )}
            </For>
          </select>
        </div>
        <div class={styles.field}>
          <label class={styles.fieldLabel}>Model</label>
          <select
            class={styles.fieldSelect}
            value={state.selectedModel ?? ""}
            onChange={(e) => {
              setSelectedModel(e.currentTarget.value || null);
            }}
          >
            <option value="">auto</option>
            <For each={state.providers}>
              {(provider) => (
                <For each={(provider.models ?? [])}>
                  {(model) => (
                    <option value={`${provider.id}/${model.id}`}>
                      {provider.name}/{model.name}
                    </option>
                  )}
                </For>
              )}
            </For>
          </select>
        </div>
      </div>
    </div>
  );
};
