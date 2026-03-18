// ── Output Block Dispatcher ────────────────────────────────────────────────

function targetsSelectedSession(sessionId) {
  return !sessionId || sessionId === state.selectedSession;
}

function applyOutputBlockEvent(payload) {
  if (!payload) return false;
  const sessionId = payload[WIRE_KEYS.SESSION_ID] || payload[WIRE_KEYS.SESSION_ID_ALIAS] || null;
  if (!targetsSelectedSession(sessionId)) {
    return false;
  }
  const block = payload && payload[WIRE_KEYS.BLOCK] ? payload[WIRE_KEYS.BLOCK] : payload;
  applyOutputBlock(block);
  return true;
}

function applyOutputBlock(block) {
  if (!block || !block.kind) return;

  if (block.kind === OUTPUT_BLOCK_KINDS.STATUS) {
    const tone = toneForMessage(block.tone || OUTPUT_BLOCK_TONES.NORMAL);
    setBadge(block.text || OUTPUT_BLOCK_KINDS.STATUS, toneForBadge(tone));
    if (!block.silent) {
      appendMessage(OUTPUT_BLOCK_KINDS.STATUS, block.text || OUTPUT_BLOCK_KINDS.STATUS, Date.now(), {
        title: `${OUTPUT_BLOCK_KINDS.STATUS} · ${tone}`,
        tone,
      });
    }
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.MESSAGE) {
    if (block.phase === MESSAGE_PHASES.START) {
      const result = appendMessage(block.role || MESSAGE_ROLES.ASSISTANT, "", Date.now(), {
        title: block.role || MESSAGE_ROLES.ASSISTANT,
      });
      state.streamMessageArticle = result.article;
      state.streamMessageNode = result.bodyNode;
      state.streamMessageText = "";
      return;
    }
    if (block.phase === MESSAGE_PHASES.DELTA) {
      if (!state.streamMessageNode) {
        const result = appendMessage(block.role || MESSAGE_ROLES.ASSISTANT, "", Date.now(), {
          title: block.role || MESSAGE_ROLES.ASSISTANT,
        });
        state.streamMessageArticle = result.article;
        state.streamMessageNode = result.bodyNode;
        state.streamMessageText = "";
      }
      state.streamMessageText = (state.streamMessageText || "") + (block.text || "");
      state.streamMessageNode.innerHTML = "";
      state.streamMessageNode.classList.add("md-root");
      renderMarkdownToNode(state.streamMessageNode, state.streamMessageText);
      nodes.messageFeed.scrollTop = nodes.messageFeed.scrollHeight;
      return;
    }
    if (block.phase === MESSAGE_PHASES.END) {
      state.streamMessageArticle = null;
      state.streamMessageNode = null;
      state.streamMessageText = "";
      return;
    }
    if (block.phase === MESSAGE_PHASES.FULL) {
      appendMessage(block.role || MESSAGE_ROLES.ASSISTANT, block.text || "", block.ts || Date.now(), {
        title: block.title || (block.role || MESSAGE_ROLES.ASSISTANT),
      });
    }
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.REASONING) {
    if (!state.showThinking) {
      return;
    }

    if (block.phase === MESSAGE_PHASES.START) {
      const result = appendMessage(MESSAGE_ROLES.REASONING, "", Date.now(), {
        title: "thinking",
        tone: OUTPUT_BLOCK_TONES.MUTED,
        beforeNode: state.streamMessageArticle,
      });
      state.streamReasoningArticle = result.article;
      state.streamReasoningNode = result.bodyNode;
      state.streamReasoningText = "";
      return;
    }
    if (block.phase === MESSAGE_PHASES.DELTA) {
      if (!state.streamReasoningNode) {
        const result = appendMessage(MESSAGE_ROLES.REASONING, "", Date.now(), {
          title: "thinking",
          tone: OUTPUT_BLOCK_TONES.MUTED,
          beforeNode: state.streamMessageArticle,
        });
        state.streamReasoningArticle = result.article;
        state.streamReasoningNode = result.bodyNode;
        state.streamReasoningText = "";
      }
      state.streamReasoningText = (state.streamReasoningText || "") + (block.text || "");
      state.streamReasoningNode.textContent = state.streamReasoningText;
      nodes.messageFeed.scrollTop = nodes.messageFeed.scrollHeight;
      return;
    }
    if (block.phase === MESSAGE_PHASES.END) {
      state.streamReasoningArticle = null;
      state.streamReasoningNode = null;
      state.streamReasoningText = "";
      return;
    }
    if (block.phase === MESSAGE_PHASES.FULL) {
      appendMessage(MESSAGE_ROLES.REASONING, block.text || "", block.ts || Date.now(), {
        title: "thinking",
        tone: OUTPUT_BLOCK_TONES.MUTED,
      });
    }
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.TOOL) {
    const phase = block.phase || TOOL_PHASES.START;
    const key = block.id || block.name || `${OUTPUT_BLOCK_KINDS.TOOL}-${Date.now()}`;
    let entry = state.streamToolBlocks.get(key);
    if (!entry) {
      entry = appendToolBlock(block);
      state.streamToolBlocks.set(key, entry);
    }
    updateToolBlock(entry, block);

    if (phase === TOOL_PHASES.DONE || phase === TOOL_PHASES.RESULT || phase === TOOL_PHASES.ERROR) {
      state.streamToolBlocks.delete(key);
    }
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.SESSION_EVENT) {
    const key = block.id || `${block.event || "event"}:${block.title || Date.now()}`;
    let entry = state.streamEventBlocks.get(key);
    if (!entry) {
      entry = appendSessionEventBlock(block);
      state.streamEventBlocks.set(key, entry);
    }
    updateSessionEventBlock(entry, block);
    state.streamEventBlocks.delete(key);
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.QUEUE_ITEM) {
    appendQueueItemBlock(block);
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.INSPECT) {
    renderInspectBlockPayload(block);
    return;
  }

  if (block.kind === OUTPUT_BLOCK_KINDS.SCHEDULER_STAGE) {
    const key =
      block.stage_id ||
      block.id ||
      `${block.profile || "scheduler"}:${block.stage || "stage"}`;
    let entry = state.streamStageBlocks.get(key);
    if (!entry) {
      entry = appendSchedulerStage(block);
      state.streamStageBlocks.set(key, entry);
    }
    updateSchedulerStage(entry, block);
    if (block.status === SCHEDULER_STAGE_STATUSES.DONE || block.status === SCHEDULER_STAGE_STATUSES.BLOCKED) {
      state.streamStageBlocks.delete(key);
    }
  }
}

function messageBodyFromParts(parts) {
  if (!Array.isArray(parts) || parts.length === 0) return "";
  const out = [];
  for (const part of parts) {
    const type = part.type;
    if (
      (type === MESSAGE_PART_TYPES.TEXT ||
        type === MESSAGE_PART_TYPES.REASONING ||
        type === MESSAGE_PART_TYPES.COMPACTION) &&
      part.text
    ) {
      out.push(part.text);
    }
  }
  return out.join("\n").trim();
}

function historyOutputBlocksFromParts(parts) {
  if (!Array.isArray(parts) || parts.length === 0) return [];
  return parts
    .map((part) => part && part[MESSAGE_PART_TYPES.OUTPUT_BLOCK] ? part[MESSAGE_PART_TYPES.OUTPUT_BLOCK] : null)
    .filter(Boolean);
}
