// ── SSE Streaming & Prompt ──────────────────────────────────────────────────

async function parseSSE(response, onEvent) {
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let eventName = null;
  let dataLines = [];

  const flush = () => {
    if (dataLines.length === 0) {
      eventName = null;
      return;
    }
    const data = dataLines.join("\n");
    dataLines = [];

    let parsed;
    try {
      parsed = JSON.parse(data);
    } catch (_) {
      parsed = { raw: data };
    }
    onEvent(eventName || SERVER_EVENT_TYPES.MESSAGE, parsed);
    eventName = null;
  };

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() || "";

    for (const lineRaw of lines) {
      const line = lineRaw.endsWith("\r") ? lineRaw.slice(0, -1) : lineRaw;
      if (!line) {
        flush();
        continue;
      }
      if (line.startsWith("event:")) {
        eventName = line.slice(6).trim();
      } else if (line.startsWith("data:")) {
        dataLines.push(line.slice(5).trimStart());
      }
    }
  }

  flush();
}

async function sendPrompt(content) {
  if (!content || interactionLocked()) return;

  state.streaming = true;
  state.abortRequested = false;
  syncInteractionState();
  setBadge("running", BADGE_TONES.WARN);
  applyOutputBlock({
    kind: OUTPUT_BLOCK_KINDS.STATUS,
    tone: OUTPUT_BLOCK_TONES.WARNING,
    text: "Running...",
    silent: true,
  });

  try {
    if (!state.selectedSession) {
      await createAndSelectSession();
    }

    applyOutputBlock({
      kind: OUTPUT_BLOCK_KINDS.MESSAGE,
      phase: MESSAGE_PHASES.FULL,
      role: MESSAGE_ROLES.USER,
      title: "you",
      text: content,
    });

    const mode = selectedMode();
    const payload = {
      content,
      stream: true,
      model: state.selectedModel,
    };
    if (mode) {
      if (mode.kind === "agent") {
        payload.agent = mode.id;
      } else if (mode.kind === "preset" || mode.kind === "profile") {
        payload.scheduler_profile = mode.id;
      }
    }

    const response = await api(`/session/${state.selectedSession}/stream`, {
      method: "POST",
      body: JSON.stringify(payload),
    });

    try {
      const snapshot = await refreshSessionSnapshot(state.selectedSession);
      setBadge(runtimeBadgeText(snapshot), BADGE_TONES.WARN);
    } catch (_) {
      setBadge("Running...", BADGE_TONES.WARN);
    }

    await parseSSE(response, (name, payload) => {
      if (name === SERVER_EVENT_TYPES.OUTPUT_BLOCK) {
        const handled = applyOutputBlockEvent(payload);
        if (!handled) {
          applyFocusedChildOutputBlockEvent(payload);
        }
        const block = payload && payload[WIRE_KEYS.BLOCK] ? payload[WIRE_KEYS.BLOCK] : payload;
        if (
          handled &&
          block &&
          (block.kind === OUTPUT_BLOCK_KINDS.SCHEDULER_STAGE || block.kind === OUTPUT_BLOCK_KINDS.TOOL)
        ) {
          scheduleExecutionTopologyRefresh();
        }
        return;
      }

      if (name === SERVER_EVENT_TYPES.QUESTION_CREATED) {
        const sessionId = payload[WIRE_KEYS.SESSION_ID] || payload[WIRE_KEYS.SESSION_ID_ALIAS];
        if (!sessionId || sessionId === state.selectedSession) {
          openQuestionPanel(interactionFromLiveQuestionEvent(payload));
          scheduleExecutionTopologyRefresh();
        }
        return;
      }

      if (name === SERVER_EVENT_TYPES.PERMISSION_REQUESTED) {
        const sessionId = payload[WIRE_KEYS.SESSION_ID] || payload[WIRE_KEYS.SESSION_ID_ALIAS];
        if (!sessionId || sessionId === state.selectedSession) {
          openPermissionPanel(permissionInteractionFromLiveEvent(payload));
          scheduleExecutionTopologyRefresh();
        }
        return;
      }

      if (QUESTION_RESOLUTION_EVENT_TYPES.includes(name)) {
        const requestId = payload[WIRE_KEYS.REQUEST_ID] || payload[WIRE_KEYS.REQUEST_ID_ALIAS];
        if (
          state.activeQuestionInteraction &&
          state.activeQuestionInteraction.request_id &&
          state.activeQuestionInteraction.request_id === requestId
        ) {
          closeQuestionPanel();
          void loadMessages();
        }
        scheduleExecutionTopologyRefresh();
        return;
      }

      if (PERMISSION_RESOLUTION_EVENT_TYPES.includes(name)) {
        const permissionId =
          payload[WIRE_KEYS.PERMISSION_ID] ||
          payload[WIRE_KEYS.PERMISSION_ID_ALIAS] ||
          payload[WIRE_KEYS.REQUEST_ID] ||
          payload[WIRE_KEYS.REQUEST_ID_ALIAS];
        if (
          state.activePermissionInteraction &&
          state.activePermissionInteraction.permission_id &&
          state.activePermissionInteraction.permission_id === permissionId
        ) {
          closePermissionPanel();
        }
        scheduleExecutionTopologyRefresh();
        return;
      }

      if (name === SERVER_EVENT_TYPES.USAGE) {
        applyStreamUsage(payload);
      } else if (name === SERVER_EVENT_TYPES.ERROR) {
        applyOutputBlock({
          kind: OUTPUT_BLOCK_KINDS.STATUS,
          tone: OUTPUT_BLOCK_TONES.ERROR,
          text: payload[WIRE_KEYS.ERROR] || "Stream error",
        });
      }
    });

    applyOutputBlock({
      kind: OUTPUT_BLOCK_KINDS.STATUS,
      tone: OUTPUT_BLOCK_TONES.SUCCESS,
      text: "Done.",
      silent: true,
    });
    await loadSessions();
  } catch (error) {
    applyOutputBlock({
      kind: OUTPUT_BLOCK_KINDS.STATUS,
      tone: OUTPUT_BLOCK_TONES.ERROR,
      text: `Send failed: ${String(error)}`,
    });
  } finally {
    state.streaming = false;
    state.abortRequested = false;
    await refreshExecutionTopology().catch(() => {});
    syncInteractionState();
    if (!state.busyAction) {
      setBadge("ready", BADGE_TONES.OK);
    }
  }
}
