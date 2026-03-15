// ── Stage Inspector Panel ──────────────────────────────────────────────────
//
// Renders a collapsible stage inspector panel that shows:
//   1. A list of distinct stages for the current session
//   2. When a stage is selected, its event log in chronological order
//
// Data source: GET /session/{id}/events/stages  — list stage IDs
//              GET /session/{id}/events?stage_id=X — query events for a stage

/** Fetch distinct stage IDs for the current session. */
async function fetchStageIds(sessionId) {
  if (!sessionId) return [];
  try {
    const res = await api(`/session/${sessionId}/events/stages`);
    return await res.json();
  } catch {
    return [];
  }
}

/** Fetch filtered events for a session. */
async function fetchStageEvents(sessionId, filter = {}) {
  if (!sessionId) return [];
  const params = new URLSearchParams();
  if (filter.stage_id) params.set("stage_id", filter.stage_id);
  if (filter.execution_id) params.set("execution_id", filter.execution_id);
  if (filter.event_type) params.set("event_type", filter.event_type);
  if (filter.since != null) params.set("since", String(filter.since));
  if (filter.limit != null) params.set("limit", String(filter.limit));
  if (filter.offset != null) params.set("offset", String(filter.offset));
  const qs = params.toString();
  const url = `/session/${sessionId}/events${qs ? `?${qs}` : ""}`;
  try {
    const res = await api(url);
    return await res.json();
  } catch {
    return [];
  }
}

/** Render a single event row in the inspector. */
function renderStageEventRow(evt) {
  const ts = new Date(evt.ts).toLocaleTimeString();
  const eid = evt.execution_id ? short(evt.execution_id, 20) : "—";
  const sid = evt.stage_id ? short(evt.stage_id, 20) : "—";
  return `
    <tr class="stage-event-row">
      <td class="stage-event-ts">${escapeHtml(ts)}</td>
      <td class="stage-event-type"><code>${escapeHtml(evt.event_type)}</code></td>
      <td class="stage-event-eid" title="${escapeHtml(evt.execution_id || "")}">${escapeHtml(eid)}</td>
      <td class="stage-event-sid" title="${escapeHtml(evt.stage_id || "")}">${escapeHtml(sid)}</td>
    </tr>
  `;
}

function renderInspectBlockPayload(block) {
  const panel = nodes.stageInspectorPanel;
  if (!panel) return;

  const stageIds = Array.isArray(block.stage_ids) ? block.stage_ids : [];
  const events = Array.isArray(block.events) ? block.events : [];
  const selectedStageId = block.filter_stage_id || stageIds[0] || null;

  if (!stageIds.length && !events.length) {
    panel.classList.add("hidden");
    panel.innerHTML = "";
    return;
  }

  panel.classList.remove("hidden");

  const tabs = stageIds
    .map((sid) => {
      const active = sid === selectedStageId ? " active" : "";
      return `<button class="stage-tab-btn${active}" data-stage-id="${escapeHtml(sid)}">${escapeHtml(short(sid, 24))}</button>`;
    })
    .join("");

  const eventTable = events.length
    ? `
      <table class="stage-event-table">
        <thead>
          <tr>
            <th>Time</th>
            <th>Event</th>
            <th>Execution</th>
            <th>Stage</th>
          </tr>
        </thead>
        <tbody>${events.map(renderStageEventRow).join("")}</tbody>
      </table>
    `
    : `<p class="muted" style="padding: var(--space-3);">No events for this stage.</p>`;

  panel.innerHTML = `
    <div class="execution-panel-head">
      <div>
        <p class="label">Stage Inspector</p>
        <h4>${stageIds.length || (selectedStageId ? 1 : 0)} stage${stageIds.length === 1 ? "" : "s"}</h4>
      </div>
    </div>
    ${tabs ? `<div class="stage-tabs">${tabs}</div>` : ""}
    <div id="stageEventList" class="stage-event-list">${eventTable}</div>
  `;

  panel.querySelectorAll(".stage-tab-btn").forEach((btn) => {
    btn.addEventListener("click", async () => {
      panel
        .querySelectorAll(".stage-tab-btn")
        .forEach((item) => item.classList.remove("active"));
      btn.classList.add("active");
      if (state.selectedSession) {
        await loadStageEvents(state.selectedSession, btn.dataset.stageId);
      }
    });
  });
}

/** Render the stage inspector into the DOM. */
async function renderStageInspector(sessionId) {
  const panel = nodes.stageInspectorPanel;
  if (!panel) return;

  if (!sessionId) {
    panel.classList.add("hidden");
    panel.innerHTML = "";
    return;
  }

  const stageIds = await fetchStageIds(sessionId);
  if (!stageIds.length) {
    panel.classList.add("hidden");
    panel.innerHTML = "";
    return;
  }

  panel.classList.remove("hidden");

  // Build stage tab buttons
  const tabs = stageIds
    .map(
      (sid) =>
        `<button class="stage-tab-btn" data-stage-id="${escapeHtml(sid)}">${escapeHtml(short(sid, 24))}</button>`
    )
    .join("");

  panel.innerHTML = `
    <div class="execution-panel-head">
      <div>
        <p class="label">Stage Inspector</p>
        <h4>${stageIds.length} stage${stageIds.length === 1 ? "" : "s"}</h4>
      </div>
    </div>
    <div class="stage-tabs">${tabs}</div>
    <div id="stageEventList" class="stage-event-list"></div>
  `;

  // Wire tab click handlers
  panel.querySelectorAll(".stage-tab-btn").forEach((btn) => {
    btn.addEventListener("click", async () => {
      panel
        .querySelectorAll(".stage-tab-btn")
        .forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      const stageId = btn.dataset.stageId;
      await loadStageEvents(sessionId, stageId);
    });
  });

  // Auto-select the first stage
  const firstBtn = panel.querySelector(".stage-tab-btn");
  if (firstBtn) {
    firstBtn.classList.add("active");
    await loadStageEvents(sessionId, stageIds[0]);
  }
}

/** Load and render events for a specific stage into the event list area. */
async function loadStageEvents(sessionId, stageId) {
  const container = document.getElementById("stageEventList");
  if (!container) return;

  const events = await fetchStageEvents(sessionId, {
    stage_id: stageId,
    limit: 200,
  });

  if (!events.length) {
    container.innerHTML = `<p class="muted" style="padding: var(--space-3);">No events for this stage.</p>`;
    return;
  }

  container.innerHTML = `
    <table class="stage-event-table">
      <thead>
        <tr>
          <th>Time</th>
          <th>Event</th>
          <th>Execution</th>
          <th>Stage</th>
        </tr>
      </thead>
      <tbody>${events.map(renderStageEventRow).join("")}</tbody>
    </table>
  `;
}

/** Refresh the stage inspector for the current session. */
function refreshStageInspector() {
  if (!state.selectedSession) return;
  void renderStageInspector(state.selectedSession);
}
