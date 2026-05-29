#[test]
fn test_exact_model_command_waits_for_enter_before_opening_picker() {
    let mut app = create_test_app();
    configure_test_remote_models(&mut app);

    for c in "/model".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }

    assert!(
        app.inline_interactive_state.is_none(),
        "typing exact /model should not synchronously open a preview"
    );
    assert_eq!(app.input(), "/model");

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .unwrap();

    let picker = app
        .inline_interactive_state
        .as_ref()
        .unwrap_or_else(|| {
            panic!(
                "exact /model command should leave the full picker open; status={:?}, pending_model_switch={:?}, messages={:?}",
                app.status_notice(),
                app.pending_model_switch,
                app.display_messages
                    .iter()
                    .map(|message| (message.role.as_str(), message.content.as_str()))
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        !picker.preview,
        "exact /model should execute the command, not select the preview row"
    );
    assert!(app.pending_model_switch.is_none());
    assert!(app.input().is_empty());
    assert!(
        picker.entries.len() > 1,
        "full model picker should show available routes"
    );
}

#[test]
fn test_agents_review_picker_saves_config_override() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        configure_test_remote_models(&mut app);
        app.open_agent_model_picker(crate::tui::AgentModelTarget::Review);

        let selected = app
            .inline_interactive_state
            .as_ref()
            .and_then(|picker| {
                picker.filtered.iter().position(|&idx| {
                    matches!(
                        picker.entries[idx].action,
                        crate::tui::PickerAction::AgentModelChoice {
                            target: crate::tui::AgentModelTarget::Review,
                            clear_override: false,
                        }
                    )
                })
            })
            .expect("review picker should include at least one model option");
        app.inline_interactive_state.as_mut().unwrap().selected = selected;
        let selected_model_idx = app.inline_interactive_state.as_ref().unwrap().filtered[selected];
        app.inline_interactive_state.as_mut().unwrap().entries[selected_model_idx].options[0]
            .available = true;

        let expected = {
            let picker = app.inline_interactive_state.as_ref().unwrap();
            let entry = &picker.entries[picker.filtered[selected]];
            let base = if entry.effort.is_some() {
                entry
                    .name
                    .rsplit_once(" (")
                    .map(|(base, _)| base.to_string())
                    .unwrap_or_else(|| entry.name.clone())
            } else {
                entry.name.clone()
            };
            let route = &entry.options[entry.selected_option];
            if route.api_method == "copilot" {
                format!("copilot:{}", base)
            } else if route.api_method == "cursor" {
                format!("cursor:{}", base)
            } else if route.api_method == "openrouter" && route.provider != "auto" {
                if base.contains('/') {
                    format!("{}@{}", base, route.provider)
                } else {
                    format!("anthropic/{}@{}", base, route.provider)
                }
            } else {
                base
            }
        };

        app.handle_inline_interactive_key(KeyCode::Enter, KeyModifiers::NONE)
            .expect("save agent model override");

        let cfg = crate::config::Config::load();
        assert_eq!(cfg.autoreview.model.as_deref(), Some(expected.as_str()));
        assert!(app.inline_interactive_state.is_none());
    });
}

#[test]
fn test_model_command_suggestions_include_matching_models() {
    let mut app = create_test_app();
    configure_test_remote_models(&mut app);

    let suggestions = app.get_suggestions_for("/model g52c");
    assert_eq!(
        suggestions.first().map(|(cmd, _)| cmd.as_str()),
        Some("/model gpt-5.2-codex")
    );
}

#[test]
fn test_model_command_trailing_space_shows_model_suggestions() {
    let mut app = create_test_app();
    configure_test_remote_models(&mut app);

    let suggestions = app.get_suggestions_for("/model ");
    assert!(
        suggestions
            .iter()
            .any(|(cmd, _)| cmd == "/model gpt-5.3-codex")
    );
}

#[test]
fn test_remote_logged_out_model_suggestions_filter_openrouter_catalog_after_logout() {
    with_temp_jcode_home(|| {
        save_test_saitec_session();
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
        .expect("save previous Kimi validation");
        crate::saitec::auth::clear_session().expect("simulate SAITEC logout");

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_provider_name = Some("openrouter".to_string());
        app.remote_provider_model = Some("openai/gpt-5.4".to_string());
        app.remote_available_entries = vec![
            "openai/gpt-5.4".to_string(),
            "anthropic/claude-sonnet-4".to_string(),
        ];
        app.remote_model_options = vec![
            crate::provider::ModelRoute {
                model: "openai/gpt-5.4".to_string(),
                provider: "OpenRouter".to_string(),
                api_method: "openrouter".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            },
            crate::provider::ModelRoute {
                model: "kimi-for-coding".to_string(),
                provider: "Kimi Code".to_string(),
                api_method: "openai-compatible:kimi".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            },
        ];

        let suggestions = app.get_suggestions_for("/model ");
        let commands: Vec<&str> = suggestions.iter().map(|(cmd, _)| cmd.as_str()).collect();

        assert!(commands.contains(&"/model DeepSeek V4 Pro"));
        assert!(commands.contains(&"/model Kimi K2.5"));
        assert!(!commands.contains(&"/model openai/gpt-5.4"));
        assert!(!commands.contains(&"/model anthropic/claude-sonnet-4"));
        assert!(!commands.contains(&"/model kimi-for-coding"));
        assert!(
            commands
                .iter()
                .all(|cmd| !cmd.to_ascii_lowercase().contains("openrouter")),
            "remote logged-out /model suggestions leaked OpenRouter names: {commands:?}"
        );
        assert!(
            commands.iter().all(|cmd| cmd
                .strip_prefix("/model ")
                .map(crate::subscription_catalog::is_curated_model)
                .unwrap_or(false)),
            "remote logged-out /model suggestions leaked non-curated routes: {commands:?}"
        );

        let provider_suggestions = app.get_suggestions_for("/model openai/gpt-5.4@");
        assert!(
            provider_suggestions.is_empty(),
            "remote logged-out provider suggestions should be hidden: {provider_suggestions:?}"
        );
    });
}

#[test]
fn test_model_command_provider_suggestions_include_openrouter_routes() {
    with_temp_jcode_home(|| {
        save_test_saitec_session();
        let mut app = create_test_app();
        configure_test_remote_openrouter_provider_routes(&mut app);

        let suggestions = app.get_suggestions_for("/model anthropic/claude-sonnet-4@");
        let commands: Vec<&str> = suggestions.iter().map(|(cmd, _)| cmd.as_str()).collect();

        assert!(commands.contains(&"/model anthropic/claude-sonnet-4@auto"));
        assert!(commands.contains(&"/model anthropic/claude-sonnet-4@Fireworks"));
        assert!(commands.contains(&"/model anthropic/claude-sonnet-4@OpenAI"));
    });
}

#[test]
fn test_model_command_provider_suggestions_rank_matching_provider_prefix() {
    with_temp_jcode_home(|| {
        save_test_saitec_session();
        let mut app = create_test_app();
        configure_test_remote_openrouter_provider_routes(&mut app);

        let suggestions = app.get_suggestions_for("/model anthropic/claude-sonnet-4@fi");
        assert_eq!(
            suggestions.first().map(|(cmd, _)| cmd.as_str()),
            Some("/model anthropic/claude-sonnet-4@Fireworks")
        );
    });
}

#[test]
fn test_model_command_provider_suggestions_normalize_bare_openai_model_to_openrouter_catalog_id() {
    let (app, _set_model_calls) = create_openrouter_spec_capture_test_app();

    let suggestions = app.get_suggestions_for("/model gpt-5.4@op");
    assert_eq!(
        suggestions.first().map(|(cmd, _)| cmd.as_str()),
        Some("/model openai/gpt-5.4@OpenAI")
    );
}

#[test]
fn test_model_command_provider_suggestions_include_auto_for_normalized_bare_openai_model() {
    let (app, _set_model_calls) = create_openrouter_spec_capture_test_app();

    let suggestions = app.get_suggestions_for("/model gpt-5.4@");
    let commands: Vec<&str> = suggestions.iter().map(|(cmd, _)| cmd.as_str()).collect();

    assert!(commands.contains(&"/model openai/gpt-5.4@auto"));
    assert!(commands.contains(&"/model openai/gpt-5.4@OpenAI"));
}

#[test]
fn test_remote_fallback_provider_suggestions_normalize_bare_openai_openrouter_routes() {
    with_temp_jcode_home(|| {
        let prev_api_key = std::env::var_os("OPENROUTER_API_KEY");
        crate::env::set_var("OPENROUTER_API_KEY", "test-openrouter-key");
        crate::auth::AuthStatus::invalidate_cache();

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_provider_model = Some("gpt-5.4".to_string());
        app.remote_available_entries = vec!["gpt-5.4".to_string()];
        app.remote_model_options.clear();

        let suggestions = app.get_suggestions_for("/model gpt-5.4@");
        let commands: Vec<&str> = suggestions.iter().map(|(cmd, _)| cmd.as_str()).collect();

        assert!(commands.contains(&"/model openai/gpt-5.4@auto"));
        assert!(commands.contains(&"/model openai/gpt-5.4@OpenAI"));

        if let Some(prev_api_key) = prev_api_key {
            crate::env::set_var("OPENROUTER_API_KEY", prev_api_key);
        } else {
            crate::env::remove_var("OPENROUTER_API_KEY");
        }
        crate::auth::AuthStatus::invalidate_cache();
    });
}

#[test]
fn test_login_command_suggestions_follow_provider_catalog() {
    let app = create_test_app();
    let suggestions = app.get_suggestions_for("/login ");

    assert!(suggestions.iter().any(|(cmd, detail)| {
        cmd == "/login jcode"
            && *detail == crate::provider_catalog::JCODE_LOGIN_PROVIDER.menu_detail
    }));
    assert!(suggestions.iter().any(|(cmd, detail)| {
        cmd == "/login base-models" && *detail == "Open the filtered base-model provider picker"
    }));
    for provider in crate::provider_catalog::saitec_visible_base_model_providers() {
        assert!(
            suggestions.iter().any(|(cmd, detail)| {
                cmd == &format!("/login {}", provider.id) && *detail == provider.menu_detail
            }),
            "missing /login suggestion for provider `{}`",
            provider.id
        );
    }
}

#[test]
fn test_model_autocomplete_completes_unique_match() {
    let mut app = create_test_app();
    configure_test_remote_models(&mut app);
    app.input = "/model g52c".to_string();
    app.cursor_pos = app.input.len();

    assert!(app.autocomplete());
    assert_eq!(app.input(), "/model gpt-5.2-codex");
}

#[test]
fn test_model_autocomplete_completes_unique_provider_match() {
    with_temp_jcode_home(|| {
        save_test_saitec_session();
        let mut app = create_test_app();
        configure_test_remote_openrouter_provider_routes(&mut app);

        app.input = "/model anthropic/claude-sonnet-4@fi".to_string();
        app.cursor_pos = app.input.len();

        assert!(app.autocomplete());
        assert_eq!(app.input(), "/model anthropic/claude-sonnet-4@Fireworks");
    });
}

#[test]
fn test_model_picker_preview_stays_open_and_updates_filter() {
    let mut app = create_test_app();
    configure_test_remote_models(&mut app);

    for c in "/model g52c".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("model picker preview should be open");
    assert!(picker.preview);
    assert_eq!(picker.filter, "g52c");
    assert!(
        picker
            .filtered
            .iter()
            .any(|&i| picker.entries[i].name == "gpt-5.2-codex")
    );
    assert_eq!(app.input(), "/model g52c");
}

#[test]
fn test_model_picker_preview_enter_selects_model() {
    let mut app = create_test_app();
    configure_test_remote_models(&mut app);

    for c in "/model g52c".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .unwrap();

    // Enter from preview mode selects the model and closes the picker
    assert!(app.inline_interactive_state.is_none());
    assert!(app.input().is_empty());
    assert_eq!(app.cursor_pos(), 0);
}
