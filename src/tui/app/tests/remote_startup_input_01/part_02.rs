#[test]
fn test_handle_server_event_available_models_updated_replaces_remote_model_catalog() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.is_remote = true;
    app.remote_available_entries = vec!["old-model".to_string()];
    app.remote_model_options = vec![crate::provider::ModelRoute {
        model: "old-model".to_string(),
        provider: "OldProvider".to_string(),
        api_method: "old-api".to_string(),
        available: false,
        detail: "old".to_string(),
        cheapness: None,
    }];

    app.handle_server_event(
        crate::protocol::ServerEvent::AvailableModelsUpdated {
            provider_name: Some("OpenAI".to_string()),
            provider_model: Some("new-model".to_string()),
            available_models: vec!["new-model".to_string(), "second-model".to_string()],
            available_model_routes: vec![crate::provider::ModelRoute {
                model: "new-model".to_string(),
                provider: "OpenAI".to_string(),
                api_method: "openai-oauth".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            }],
        },
        &mut remote,
    );

    assert_eq!(
        app.remote_available_entries,
        vec!["new-model".to_string(), "second-model".to_string()]
    );
    assert_eq!(app.remote_model_options.len(), 1);
    assert_eq!(app.remote_model_options[0].model, "new-model");
    assert_eq!(app.remote_model_options[0].provider, "OpenAI");
    assert!(app.remote_model_options[0].available);
    assert_eq!(app.remote_provider_name.as_deref(), Some("OpenAI"));
    assert_eq!(app.remote_provider_model.as_deref(), Some("new-model"));
}

#[test]
fn test_refresh_model_list_command_shows_summary_and_status_notice() {
    let mut app = create_refresh_summary_test_app(crate::provider::ModelCatalogRefreshSummary {
        model_count_before: 12,
        model_count_after: 15,
        models_added: 3,
        models_removed: 0,
        route_count_before: 20,
        route_count_after: 29,
        routes_added: 9,
        routes_removed: 0,
        routes_changed: 2,
    });

    assert!(super::model_context::handle_model_command(
        &mut app,
        "/refresh-model-list"
    ));

    assert_eq!(
        app.status_notice(),
        Some("Model list refreshed: +3 models, +9 routes, ~2 changed".to_string())
    );

    let last = app.display_messages.last().expect("display message");
    assert_eq!(last.role, "system");
    assert!(last.content.contains("**Model List Refresh Complete**"));
    assert!(last.content.contains("Models: 12 → 15  (+3 / -0)"));
    assert!(last.content.contains("Routes: 20 → 29  (+9 / -0 / ~2)"));
}

#[test]
fn test_remote_available_models_updated_after_refresh_shows_summary_and_updates_catalog() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let mut remote = crate::tui::backend::RemoteConnection::dummy();

    app.is_remote = true;
    app.pending_remote_model_refresh_snapshot = Some((
        vec!["old-model".to_string()],
        vec![crate::provider::ModelRoute {
            model: "old-model".to_string(),
            provider: "OpenAI".to_string(),
            api_method: "responses".to_string(),
            available: true,
            detail: "old detail".to_string(),
            cheapness: None,
        }],
    ));

    app.handle_server_event(
        crate::protocol::ServerEvent::AvailableModelsUpdated {
            provider_name: None,
            provider_model: None,
            available_models: vec!["old-model".to_string(), "new-model".to_string()],
            available_model_routes: vec![
                crate::provider::ModelRoute {
                    model: "old-model".to_string(),
                    provider: "OpenAI".to_string(),
                    api_method: "responses".to_string(),
                    available: true,
                    detail: "new detail".to_string(),
                    cheapness: None,
                },
                crate::provider::ModelRoute {
                    model: "new-model".to_string(),
                    provider: "OpenRouter".to_string(),
                    api_method: "chat".to_string(),
                    available: true,
                    detail: String::new(),
                    cheapness: None,
                },
            ],
        },
        &mut remote,
    );

    assert_eq!(
        app.status_notice(),
        Some("Model list refreshed: +1 models, +1 routes, ~1 changed".to_string())
    );
    assert_eq!(
        app.remote_available_entries,
        vec!["old-model".to_string(), "new-model".to_string()]
    );
    assert_eq!(app.remote_model_options.len(), 2);
    assert!(app.pending_remote_model_refresh_snapshot.is_none());

    let last = app.display_messages.last().expect("display message");
    assert_eq!(last.role, "system");
    assert!(last.content.contains("**Model List Refresh Complete**"));
    assert!(last.content.contains("Models: 1 → 2  (+1 / -0)"));
    assert!(last.content.contains("Routes: 1 → 2  (+1 / -0 / ~1)"));
}

#[test]
fn test_remote_model_command_opens_picker_even_if_remote_writer_is_blocked() {
    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_provider_model = Some("kimi-for-coding".to_string());
    app.remote_available_entries = vec!["kimi-for-coding".to_string()];
    app.remote_model_options = vec![crate::provider::ModelRoute {
        model: "kimi-for-coding".to_string(),
        provider: "Kimi Code".to_string(),
        api_method: "openai-compatible:kimi".to_string(),
        available: true,
        detail: "validated".to_string(),
        cheapness: None,
    }];
    app.input = "/model".to_string();
    app.cursor_pos = app.input.len();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut remote = rt.block_on(async { crate::tui::backend::RemoteConnection::dummy() });
    let writer = remote.writer();
    let writer_guard = rt.block_on(writer.lock());

    let result = rt.block_on(async {
        tokio::time::timeout(
            Duration::from_millis(50),
            app.handle_remote_key(KeyCode::Enter, KeyModifiers::empty(), &mut remote),
        )
        .await
    });

    drop(writer_guard);

    assert!(
        result.is_ok(),
        "/model should not block the TUI on the remote writer"
    );
    result
        .expect("remote model command should complete promptly")
        .expect("remote model command should not fail");

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("/model should open the remote model picker immediately");
    assert_eq!(picker.kind, crate::tui::PickerKind::Model);
    assert!(!picker.preview);
    assert!(
        picker
            .entries
            .iter()
            .any(|entry| entry.name == "kimi-for-coding"),
        "picker should include the already known Kimi model"
    );
}

#[test]
fn test_remote_model_command_does_not_refresh_catalog_implicitly() {
    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_provider_model = Some("kimi-for-coding".to_string());
    app.remote_available_entries = vec!["kimi-for-coding".to_string()];
    app.remote_model_options = vec![crate::provider::ModelRoute {
        model: "kimi-for-coding".to_string(),
        provider: "Kimi Code".to_string(),
        api_method: "openai-compatible:kimi".to_string(),
        available: true,
        detail: "validated".to_string(),
        cheapness: None,
    }];
    app.input = "/model".to_string();
    app.cursor_pos = app.input.len();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut remote = rt.block_on(async { crate::tui::backend::RemoteConnection::dummy() });
    let next_request_id = remote.next_request_id_for_test();

    rt.block_on(async {
        app.handle_remote_key(KeyCode::Enter, KeyModifiers::empty(), &mut remote)
            .await
            .expect("remote /model should be handled");
    });

    assert_eq!(
        remote.next_request_id_for_test(),
        next_request_id,
        "/model should open the picker from cached routes; /refresh-model-list is the explicit refresh command"
    );
}

#[test]
fn test_remote_model_picker_merges_configured_kimi_when_server_routes_are_present() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::provider_catalog::save_env_value_to_env_file(
        "KIMI_API_KEY",
        "kimi.env",
        Some("good-kimi-key"),
    )
    .expect("save Kimi key");
    crate::auth::validation::save(
        "kimi",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: Some(true),
            validated_models: vec!["kimi-for-coding".to_string()],
            summary: "tool_smoke: AUTH_TEST_OK".to_string(),
        },
    )
    .expect("save passing Kimi validation");
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_provider_model = Some("server-model".to_string());
    app.remote_available_entries.clear();
    app.remote_model_options = vec![crate::provider::ModelRoute {
        model: "server-model".to_string(),
        provider: "Server Provider".to_string(),
        api_method: "server".to_string(),
        available: true,
        detail: "server catalog".to_string(),
        cheapness: None,
    }];

    app.open_model_picker();

    match prev_home {
        Some(value) => crate::env::set_var("JCODE_HOME", value),
        None => crate::env::remove_var("JCODE_HOME"),
    }
    crate::auth::AuthStatus::invalidate_cache();

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("model picker should be open");
    assert!(
        picker
            .entries
            .iter()
            .any(|entry| entry.name == "server-model"),
        "server-provided routes should remain visible"
    );
    let kimi_entry = picker
        .entries
        .iter()
        .find(|entry| entry.name == "kimi-for-coding")
        .expect("configured Kimi route should be merged into the remote picker");
    assert!(
        kimi_entry.options.iter().any(|route| {
            route.provider == "Kimi Code"
                && route.api_method == "openai-compatible:kimi"
                && route.available
        }),
        "validated Kimi route should be selectable, got: {:?}",
        kimi_entry.options
    );
}

#[test]
fn test_model_picker_page_keys_move_selection() {
    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_model_options = (0..30)
        .map(|idx| crate::provider::ModelRoute {
            model: format!("server-model-{idx:02}"),
            provider: "Server Provider".to_string(),
            api_method: "server".to_string(),
            available: true,
            detail: "server catalog".to_string(),
            cheapness: None,
        })
        .collect();

    app.open_model_picker();
    assert_eq!(
        app.inline_interactive_state
            .as_ref()
            .expect("model picker should open")
            .selected,
        0
    );

    app.handle_inline_interactive_key(KeyCode::PageDown, KeyModifiers::empty())
        .expect("PageDown should be handled");
    let selected_after_page_down = app
        .inline_interactive_state
        .as_ref()
        .expect("model picker should stay open")
        .selected;
    assert!(
        selected_after_page_down > 0,
        "PageDown should advance the selected model"
    );

    app.handle_inline_interactive_key(KeyCode::End, KeyModifiers::empty())
        .expect("End should be handled");
    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("model picker should stay open");
    let last = picker.filtered.len().saturating_sub(1);
    assert_eq!(picker.selected, last);

    app.handle_inline_interactive_key(KeyCode::PageUp, KeyModifiers::empty())
        .expect("PageUp should be handled");
    assert!(
        app.inline_interactive_state
            .as_ref()
            .expect("model picker should stay open")
            .selected
            < last,
        "PageUp should move back from the last model"
    );

    app.handle_inline_interactive_key(KeyCode::Home, KeyModifiers::empty())
        .expect("Home should be handled");
    assert_eq!(
        app.inline_interactive_state
            .as_ref()
            .expect("model picker should stay open")
            .selected,
        0
    );
}

#[test]
fn test_model_picker_copilot_models_have_copilot_route() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::clear_runtime_env();
        save_test_saitec_session();
        save_test_provider_validation("copilot", &["grok-code-fast-1"]);
        let mut app = create_test_app();
        configure_test_remote_models_with_copilot(&mut app);

        app.open_model_picker();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");

        // grok-code-fast-1 is NOT in ALL_CLAUDE_MODELS or ALL_OPENAI_MODELS,
        // so it should get a copilot route
        let grok_entry = picker
            .entries
            .iter()
            .find(|m| m.name == "grok-code-fast-1")
            .expect("grok-code-fast-1 should be in picker");

        assert!(
            grok_entry.options.iter().any(|r| r.api_method == "copilot"),
            "grok-code-fast-1 should have a copilot route, got: {:?}",
            grok_entry.options
        );
    });
}

#[test]
fn test_model_picker_remote_comtegra_model_uses_comtegra_route_not_copilot() {
    with_temp_jcode_home(|| {
        save_test_provider_validation("comtegra", &["glm-51-nvfp4"]);
        let prev_key = std::env::var("COMTEGRA_API_KEY").ok();
        crate::env::set_var("COMTEGRA_API_KEY", "test-key");

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_available_entries = vec!["glm-51-nvfp4".to_string()];

        app.open_model_picker();

        match prev_key {
            Some(value) => crate::env::set_var("COMTEGRA_API_KEY", value),
            None => crate::env::remove_var("COMTEGRA_API_KEY"),
        }

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let glm_entry = picker
            .entries
            .iter()
            .find(|m| m.name == "glm-51-nvfp4")
            .expect("glm-51-nvfp4 should be in picker");

        assert!(
            glm_entry.options.iter().any(|r| {
                r.provider == "Comtegra GPU Cloud"
                    && r.api_method == "openai-compatible:comtegra"
                    && r.available
                    && !r.detail.contains("runtime not validated")
            }),
            "validated glm route should be Comtegra/api key, got: {:?}",
            glm_entry.options
        );
        assert!(
            !glm_entry.options.iter().any(|r| r.api_method == "copilot"),
            "glm route should not fall back to Copilot, got: {:?}",
            glm_entry.options
        );
    });
}

#[test]
fn test_remote_model_picker_adds_validated_kimi_and_blocks_failed_zai_routes() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::provider_catalog::save_env_value_to_env_file(
        "ZHIPU_API_KEY",
        "zai.env",
        Some("bad-zai-key"),
    )
    .expect("save Z.AI key");
    crate::provider_catalog::save_env_value_to_env_file(
        "KIMI_API_KEY",
        "kimi.env",
        Some("good-kimi-key"),
    )
    .expect("save Kimi key");
    crate::auth::validation::save(
        "zai",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: false,
            provider_smoke_ok: Some(false),
            tool_smoke_ok: None,
            validated_models: Vec::new(),
            summary: "provider_smoke: invalid api key".to_string(),
        },
    )
    .expect("save failed Z.AI validation");
    crate::auth::validation::save(
        "kimi",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: Some(true),
            validated_models: vec!["kimi-for-coding".to_string()],
            summary: "tool_smoke: AUTH_TEST_OK".to_string(),
        },
    )
    .expect("save passing Kimi validation");
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_available_entries = vec!["glm-4.5".to_string()];
    app.remote_model_options.clear();
    app.open_model_picker();

    match prev_home {
        Some(value) => crate::env::set_var("JCODE_HOME", value),
        None => crate::env::remove_var("JCODE_HOME"),
    }
    crate::auth::AuthStatus::invalidate_cache();

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("model picker should be open");
    assert!(
        !picker.entries.iter().any(|entry| entry.name == "glm-4.5"),
        "failed Z.AI validation should hide GLM route, got: {:?}",
        picker
            .entries
            .iter()
            .map(|entry| (&entry.name, &entry.options))
            .collect::<Vec<_>>()
    );

    let kimi_entry = picker
        .entries
        .iter()
        .find(|entry| entry.name == "kimi-for-coding")
        .expect("validated Kimi route should be added to the remote picker");
    assert!(
        kimi_entry.options.iter().any(|route| {
            route.provider == "Kimi Code"
                && route.api_method == "openai-compatible:kimi"
                && route.available
        }),
        "Kimi Code should be selectable when runtime validation passed, got: {:?}",
        kimi_entry.options
    );
}

#[test]
fn test_model_picker_remote_bedrock_model_hides_without_runtime_validation() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var("JCODE_HOME").ok();
    let prev_key = std::env::var(crate::provider::bedrock::API_KEY_ENV).ok();
    let prev_region = std::env::var(crate::provider::bedrock::REGION_ENV).ok();
    let temp = tempfile::tempdir().expect("tempdir");
    crate::env::set_var("JCODE_HOME", temp.path().display().to_string());
    crate::env::set_var(
        crate::provider::bedrock::API_KEY_ENV,
        "bedrock-api-key-test",
    );
    crate::env::set_var(crate::provider::bedrock::REGION_ENV, "us-east-2");
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_available_entries = vec!["us.amazon.nova-micro-v1:0".to_string()];

    app.open_model_picker();

    match prev_home {
        Some(value) => crate::env::set_var("JCODE_HOME", value),
        None => crate::env::remove_var("JCODE_HOME"),
    }
    match prev_key {
        Some(value) => crate::env::set_var(crate::provider::bedrock::API_KEY_ENV, value),
        None => crate::env::remove_var(crate::provider::bedrock::API_KEY_ENV),
    }
    match prev_region {
        Some(value) => crate::env::set_var(crate::provider::bedrock::REGION_ENV, value),
        None => crate::env::remove_var(crate::provider::bedrock::REGION_ENV),
    }
    crate::auth::AuthStatus::invalidate_cache();

    assert!(
        app.inline_interactive_state.is_none(),
        "unvalidated Bedrock route should be hidden from the picker"
    );
    assert_eq!(app.status_notice(), Some("No models available".to_string()));
}

#[test]
fn test_remote_model_picker_filters_kimi_compatible_generic_claude_routes_without_provider_name() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let env_keys = [
        "JCODE_HOME",
        "KIMI_API_KEY",
        "ZHIPU_API_KEY",
        "ZAI_API_KEY",
        "BAILIAN_CODING_PLAN_API_KEY",
        "JCODE_OPENROUTER_API_BASE",
        "JCODE_OPENROUTER_API_KEY_NAME",
        "JCODE_OPENROUTER_ENV_FILE",
        "JCODE_OPENROUTER_CACHE_NAMESPACE",
        "JCODE_NAMED_PROVIDER_PROFILE",
        "JCODE_PROVIDER_PROFILE_ACTIVE",
    ];
    let saved_env = env_keys
        .iter()
        .map(|key| (*key, std::env::var_os(key)))
        .collect::<Vec<_>>();
    crate::env::set_var("JCODE_HOME", temp.path());
    for key in env_keys.iter().copied().filter(|key| *key != "JCODE_HOME") {
        crate::env::remove_var(key);
    }
    crate::auth::validation::save(
        "kimi",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: Some(true),
            validated_models: vec!["kimi-for-coding".to_string()],
            summary: "tool_smoke: AUTH_TEST_OK".to_string(),
        },
    )
    .expect("save passing Kimi validation");
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.is_remote = true;
    app.remote_provider_name = None;
    app.remote_provider_model = Some("kimi-for-coding".to_string());
    app.remote_model_options = vec![
        crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "openai-compatible".to_string(),
            available: true,
            detail: "custom endpoint".to_string(),
            cheapness: None,
        },
        crate::provider::ModelRoute {
            model: "claude-opus-4-6".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "openai-compatible".to_string(),
            available: true,
            detail: "custom endpoint".to_string(),
            cheapness: None,
        },
        crate::provider::ModelRoute {
            model: "claude-sonnet-4-6".to_string(),
            provider: "OpenRouter".to_string(),
            api_method: "openrouter".to_string(),
            available: true,
            detail: String::new(),
            cheapness: None,
        },
    ];

    app.open_model_picker();

    for (key, value) in saved_env {
        match value {
            Some(value) => crate::env::set_var(key, value),
            None => crate::env::remove_var(key),
        }
    }
    crate::auth::AuthStatus::invalidate_cache();

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("model picker should be open");
    let names = picker
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"kimi-for-coding"), "entries: {:?}", names);
    assert!(
        !names.iter().any(|name| name.starts_with("claude-")),
        "Kimi-compatible remote picker should not expose Claude routes: {:?}",
        names
    );
}

#[test]
fn test_model_picker_preserves_recommendation_priority_order() {
    with_temp_jcode_home(|| {
        save_test_provider_validation(
            "openai",
            &[
                "gpt-5.2",
                "gpt-5.5",
                "gpt-5.4",
                "gpt-5.4-pro",
                "gpt-5.3-codex-spark",
                "gpt-5.3-codex",
                "claude-opus-4-7",
            ],
        );
        let mut app = create_test_app();
        configure_test_remote_models_with_openai_recommendations(&mut app);

        app.open_model_picker();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");

        let model_names: Vec<&str> = picker.entries.iter().map(|m| m.name.as_str()).collect();

        assert_eq!(model_names.first().copied(), Some("gpt-5.2"));

        let gpt55 = picker
            .entries
            .iter()
            .position(|model| model.name == "gpt-5.5")
            .expect("gpt-5.5 should be present");
        let gpt54 = picker
            .entries
            .iter()
            .position(|model| model.name == "gpt-5.4")
            .expect("gpt-5.4 should be present");
        let gpt54_pro = picker
            .entries
            .iter()
            .position(|model| model.name == "gpt-5.4-pro")
            .expect("gpt-5.4-pro should be present");
        let claude_opus = picker
            .entries
            .iter()
            .position(|model| model.name == "claude-opus-4-7")
            .expect("claude-opus-4-7 should be present");
        let spark = picker
            .entries
            .iter()
            .position(|model| model.name == "gpt-5.3-codex-spark")
            .expect("gpt-5.3-codex-spark should be present");
        let codex = picker
            .entries
            .iter()
            .position(|model| model.name == "gpt-5.3-codex")
            .expect("gpt-5.3-codex should be present");

        assert!(
            gpt55 < claude_opus,
            "gpt-5.5 should rank ahead of claude-opus-4-7, got {:?}",
            model_names
        );
        assert!(
            claude_opus < gpt54,
            "claude-opus-4-7 should rank ahead of unrecommended gpt-5.4, got {:?}",
            model_names
        );
        assert!(
            claude_opus < gpt54_pro,
            "claude-opus-4-7 should rank ahead of unrecommended gpt-5.4-pro, got {:?}",
            model_names
        );
        assert!(
            picker.entries[gpt55].recommended,
            "gpt-5.5 should be recommended"
        );
        assert!(
            picker.entries[claude_opus].recommended,
            "claude-opus-4-7 should be recommended"
        );
        assert!(
            !picker.entries[gpt54].recommended,
            "gpt-5.4 should not be recommended"
        );
        assert!(
            !picker.entries[gpt54_pro].recommended,
            "gpt-5.4-pro should not be recommended"
        );
        assert!(
            !picker.entries[spark].recommended,
            "gpt-5.3-codex-spark should not be recommended"
        );
        assert!(
            !picker.entries[codex].recommended,
            "gpt-5.3-codex should not be recommended"
        );
    });
}
