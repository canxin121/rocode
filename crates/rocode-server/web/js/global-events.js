// ── Global ServerEvent Subscription ────────────────────────────────────────

let globalServerEventStarted = false;
let globalServerEventGeneration = 0;
let globalServerEventReconnectTimer = null;
let globalSessionRefreshTimer = null;
let globalMessageRefreshTimer = null;
let globalSessionSnapshotTimer = null;
let globalConfigRefreshTimer = null;

function globalServerEventType(name, payload) {
  if (name && name !== SERVER_EVENT_TYPES.MESSAGE) return name;
  return payload && payload[WIRE_KEYS.TYPE] ? payload[WIRE_KEYS.TYPE] : SERVER_EVENT_TYPES.MESSAGE;
}

function globalServerEventSessionId(payload) {
  return payload && (
    payload[WIRE_KEYS.SESSION_ID] ||
    payload[WIRE_KEYS.SESSION_ID_ALIAS] ||
    payload[WIRE_KEYS.PARENT_ID] ||
    payload[WIRE_KEYS.PARENT_ID_ALIAS] ||
    payload[WIRE_KEYS.CHILD_ID] ||
    payload[WIRE_KEYS.CHILD_ID_ALIAS]
  );
}

function scheduleGlobalSessionIndexRefresh(delay = 180) {
  if (globalSessionRefreshTimer) {
    clearTimeout(globalSessionRefreshTimer);
  }
  globalSessionRefreshTimer = setTimeout(() => {
    globalSessionRefreshTimer = null;
    const previousSelectedSession = state.selectedSession;
    void refreshSessionsIndex()
      .then(() => {
        if (!state.streaming && previousSelectedSession !== state.selectedSession) {
          return loadMessages();
        }
        return null;
      })
      .catch(() => {});
  }, delay);
}

function scheduleGlobalSelectedMessagesRefresh(delay = 180) {
  if (!state.selectedSession || state.streaming) return;
  if (globalMessageRefreshTimer) {
    clearTimeout(globalMessageRefreshTimer);
  }
  globalMessageRefreshTimer = setTimeout(() => {
    globalMessageRefreshTimer = null;
    if (!state.streaming) {
      void loadMessages().catch(() => {});
    }
  }, delay);
}

function scheduleGlobalSelectedSessionSnapshotRefresh(delay = 120) {
  if (!state.selectedSession) return;
  if (globalSessionSnapshotTimer) {
    clearTimeout(globalSessionSnapshotTimer);
  }
  globalSessionSnapshotTimer = setTimeout(() => {
    globalSessionSnapshotTimer = null;
    void refreshSessionSnapshot().catch(() => {});
  }, delay);
}

function scheduleGlobalConfigRefresh(delay = 250) {
  if (globalConfigRefreshTimer) {
    clearTimeout(globalConfigRefreshTimer);
  }
  globalConfigRefreshTimer = setTimeout(() => {
    globalConfigRefreshTimer = null;
    void Promise.all([loadProviders(), loadModes(), loadUiCommands()])
      .then(() => loadWebUiPreferences())
      .then(() => {
        if (nodes.commandPanel && !nodes.commandPanel.classList.contains("hidden")) {
          return loadSettingsWorkspace({ force: true });
        }
        return null;
      })
      .catch(() => {});
  }, delay);
}

function maybeOpenGlobalQuestion(payload) {
  const sessionId = payload[WIRE_KEYS.SESSION_ID] || payload[WIRE_KEYS.SESSION_ID_ALIAS];
  if (!sessionId || sessionId !== state.selectedSession) return;

  const interaction = interactionFromLiveQuestionEvent(payload);
  if (!interaction || !interaction.request_id) return;
  if (
    state.activeQuestionInteraction &&
    state.activeQuestionInteraction.request_id === interaction.request_id
  ) {
    return;
  }

  openQuestionPanel(interaction);
}

function maybeResolveGlobalQuestion(payload) {
  const requestId = payload[WIRE_KEYS.REQUEST_ID] || payload[WIRE_KEYS.REQUEST_ID_ALIAS];
  if (
    requestId &&
    state.activeQuestionInteraction &&
    state.activeQuestionInteraction.request_id === requestId
  ) {
    closeQuestionPanel();
  }
}

function maybeOpenGlobalPermission(payload) {
  const interaction = permissionInteractionFromLiveEvent(payload);
  if (!interaction || !interaction.permission_id) return;
  if (!interaction.session_id || interaction.session_id !== state.selectedSession) return;
  if (
    state.activePermissionInteraction &&
    state.activePermissionInteraction.permission_id === interaction.permission_id
  ) {
    return;
  }

  openPermissionPanel(interaction);
}

function maybeResolveGlobalPermission(payload) {
  const permissionId =
    payload[WIRE_KEYS.PERMISSION_ID] ||
    payload[WIRE_KEYS.PERMISSION_ID_ALIAS] ||
    payload[WIRE_KEYS.REQUEST_ID] ||
    payload[WIRE_KEYS.REQUEST_ID_ALIAS];
  if (
    permissionId &&
    state.activePermissionInteraction &&
    state.activePermissionInteraction.permission_id === permissionId
  ) {
    closePermissionPanel();
  }
}

function handleGlobalServerEvent(name, payload) {
  const type = globalServerEventType(name, payload);

  if (type === SERVER_EVENT_TYPES.MESSAGE) {
    return;
  }

  if (type === SERVER_EVENT_TYPES.OUTPUT_BLOCK) {
    if (state.streaming) return;
    const handled = applyOutputBlockEvent(payload);
    if (!handled && !applyFocusedChildOutputBlockEvent(payload)) return;
    const block = payload && payload[WIRE_KEYS.BLOCK] ? payload[WIRE_KEYS.BLOCK] : payload;
    if (
      block &&
      (block.kind === OUTPUT_BLOCK_KINDS.SCHEDULER_STAGE || block.kind === OUTPUT_BLOCK_KINDS.TOOL)
    ) {
      scheduleExecutionTopologyRefresh(60);
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.USAGE) {
    if (!state.streaming && globalServerEventSessionId(payload) === state.selectedSession) {
      applyStreamUsage(payload);
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.ERROR) {
    if (!state.streaming && globalServerEventSessionId(payload) === state.selectedSession) {
      applyOutputBlock({
        kind: OUTPUT_BLOCK_KINDS.STATUS,
        tone: OUTPUT_BLOCK_TONES.ERROR,
        text: payload[WIRE_KEYS.ERROR] || payload[WIRE_KEYS.MESSAGE] || "Stream error",
      });
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.SESSION_UPDATED) {
    scheduleGlobalSessionIndexRefresh();
    return;
  }

  if (type === SERVER_EVENT_TYPES.SESSION_STATUS) {
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleGlobalSelectedSessionSnapshotRefresh(80);
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.EXECUTION_TOPOLOGY_CHANGED) {
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleExecutionTopologyRefresh(60);
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.QUESTION_CREATED) {
    maybeOpenGlobalQuestion(payload);
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleExecutionTopologyRefresh(60);
    }
    return;
  }

  if (QUESTION_RESOLUTION_EVENT_TYPES.includes(type)) {
    maybeResolveGlobalQuestion(payload);
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleExecutionTopologyRefresh(60);
      scheduleGlobalSelectedMessagesRefresh(120);
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.PERMISSION_REQUESTED) {
    maybeOpenGlobalPermission(payload);
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleExecutionTopologyRefresh(60);
    }
    return;
  }

  if (PERMISSION_RESOLUTION_EVENT_TYPES.includes(type)) {
    maybeResolveGlobalPermission(payload);
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleExecutionTopologyRefresh(60);
      scheduleGlobalSelectedMessagesRefresh(120);
    }
    return;
  }

  if (type === SERVER_EVENT_TYPES.CONFIG_UPDATED) {
    scheduleGlobalConfigRefresh();
    return;
  }

  if (
    type === SERVER_EVENT_TYPES.CHILD_SESSION_ATTACHED ||
    type === SERVER_EVENT_TYPES.CHILD_SESSION_DETACHED
  ) {
    const parentId = payload[WIRE_KEYS.PARENT_ID] || payload[WIRE_KEYS.PARENT_ID_ALIAS];
    const childId = payload[WIRE_KEYS.CHILD_ID] || payload[WIRE_KEYS.CHILD_ID_ALIAS];
    scheduleGlobalSessionIndexRefresh();
    if (state.selectedSession && (state.selectedSession === parentId || state.selectedSession === childId)) {
      scheduleExecutionTopologyRefresh(60);
      scheduleGlobalSelectedMessagesRefresh(120);
    }
    return;
  }

  if (
    type === SERVER_EVENT_TYPES.TOOL_CALL_LIFECYCLE ||
    type === SERVER_EVENT_TYPES.DIFF_UPDATED
  ) {
    if (globalServerEventSessionId(payload) === state.selectedSession) {
      scheduleExecutionTopologyRefresh(60);
    }
  }
}

function startGlobalServerEventStream() {
  if (globalServerEventStarted) return;

  globalServerEventStarted = true;
  globalServerEventGeneration += 1;
  const generation = globalServerEventGeneration;

  void (async () => {
    while (globalServerEventStarted && generation === globalServerEventGeneration) {
      try {
        const response = await fetch("/event", {
          headers: {
            Accept: "text/event-stream",
          },
        });

        if (!response.ok) {
          throw new Error(`${response.status} ${response.statusText}`);
        }

        await parseSSE(response, (eventName, eventPayload) => {
          handleGlobalServerEvent(eventName, eventPayload);
        });
      } catch (error) {
        console.warn("Global ServerEvent stream disconnected", error);
      }

      if (!globalServerEventStarted || generation !== globalServerEventGeneration) {
        break;
      }

      await new Promise((resolve) => {
        globalServerEventReconnectTimer = setTimeout(() => {
          globalServerEventReconnectTimer = null;
          resolve();
        }, 1500);
      });
    }
  })();
}
