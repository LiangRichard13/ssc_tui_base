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
fn test_remote_tick_dispatches_pending_model_switch_without_extra_keypress() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.is_remote = true;
    app.pending_model_switch = Some("kimi:kimi-for-coding".to_string());

    rt.block_on(crate::tui::app::remote::handle_tick(&mut app, &mut remote));

    assert!(
        app.pending_model_switch.is_none(),
        "remote tick should dispatch queued model switches even without another keypress"
    );
}

#[test]
fn test_remote_successful_kimi_turn_marks_saved_key_runtime_validated() {
    with_temp_jcode_home(|| {
        let config_dir = crate::storage::app_config_dir().expect("config dir");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::write(config_dir.join("kimi.env"), "KIMI_API_KEY=secret-kimi-key\n")
            .expect("write Kimi env file");
        crate::auth::AuthStatus::invalidate_cache();

        let mut app = create_test_app();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let mut remote = crate::tui::backend::RemoteConnection::dummy();

        app.is_remote = true;
        app.remote_provider_model = Some("kimi-for-coding".to_string());
        app.remote_model_options = vec![crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "openai-compatible:kimi".to_string(),
            available: false,
            detail: "runtime not validated".to_string(),
            cheapness: None,
        }];
        app.current_message_id = Some(42);
        app.is_processing = true;

        app.handle_server_event(
            crate::protocol::ServerEvent::TextDelta {
                text: "Kimi response".to_string(),
            },
            &mut remote,
        );
        app.handle_server_event(crate::protocol::ServerEvent::Done { id: 42 }, &mut remote);

        let record = crate::auth::validation::get("kimi")
            .expect("successful Kimi turn should persist runtime validation");
        assert!(record.success);
        assert_eq!(record.provider_smoke_ok, Some(true));
        assert!(
            record
                .validated_models
                .iter()
                .any(|model| model == "kimi-for-coding"),
            "Kimi validation should include the successful model, got {:?}",
            record.validated_models
        );

        let route = App::remote_openai_compatible_route_for_model("kimi-for-coding")
            .expect("Kimi route should resolve after validation");
        assert!(route.available, "Kimi route should become selectable");
    });
}

#[test]
fn test_remote_successful_kimi_turn_refreshes_open_login_picker_validation_state() {
    with_temp_jcode_home(|| {
        let config_dir = crate::storage::app_config_dir().expect("config dir");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::write(config_dir.join("kimi.env"), "KIMI_API_KEY=secret-kimi-key\n")
            .expect("write Kimi env file");
        crate::auth::AuthStatus::invalidate_cache();

        let mut app = create_test_app();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let mut remote = crate::tui::backend::RemoteConnection::dummy();

        app.open_saitec_base_model_login_picker();
        for _ in 0..4 {
            app.handle_key(KeyCode::Down, KeyModifiers::empty())
                .expect("move selection to Kimi");
        }

        let backend = ratatui::backend::TestBackend::new(120, 40);
        let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| crate::tui::ui::draw(frame, &app))
            .expect("draw login picker before validation");
        let before = buffer_to_text(&terminal);
        assert!(
            before.contains("Kimi Code") && before.contains("not validated yet"),
            "test precondition should show unvalidated Kimi picker detail, got:\n{before}"
        );

        app.is_remote = true;
        app.remote_provider_model = Some("kimi-for-coding".to_string());
        app.remote_model_options = vec![crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "openai-compatible:kimi".to_string(),
            available: false,
            detail: "runtime not validated".to_string(),
            cheapness: None,
        }];
        app.current_message_id = Some(42);
        app.is_processing = true;

        app.handle_server_event(
            crate::protocol::ServerEvent::TextDelta {
                text: "Kimi response".to_string(),
            },
            &mut remote,
        );
        app.handle_server_event(crate::protocol::ServerEvent::Done { id: 42 }, &mut remote);

        terminal
            .draw(|frame| crate::tui::ui::draw(frame, &app))
            .expect("draw login picker after validation");
        let after = buffer_to_text(&terminal);
        assert!(
            after.contains("Kimi Code") && after.contains("runtime validated"),
            "open login picker should refresh after successful Kimi turn, got:\n{after}"
        );
    });
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
