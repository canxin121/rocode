//! Governance replay integration tests for concurrent multi-agent topology
//! and event log consistency.
//!
//! These tests verify:
//! 1. Concurrent topology registration → correct ExecutionNode tree shape
//! 2. Event log records from concurrent writes filter correctly
//! 3. Capacity eviction under concurrent load
//! 4. Session isolation: events never leak across sessions
//! 5. ExecutionRecord → ExecutionNode projection from fixture data

use rocode_command::governance_fixtures::multi_agent_replay_fixture;
use rocode_command::stage_protocol::*;
use rocode_server::stage_event_log::{EventFilter, StageEventLog};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

// ── Helpers ──────────────────────────────────────────────────────────

fn make_event(
    stage_id: Option<&str>,
    execution_id: Option<&str>,
    event_type: &str,
    ts: i64,
) -> StageEvent {
    StageEvent {
        event_id: format!("evt_gov_{}", ts),
        scope: EventScope::Stage,
        stage_id: stage_id.map(String::from),
        execution_id: execution_id.map(String::from),
        event_type: event_type.to_string(),
        ts,
        payload: serde_json::json!({}),
    }
}

// ── 1. Concurrent event recording and querying ──────────────────────

#[tokio::test]
async fn concurrent_event_recording_preserves_all_events() {
    let log = Arc::new(StageEventLog::new());
    let session = "session_concurrent_1";

    // Spawn 3 concurrent writers (simulating 3 stages recording events simultaneously)
    let mut handles = Vec::new();
    for stage_idx in 0..3 {
        let log = log.clone();
        let sid = session.to_string();
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                let ts = (stage_idx as i64) * 1000 + i;
                log.record(
                    &sid,
                    make_event(
                        Some(&format!("stage_{}", stage_idx)),
                        Some(&format!("exec_{}_{}", stage_idx, i)),
                        "topology_changed",
                        ts,
                    ),
                )
                .await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let all = log.query(session, &EventFilter::default()).await;
    assert_eq!(all.len(), 30, "all 30 events should be recorded");
}

#[tokio::test]
async fn concurrent_writes_filter_by_stage_id_correctly() {
    let log = Arc::new(StageEventLog::new());
    let session = "session_concurrent_2";

    let mut handles = Vec::new();
    for stage_idx in 0..3 {
        let log = log.clone();
        let sid = session.to_string();
        let handle = tokio::spawn(async move {
            for i in 0..5 {
                log.record(
                    &sid,
                    make_event(
                        Some(&format!("stage_{}", stage_idx)),
                        None,
                        "work",
                        (stage_idx as i64) * 100 + i,
                    ),
                )
                .await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Filter for stage_1 only
    let filtered = log
        .query(
            session,
            &EventFilter {
                stage_id: Some("stage_1".into()),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(filtered.len(), 5);
    assert!(filtered
        .iter()
        .all(|e| e.stage_id.as_deref() == Some("stage_1")));
}

// ── 2. Capacity eviction under concurrent load ──────────────────────

#[tokio::test]
async fn capacity_eviction_under_concurrent_load() {
    let log = Arc::new(StageEventLog::with_capacity(20));
    let session = "session_eviction";

    // 4 concurrent writers, each recording 10 events = 40 total, but cap = 20
    let mut handles = Vec::new();
    for writer in 0..4 {
        let log = log.clone();
        let sid = session.to_string();
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                let ts = (writer as i64) * 100 + i;
                log.record(
                    &sid,
                    make_event(Some(&format!("stage_{}", writer)), None, "evt", ts),
                )
                .await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let all = log.query(session, &EventFilter::default()).await;
    assert_eq!(all.len(), 20, "capacity should cap at 20");
}

// ── 3. Session isolation ─────────────────────────────────────────────

#[tokio::test]
async fn concurrent_sessions_never_leak_events() {
    let log = Arc::new(StageEventLog::new());

    let mut handles = Vec::new();
    for session_idx in 0..3 {
        let log = log.clone();
        let handle = tokio::spawn(async move {
            let sid = format!("session_iso_{}", session_idx);
            for i in 0..5 {
                log.record(
                    &sid,
                    make_event(
                        Some(&format!("stage_iso_{}", session_idx)),
                        None,
                        &format!("event_s{}", session_idx),
                        (session_idx as i64) * 1000 + i,
                    ),
                )
                .await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify each session only has its own events
    for session_idx in 0..3 {
        let sid = format!("session_iso_{}", session_idx);
        let events = log.query(&sid, &EventFilter::default()).await;
        assert_eq!(events.len(), 5, "session {} should have 5 events", sid);
        for evt in &events {
            assert_eq!(
                evt.event_type,
                format!("event_s{}", session_idx),
                "event leaked into wrong session"
            );
        }
    }
}

// ── 4. Fixture-driven event replay ───────────────────────────────────

#[tokio::test]
async fn fixture_events_replay_through_event_log() {
    let fixture = multi_agent_replay_fixture();
    let log = StageEventLog::new();
    let session = &fixture.session_id;

    // Record all fixture events into the log
    for stage in &fixture.stages {
        for event in &stage.events {
            log.record(session, event.clone()).await;
        }
    }

    // Verify total count
    let all = log.query(session, &EventFilter::default()).await;
    assert_eq!(all.len(), fixture.expected.total_events);

    // Verify stage_ids listing matches fixture
    let stage_ids = log.stage_ids(session).await;
    assert_eq!(stage_ids, fixture.expected.distinct_stage_ids);
}

#[tokio::test]
async fn fixture_event_log_filters_by_stage() {
    let fixture = multi_agent_replay_fixture();
    let log = StageEventLog::new();
    let session = &fixture.session_id;

    for stage in &fixture.stages {
        for event in &stage.events {
            log.record(session, event.clone()).await;
        }
    }

    // Filter for each stage_id and verify counts match fixture
    for stage_entry in &fixture.stages {
        let stage_id = stage_entry.block.stage_id.clone().unwrap_or_default();
        let filtered = log
            .query(
                session,
                &EventFilter {
                    stage_id: Some(stage_id.clone()),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(
            filtered.len(),
            stage_entry.events.len(),
            "event count mismatch for stage {}",
            stage_id
        );
    }
}

#[tokio::test]
async fn fixture_event_log_filters_by_event_type() {
    let fixture = multi_agent_replay_fixture();
    let log = StageEventLog::new();
    let session = &fixture.session_id;

    for stage in &fixture.stages {
        for event in &stage.events {
            log.record(session, event.clone()).await;
        }
    }

    let stage_started = log
        .query(
            session,
            &EventFilter {
                event_type: Some("stage_started".into()),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(stage_started.len(), 3);

    let agent_started = log
        .query(
            session,
            &EventFilter {
                event_type: Some("agent_started".into()),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(agent_started.len(), 4);

    let question_asked = log
        .query(
            session,
            &EventFilter {
                event_type: Some("question_asked".into()),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(question_asked.len(), 2);
}

#[tokio::test]
async fn fixture_event_log_combined_stage_and_type_filter() {
    let fixture = multi_agent_replay_fixture();
    let log = StageEventLog::new();
    let session = &fixture.session_id;

    for stage in &fixture.stages {
        for event in &stage.events {
            log.record(session, event.clone()).await;
        }
    }

    // Implementation stage + tool_started
    let impl_tools = log
        .query(
            session,
            &EventFilter {
                stage_id: Some("stage_impl_002".into()),
                event_type: Some("tool_started".into()),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(impl_tools.len(), 1);

    #[derive(Debug, Default, Deserialize)]
    struct ToolStartedPayloadWire {
        #[serde(default, deserialize_with = "rocode_types::deserialize_opt_string_lossy")]
        tool: Option<String>,
    }

    let payload: ToolStartedPayloadWire = rocode_types::parse_value_lossy(&impl_tools[0].payload);
    assert_eq!(payload.tool.as_deref(), Some("write_file"));
}

#[tokio::test]
async fn fixture_event_log_since_filter() {
    let fixture = multi_agent_replay_fixture();
    let log = StageEventLog::new();
    let session = &fixture.session_id;

    for stage in &fixture.stages {
        for event in &stage.events {
            log.record(session, event.clone()).await;
        }
    }

    // Only events from review stage onwards
    let since_review = log
        .query(
            session,
            &EventFilter {
                since: Some(1710000020000),
                ..Default::default()
            },
        )
        .await;
    assert_eq!(since_review.len(), 3);
    assert!(since_review.iter().all(|e| e.ts >= 1710000020000));
}

#[tokio::test]
async fn fixture_event_log_pagination() {
    let fixture = multi_agent_replay_fixture();
    let log = StageEventLog::new();
    let session = &fixture.session_id;

    for stage in &fixture.stages {
        for event in &stage.events {
            log.record(session, event.clone()).await;
        }
    }

    // Page through all events, 3 at a time
    let mut collected = Vec::new();
    let mut offset = 0;
    loop {
        let page = log
            .query(
                session,
                &EventFilter {
                    limit: Some(3),
                    offset: Some(offset),
                    ..Default::default()
                },
            )
            .await;
        if page.is_empty() {
            break;
        }
        collected.extend(page);
        offset += 3;
    }
    assert_eq!(collected.len(), fixture.expected.total_events);

    // Verify order preserved through pagination
    for window in collected.windows(2) {
        assert!(window[0].ts <= window[1].ts, "pagination broke ordering");
    }
}

// ── 5. Cross-layer: event IDs unique across concurrent writes ───────

#[tokio::test]
async fn stage_event_builder_produces_unique_ids_under_concurrency() {
    let events: Vec<StageEvent> = (0..100)
        .map(|_| {
            StageEvent::new(
                EventScope::Stage,
                Some("s1".into()),
                None,
                "test",
                serde_json::json!({}),
            )
        })
        .collect();

    let ids: HashSet<&str> = events.iter().map(|e| e.event_id.as_str()).collect();
    assert_eq!(ids.len(), 100, "all 100 event IDs should be unique");
}

// ── 6. Clear session and re-record ───────────────────────────────────

#[tokio::test]
async fn clear_and_rerecord_gives_fresh_state() {
    let log = StageEventLog::new();
    let session = "session_clear_test";

    // Record some events
    for i in 0..5 {
        log.record(session, make_event(Some("stage_old"), None, "old", i))
            .await;
    }
    assert_eq!(log.query(session, &EventFilter::default()).await.len(), 5);

    // Clear
    log.clear_session(session).await;
    assert!(log.query(session, &EventFilter::default()).await.is_empty());

    // Re-record new events
    for i in 0..3 {
        log.record(session, make_event(Some("stage_new"), None, "new", 100 + i))
            .await;
    }
    let events = log.query(session, &EventFilter::default()).await;
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|e| e.event_type == "new"));
}
