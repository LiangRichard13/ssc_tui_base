#[test]
fn test_handle_server_event_updates_status_detail() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.handle_server_event(
        crate::protocol::ServerEvent::StatusDetail {
            detail: "reusing websocket".to_string(),
        },
        &mut remote,
    );

    assert_eq!(app.status_detail.as_deref(), Some("reusing websocket"));
}

#[test]
fn test_handle_server_event_transcript_replace_updates_input() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.input = "old draft".to_string();
    app.cursor_pos = app.input.len();

    app.handle_server_event(
        crate::protocol::ServerEvent::Transcript {
            text: "new dictated text".to_string(),
            mode: crate::protocol::TranscriptMode::Replace,
        },
        &mut remote,
    );

    assert_eq!(app.input, "new dictated text");
    assert_eq!(app.cursor_pos, app.input.len());
    assert_eq!(
        app.status_notice(),
        Some("Transcript replaced input".to_string())
    );
}

#[test]
fn test_local_bus_dictation_completion_applies_transcript() {
    let mut app = create_test_app();
    let session_id = app.session.id.clone();
    app.input = "draft".to_string();
    app.cursor_pos = app.input.len();
    app.dictation_in_flight = true;
    app.dictation_request_id = Some("dictation_123".to_string());
    app.dictation_target_session_id = Some(session_id.clone());

    crate::tui::app::local::handle_bus_event(
        &mut app,
        Ok(crate::bus::BusEvent::DictationCompleted {
            dictation_id: "dictation_123".to_string(),
            session_id: Some(session_id),
            text: " dictated text".to_string(),
            mode: crate::protocol::TranscriptMode::Append,
        }),
    );

    assert_eq!(app.input, "draft dictated text");
    assert_eq!(app.status_notice(), Some("Transcript appended".to_string()));
}

#[test]
fn test_remote_api_key_login_escape_closes_text_entry_overlay() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.set_pending_api_key_login_for_tests("kimi", "Kimi", "KIMI_API_KEY");
    app.input = "secret-api-key".to_string();
    app.cursor_pos = app.input.len();

    rt.block_on(app.handle_remote_key(KeyCode::Esc, KeyModifiers::empty(), &mut remote))
        .expect("esc should close the remote API-key login overlay");

    assert!(
        app.pending_login.is_none(),
        "remote API-key login should be dismissed by esc"
    );
    assert_eq!(app.input(), "");
    let last = app
        .display_messages()
        .last()
        .expect("missing cancellation message");
    assert!(last.content.contains("Login cancelled."));
}

#[test]
fn test_remote_up_arrow_walks_back_through_multiple_history_entries() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.input = "first prompt".to_string();
    app.submit_input();
    app.input = "second prompt".to_string();
    app.submit_input();
    app.input = "third prompt".to_string();
    app.submit_input();

    rt.block_on(app.handle_remote_key(KeyCode::Up, KeyModifiers::empty(), &mut remote))
        .expect("first up should recall the newest history entry in remote mode");
    assert_eq!(app.input(), "third prompt");

    rt.block_on(app.handle_remote_key(KeyCode::Up, KeyModifiers::empty(), &mut remote))
        .expect("second up should continue walking backward in remote mode");
    assert_eq!(app.input(), "second prompt");

    rt.block_on(app.handle_remote_key(KeyCode::Up, KeyModifiers::empty(), &mut remote))
        .expect("third up should reach the oldest entry in remote mode");
    assert_eq!(app.input(), "first prompt");
}
