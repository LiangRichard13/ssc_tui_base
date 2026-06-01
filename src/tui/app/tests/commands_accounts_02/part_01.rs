#[test]
fn test_usage_card_renders_when_loading() {
    let mut app = create_test_app();
    app.open_usage_inline_loading();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("usage card draw should succeed");

    let text = buffer_to_text(&terminal);
    assert!(
        text.contains("╭"),
        "usage card should render as rounded box, got:\n{text}"
    );
    assert!(
        text.contains("Refreshing usage"),
        "usage card should be visible while loading, got:\n{text}"
    );
    assert!(
        text.contains("Checking connected provider limits"),
        "usage card should include loading details, got:\n{text}"
    );
}

#[test]
fn test_usage_card_does_not_capture_typing() {
    let mut app = create_test_app();
    app.open_usage_inline_loading();
    assert!(app.usage_overlay.is_none());

    app.handle_key(KeyCode::Char('h'), KeyModifiers::empty())
        .expect("type after usage card");

    assert!(app.usage_overlay.is_none());
    assert_eq!(app.input(), "h");
}

#[test]
fn test_usage_report_updates_display_only_card_without_system_message() {
    let mut app = create_test_app();
    app.usage_report_refreshing = true;
    app.handle_usage_report(vec![crate::usage::ProviderUsage {
        provider_name: "OpenAI (ChatGPT)".to_string(),
        limits: vec![crate::usage::UsageLimit {
            name: "5h".to_string(),
            usage_percent: 82.0,
            resets_at: None,
        }],
        extra_info: vec![("plan".to_string(), "pro".to_string())],
        hard_limit_reached: false,
        error: None,
    }]);

    assert!(!app.usage_report_refreshing);
    assert!(app.inline_view_state.is_none());
    assert!(app.usage_overlay.is_none());
    let msg = app.display_messages().last().expect("missing usage card");
    assert_eq!(msg.role, "usage");
    assert!(msg.content.contains("OpenAI (ChatGPT)"));
    assert!(msg.content.contains("5h"));
    assert!(msg.content.contains("82%"));
    assert!(msg.content.contains("plan: pro"));
    assert!(app.materialized_provider_messages().is_empty());
}

#[test]
fn test_usage_progress_updates_card_incrementally() {
    let mut app = create_test_app();
    app.open_usage_inline_loading();

    app.handle_usage_report_progress(crate::usage::ProviderUsageProgress {
        results: vec![crate::usage::ProviderUsage {
            provider_name: "Anthropic (Claude)".to_string(),
            limits: vec![crate::usage::UsageLimit {
                name: "5-hour window".to_string(),
                usage_percent: 41.0,
                resets_at: None,
            }],
            extra_info: Vec::new(),
            hard_limit_reached: false,
            error: None,
        }],
        completed: 1,
        total: 2,
        done: false,
        from_cache: false,
    });

    assert!(app.usage_report_refreshing);
    assert_eq!(
        app.display_messages()
            .iter()
            .filter(|message| message.role == "usage")
            .count(),
        1
    );
    let detail = &app
        .display_messages()
        .last()
        .expect("missing usage card")
        .content;
    assert!(detail.contains("5-hour window") || detail.contains("Refreshing usage (1/2)"));
}

#[test]
fn test_usage_with_suffix_does_not_open_picker_preview() {
    let mut app = create_test_app();

    for c in "/usage open".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }

    assert!(app.inline_interactive_state.is_none());
    assert_eq!(app.input(), "/usage open");
}

#[test]
fn test_show_accounts_includes_masked_email_column() {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let accounts = vec![crate::auth::claude::AnthropicAccount {
        label: "work".to_string(),
        access: "acc".to_string(),
        refresh: "ref".to_string(),
        expires: now_ms + 60000,
        email: Some("user@example.com".to_string()),
        scopes: Vec::new(),
        subscription_type: Some("max".to_string()),
    }];

    let mut lines = vec!["**Anthropic Accounts:**\n".to_string()];
    lines.push("| Account | Email | Status | Subscription | Active |".to_string());
    lines.push("|---------|-------|--------|-------------|--------|".to_string());

    for account in &accounts {
        let status = if account.expires > now_ms {
            "✓ valid"
        } else {
            "⚠ expired"
        };
        let email = account
            .email
            .as_deref()
            .map(mask_email)
            .unwrap_or_else(|| "unknown".to_string());
        let sub = account.subscription_type.as_deref().unwrap_or("unknown");
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            account.label, email, status, sub, "◉"
        ));
    }

    let output = lines.join("\n");
    assert!(output.contains("| Account | Email | Status | Subscription | Active |"));
    assert!(output.contains("u***r@example.com"));
}

#[test]
fn test_account_openai_command_opens_account_picker() {
    with_temp_jcode_home(|| {
        let now_ms = chrono::Utc::now().timestamp_millis();

        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "work".to_string(),
            access_token: "acc".to_string(),
            refresh_token: "ref".to_string(),
            id_token: None,
            account_id: Some("acct_work".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("user@example.com".to_string()),
        })
        .unwrap();

        let mut app = create_test_app();
        app.input = "/account openai".to_string();
        app.submit_input();

        assert!(app.account_picker_overlay.is_none());
        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("/account openai should open the inline account picker");
        assert_eq!(picker.kind, crate::tui::PickerKind::Account);
        assert!(picker.entries.iter().any(|entry| {
            matches!(
                entry.action,
                crate::tui::PickerAction::Account(crate::tui::AccountPickerAction::Switch {
                    ref provider_id,
                    ..
                }) if provider_id == "openai"
            )
        }));
        assert!(
            picker
                .entries
                .iter()
                .any(|entry| entry.name == "account center")
        );
    });
}

#[test]
fn test_account_command_opens_account_picker() {
    with_temp_jcode_home(|| {
        let now_ms = chrono::Utc::now().timestamp_millis();

        crate::auth::claude::upsert_account(crate::auth::claude::AnthropicAccount {
            label: "claude-1".to_string(),
            access: "claude_acc".to_string(),
            refresh: "claude_ref".to_string(),
            expires: now_ms + 60_000,
            email: Some("claude@example.com".to_string()),
            scopes: Vec::new(),
            subscription_type: Some("pro".to_string()),
        })
        .unwrap();

        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "work".to_string(),
            access_token: "acc".to_string(),
            refresh_token: "ref".to_string(),
            id_token: None,
            account_id: Some("acct_work".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("user@example.com".to_string()),
        })
        .unwrap();

        let mut app = create_test_app();
        app.input = "/account".to_string();
        app.submit_input();

        assert!(app.account_picker_overlay.is_none());
        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("/account should open the inline account picker");
        assert!(picker.entries.iter().any(|entry| {
            matches!(
                entry.action,
                crate::tui::PickerAction::Account(crate::tui::AccountPickerAction::Switch {
                    ref provider_id,
                    ref label
                }) if provider_id == "claude" && label == "claude-1"
            )
        }));
        assert!(picker.entries.iter().any(|entry| {
            matches!(
                entry.action,
                crate::tui::PickerAction::Account(crate::tui::AccountPickerAction::Switch {
                    ref provider_id,
                    ..
                }) if provider_id == "openai"
            )
        }));
        assert!(
            picker
                .entries
                .iter()
                .any(|entry| entry.name == "account center")
        );
    });
}

#[test]
fn test_account_picker_supports_arrow_and_vim_navigation() {
    with_temp_jcode_home(|| {
        let now_ms = chrono::Utc::now().timestamp_millis();

        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "first".to_string(),
            access_token: "acc1".to_string(),
            refresh_token: "ref1".to_string(),
            id_token: None,
            account_id: Some("acct_1".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("first@example.com".to_string()),
        })
        .unwrap();
        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "second".to_string(),
            access_token: "acc2".to_string(),
            refresh_token: "ref2".to_string(),
            id_token: None,
            account_id: Some("acct_2".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("second@example.com".to_string()),
        })
        .unwrap();

        let mut app = create_test_app();
        app.input = "/account openai".to_string();
        app.submit_input();

        let initial_selected = app
            .inline_interactive_state
            .as_ref()
            .expect("inline account picker should open")
            .selected;

        app.handle_key(KeyCode::Down, KeyModifiers::empty())
            .unwrap();
        let after_arrow = app.inline_interactive_state.as_ref().unwrap().selected;
        assert_eq!(after_arrow, initial_selected + 1);

        app.handle_key(KeyCode::Char('j'), KeyModifiers::empty())
            .unwrap();
        let after_vim = app.inline_interactive_state.as_ref().unwrap().selected;
        assert_eq!(after_vim, after_arrow + 1);

        app.handle_key(KeyCode::Char('k'), KeyModifiers::empty())
            .unwrap();
        assert_eq!(
            app.inline_interactive_state.as_ref().unwrap().selected,
            after_arrow
        );
    });
}

#[test]
fn test_account_picker_preview_from_input_filters_accounts() {
    with_temp_jcode_home(|| {
        let now_ms = chrono::Utc::now().timestamp_millis();

        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "first".to_string(),
            access_token: "acc1".to_string(),
            refresh_token: "ref1".to_string(),
            id_token: None,
            account_id: Some("acct_1".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("first@example.com".to_string()),
        })
        .unwrap();
        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "second".to_string(),
            access_token: "acc2".to_string(),
            refresh_token: "ref2".to_string(),
            id_token: None,
            account_id: Some("acct_2".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("second@example.com".to_string()),
        })
        .unwrap();

        let mut app = create_test_app();
        for c in "/account openai sec".chars() {
            app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
                .unwrap();
        }

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("account preview should open");
        assert!(picker.preview, "account picker should stay in preview mode");
        assert_eq!(picker.kind, crate::tui::PickerKind::Account);
        assert_eq!(picker.filter, "sec");
        assert!(app.account_picker_overlay.is_none());
        assert_eq!(app.input(), "/account openai sec");
    });
}

#[test]
fn test_account_picker_preview_stays_closed_for_explicit_subcommands() {
    let mut app = create_test_app();

    for c in "/account openai settings".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }

    assert!(app.inline_interactive_state.is_none());
    assert_eq!(app.input(), "/account openai settings");
}

#[test]
fn test_account_command_combines_claude_and_openai_accounts() {
    with_temp_jcode_home(|| {
        let now_ms = chrono::Utc::now().timestamp_millis();

        crate::auth::claude::upsert_account(crate::auth::claude::AnthropicAccount {
            label: "claude-1".to_string(),
            access: "claude_acc".to_string(),
            refresh: "claude_ref".to_string(),
            expires: now_ms + 60_000,
            email: Some("claude@example.com".to_string()),
            scopes: Vec::new(),
            subscription_type: Some("pro".to_string()),
        })
        .unwrap();
        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "openai-1".to_string(),
            access_token: "acc".to_string(),
            refresh_token: "ref".to_string(),
            id_token: None,
            account_id: Some("acct_openai_1".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("openai@example.com".to_string()),
        })
        .unwrap();

        let mut app = create_test_app();
        app.input = "/account".to_string();
        app.submit_input();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("inline account picker should open");
        assert!(picker.entries.iter().any(|entry| {
            matches!(
                entry.action,
                crate::tui::PickerAction::Account(crate::tui::AccountPickerAction::Switch {
                    ref provider_id,
                    ref label
                }) if provider_id == "claude" && label == "claude-1"
            )
        }));
        assert!(picker.entries.iter().any(|entry| {
            matches!(
                entry.action,
                crate::tui::PickerAction::Account(crate::tui::AccountPickerAction::Switch {
                    ref provider_id,
                    ref label
                }) if provider_id == "openai" && label == "openai-1"
            )
        }));
        assert!(
            picker
                .entries
                .iter()
                .any(|entry| entry.name == "account center")
        );
    });
}

#[cfg(unix)]
#[test]
fn test_account_command_uses_fast_auth_snapshot_without_running_cursor_status() {
    use std::os::unix::fs::PermissionsExt;

    with_temp_jcode_home(|| {
        let prev_cursor_cli_path = std::env::var_os("JCODE_CURSOR_CLI_PATH");
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let marker = temp.path().join("cursor-status-ran");
        let script = temp.path().join("cursor-agent-mock");

        std::fs::write(
            &script,
            format!("#!/bin/sh\necho ran > \"{}\"\nexit 0\n", marker.display()),
        )
        .expect("write mock cursor agent");
        let mut permissions = std::fs::metadata(&script)
            .expect("stat mock cursor agent")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod mock cursor agent");

        let mut app = create_test_app();

        crate::env::set_var("JCODE_CURSOR_CLI_PATH", &script);
        crate::auth::AuthStatus::invalidate_cache();
        let _ = std::fs::remove_file(&marker);

        app.input = "/account".to_string();
        app.submit_input();

        assert!(app.inline_interactive_state.is_some());
        assert!(
            !marker.exists(),
            "/account should not execute `cursor-agent status` on open"
        );

        match prev_cursor_cli_path {
            Some(value) => crate::env::set_var("JCODE_CURSOR_CLI_PATH", value),
            None => crate::env::remove_var("JCODE_CURSOR_CLI_PATH"),
        }
        crate::auth::AuthStatus::invalidate_cache();
    });
}

#[test]
fn test_account_switch_shorthand_switches_openai_account_by_label() {
    with_temp_jcode_home(|| {
        let now_ms = chrono::Utc::now().timestamp_millis();

        crate::auth::codex::upsert_account(crate::auth::codex::OpenAiAccount {
            label: "openai2".to_string(),
            access_token: "acc".to_string(),
            refresh_token: "ref".to_string(),
            id_token: None,
            account_id: Some("acct_openai2".to_string()),
            expires_at: Some(now_ms + 60_000),
            email: Some("user2@example.com".to_string()),
        })
        .unwrap();

        let mut app = create_test_app();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            app.input = "/account switch openai2".to_string();
            app.submit_input();

            assert_eq!(
                crate::auth::codex::active_account_label().as_deref(),
                Some("openai-1")
            );
        });
    });
}

#[test]
fn test_account_picker_prompt_new_openai_label_cancel_clears_prompt() {
    let mut app = create_test_app();
    app.prompt_new_account_label(crate::tui::account_picker::AccountProviderKind::OpenAi);

    assert!(matches!(
        app.pending_account_input,
        Some(super::auth::PendingAccountInput::NewAccountLabel { ref provider_id, .. }) if provider_id == "openai"
    ));

    app.input = "/cancel".to_string();
    app.submit_input();

    assert!(app.pending_account_input.is_none());
    assert!(app.pending_login.is_none());
}

#[test]
fn test_login_command_opens_login_mode_selector_overlay() {
    let mut app = create_test_app();
    app.input = "/login".to_string();
    app.submit_input();

    assert!(
        app.account_picker_overlay.is_some(),
        "/login should open the top-level login mode selector"
    );
    assert!(
        app.pending_login.is_none(),
        "/login should no longer jump directly into the Saitec form"
    );
}

#[test]
fn test_login_mode_selector_uses_simple_two_option_dialog() {
    let mut app = create_test_app();
    app.input = "/login".to_string();
    app.submit_input();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login selector draw should succeed");
    let text = buffer_to_text(&terminal);

    assert!(text.contains("SAITEC"), "rendered selector:\n{text}");
    assert!(text.contains("Base models"), "rendered selector:\n{text}");
    assert!(
        text.contains("sign in to SAITEC and unlock the TUI"),
        "rendered selector:\n{text}"
    );
    assert!(
        text.contains("open the filtered base-model login picker"),
        "rendered selector:\n{text}"
    );
    assert!(!text.contains("Overview"), "rendered selector:\n{text}");
    assert!(
        !text.contains("Providers & Quick Actions"),
        "rendered selector:\n{text}"
    );
    assert!(
        !text.contains("saved accounts stay surfaced here"),
        "rendered selector:\n{text}"
    );
}

#[test]
fn test_enter_on_login_preview_submits_login_command_and_opens_selector() {
    let mut app = create_test_app();
    app.input = "/login".to_string();
    app.sync_model_picker_preview_from_input();
    assert!(
        app.inline_interactive_state.is_none(),
        "exact /login should not open the inline login preview"
    );

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should submit login command");

    assert!(app.inline_interactive_state.is_none());

    assert!(
        app.account_picker_overlay.is_some() || app.pending_login.is_some(),
        "enter on exact /login should submit the command"
    );

    if app.pending_login.is_none() {
        assert!(app.account_picker_overlay.is_some());
        assert_eq!(app.input(), "");
        assert_eq!(app.cursor_pos, 0);
    }
}

#[test]
fn test_exact_login_typing_keeps_inline_login_preview_closed() {
    let mut app = create_test_app();

    for c in "/login".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .expect("type /login");
    }

    assert!(
        app.inline_interactive_state.is_none(),
        "typing exact /login should not show the ITEM/PROVIDER/ACTION preview"
    );
    assert_eq!(app.input(), "/login");
}

#[test]
fn test_login_mode_selector_enter_defaults_to_saitec_form() {
    let mut app = create_test_app();
    app.open_login_mode_selector();

    assert!(app.account_picker_overlay.is_some());

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should activate the default login mode");

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state after Enter on selector: {other:?}"),
    }
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_clears_stale_saitec_form_before_entering_saitec_branch() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();
    app.open_login_mode_selector();

    assert!(
        app.pending_login.is_none(),
        "opening the top-level login selector should dismiss the stale pending login form"
    );
    assert!(app.account_picker_overlay.is_some());

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should activate the default login mode");

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!(
            "unexpected pending login state after Enter on selector with stale form: {other:?}"
        ),
    }
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_up_after_down_returns_to_saitec_without_closing_selector() {
    let mut app = create_test_app();
    app.open_login_mode_selector();

    assert!(app.account_picker_overlay.is_some());

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should move the selector to Base models");
    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("up should move back to SAITEC inside the selector");

    assert!(
        app.account_picker_overlay.is_some(),
        "up navigation should keep the login mode selector open"
    );
    assert!(app.pending_login.is_none());
    assert_eq!(app.input(), "");
    assert_eq!(app.cursor_pos, 0);

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should activate SAITEC after navigating back up");

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state after selector up/down navigation: {other:?}"),
    }
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_mouse_click_opens_saitec_form() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    app.open_login_mode_selector();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login selector draw should succeed");

    let click_row = 15;
    let click_col = 32;
    let handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: click_col,
        row: click_row,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!handled, "clicks should request an immediate redraw");
    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state after mouse click on SAITEC: {other:?}"),
    }
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_mouse_click_opens_base_models_picker() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    app.open_login_mode_selector();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login selector draw should succeed");

    let handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 32,
        row: 18,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!handled, "clicks should request an immediate redraw");
    assert!(
        app.login_picker_overlay.is_some(),
        "clicking Base models should open the filtered login picker"
    );
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_click_does_not_immediately_activate_base_model_provider_on_mouse_up() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    app.open_login_mode_selector();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login selector draw should succeed");

    let down_handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 32,
        row: 18,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!down_handled, "clicks should request an immediate redraw");
    assert!(
        app.login_picker_overlay.is_some(),
        "mouse down should open the filtered base-model login picker"
    );
    assert!(
        app.pending_login.is_none(),
        "opening the base-model picker should not already start a provider login flow"
    );

    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("base-model picker draw should succeed");

    let up_handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 32,
        row: 18,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!up_handled, "mouse up should request an immediate redraw");
    assert!(
        app.login_picker_overlay.is_some(),
        "the mouse-up event from the original click should not immediately close the new picker"
    );
    assert!(
        app.pending_login.is_none(),
        "the original click should not leak into the provider picker and start a login flow"
    );
    assert!(
        !app.should_quit,
        "opening the second-level login picker should never trigger app exit"
    );
}

#[test]
fn test_login_mode_selector_mouse_up_opens_saitec_form() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    app.open_login_mode_selector();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login selector draw should succeed");

    let handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 32,
        row: 15,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!handled, "clicks should request an immediate redraw");
    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state after mouse up on SAITEC: {other:?}"),
    }
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_mouse_click_on_blank_separator_still_opens_saitec_form() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    app.open_login_mode_selector();

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login selector draw should succeed");

    let handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 32,
        row: 17,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!handled, "clicks should request an immediate redraw");
    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!(
            "unexpected pending login state after blank-separator click on SAITEC: {other:?}"
        ),
    }
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_login_mode_selector_routes_base_models_to_filtered_login_picker() {
    let mut app = create_test_app();
    app.open_login_mode_selector();

    assert!(app.account_picker_overlay.is_some());

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should move login mode selection");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should activate the selected login mode");

    assert!(
        app.login_picker_overlay.is_some(),
        "base-model branch should open the login picker overlay"
    );
    assert!(
        app.account_picker_overlay.is_none(),
        "base-model branch should not reopen the account picker overlay"
    );
}

#[test]
fn test_login_mode_selector_clears_stale_saitec_form_before_entering_base_models_branch() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();
    app.open_login_mode_selector();

    assert!(app.pending_login.is_none());
    assert!(app.account_picker_overlay.is_some());

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should move login mode selection");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should activate the selected login mode");

    assert!(
        app.login_picker_overlay.is_some(),
        "base-model branch should open the login picker overlay even if a stale saitec form existed"
    );
    assert!(app.pending_login.is_none());
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_filtered_login_picker_contains_only_saitec_allowlisted_providers() {
    let mut app = create_test_app();
    app.input = "/login base-models".to_string();
    app.submit_input();

    let picker_cell = app
        .login_picker_overlay
        .as_ref()
        .expect("login picker overlay should open");
    let picker = picker_cell.borrow();
    let profile = picker.debug_memory_profile();

    assert_eq!(profile["items_count"], 5);
    assert_eq!(profile["filtered_count"], 5);
    drop(picker);

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("filtered login picker draw should succeed");
    let text = buffer_to_text(&terminal);

    assert!(text.contains("OpenAI"), "rendered picker:\n{text}");
    assert!(text.contains("Claude"), "rendered picker:\n{text}");
    assert!(text.contains("Z.AI"), "rendered picker:\n{text}");
    assert!(text.contains("Kimi"), "rendered picker:\n{text}");
    assert!(text.contains("Alibaba"), "rendered picker:\n{text}");
    assert!(!text.contains("Google"), "rendered picker:\n{text}");
    assert!(!text.contains("Bedrock"), "rendered picker:\n{text}");
    assert!(!text.contains("Azure"), "rendered picker:\n{text}");
}

#[test]
fn test_filtered_login_picker_uses_validation_results_for_provider_status_text() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let previous_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    crate::provider_catalog::save_env_value_to_env_file("ZHIPU_API_KEY", "zai.env", Some("zai-test-key"))
        .expect("save Z.AI key");
    crate::auth::validation::save(
        "zai",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: false,
            provider_smoke_ok: Some(false),
            tool_smoke_ok: Some(false),
            validated_models: Vec::new(),
            summary: "provider_smoke: unauthorized".to_string(),
        },
    )
    .expect("save failed validation");
    crate::auth::validation::save(
        "kimi",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: Some(true),
            validated_models: Vec::new(),
            summary: "tool_smoke: AUTH_TEST_OK".to_string(),
        },
    )
    .expect("save passing validation");
    crate::provider_catalog::save_env_value_to_env_file("KIMI_API_KEY", "kimi.env", Some("kimi-test-key"))
        .expect("save Kimi key");

    let mut app = create_test_app();
    app.input = "/login base-models".to_string();
    app.submit_input();
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("move selection to OpenAI");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("move selection to Z.AI");

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("filtered login picker draw should succeed");
    let text = buffer_to_text(&terminal);

    assert!(
        text.contains("needs attention"),
        "providers with failed runtime validation should show attention status, got:\n{text}"
    );
    assert!(
        text.contains("validation failed"),
        "failed provider detail should surface the recorded validation failure, got:\n{text}"
    );

    if let Some(previous_home) = previous_home {
        crate::env::set_var("JCODE_HOME", previous_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}

#[test]
fn test_filtered_login_picker_mouse_click_starts_selected_provider_login() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/login base-models".to_string();
        app.submit_input();
    });

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("filtered login picker draw should succeed");

    let picker_cell = app
        .login_picker_overlay
        .as_ref()
        .expect("login picker overlay should be open");
    let picker = picker_cell.borrow();
    let list_area = picker
        .debug_provider_list_area_for_tests()
        .expect("render should record provider list area");
    drop(picker);

    let handled = rt.block_on(async {
        app.handle_mouse_event(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: list_area.x + 1,
            row: list_area.y + 1,
            modifiers: KeyModifiers::empty(),
        })
    });

    assert!(!handled, "clicks should request an immediate redraw");
    assert!(
        app.login_picker_overlay.is_none(),
        "clicking a provider should close the picker and start its login flow"
    );
    assert!(
        app.pending_login.is_some()
            || app
                .display_messages()
                .iter()
                .any(|msg| msg.content.contains("OpenAI")),
        "clicking the first provider should start the OpenAI login flow"
    );
}

#[test]
fn test_filtered_login_picker_mouse_up_after_click_keeps_zai_api_key_login_open() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/login base-models".to_string();
        app.submit_input();
    });

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("filtered login picker draw should succeed");

    let picker_cell = app
        .login_picker_overlay
        .as_ref()
        .expect("login picker overlay should be open");
    let picker = picker_cell.borrow();
    let list_area = picker
        .debug_provider_list_area_for_tests()
        .expect("render should record provider list area");
    drop(picker);

    let zai_row = list_area.y + 2;
    let click_col = list_area.x + 1;

    let handled_down = rt.block_on(async {
        app.handle_mouse_event(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: click_col,
            row: zai_row,
            modifiers: KeyModifiers::empty(),
        })
    });

    assert!(!handled_down, "clicks should request an immediate redraw");
    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::ApiKeyProfile {
            provider,
            provider_id,
            ..
        }) => {
            assert_eq!(provider, "Z.AI");
            assert_eq!(provider_id, "zai");
        }
        other => panic!("expected Z.AI API-key login after mouse down, got: {other:?}"),
    }
    assert!(app.login_picker_overlay.is_none());

    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let handled_up = rt.block_on(async {
        app.handle_mouse_event(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: click_col,
            row: zai_row,
            modifiers: KeyModifiers::empty(),
        })
    });

    assert!(!handled_up, "mouse up should request an immediate redraw");
    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::ApiKeyProfile {
            provider,
            provider_id,
            ..
        }) => {
            assert_eq!(provider, "Z.AI");
            assert_eq!(provider_id, "zai");
        }
        other => panic!("Z.AI API-key login should remain open after mouse up, got: {other:?}"),
    }
    assert!(
        !app.should_quit,
        "clicking into Z.AI login should not trigger app exit"
    );
}

#[test]
fn test_login_base_models_command_opens_filtered_login_picker() {
    let mut app = create_test_app();
    app.input = "/login base-models".to_string();
    app.submit_input();

    assert!(
        app.login_picker_overlay.is_some(),
        "/login base-models should open the filtered login picker"
    );
    assert!(
        app.account_picker_overlay.is_none(),
        "/login base-models should not open the account center"
    );
}

#[test]
fn test_login_openai_starts_allowed_base_model_login() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/login openai".to_string();
        app.submit_input();
    });

    assert!(app.account_picker_overlay.is_none());
    assert!(app.login_picker_overlay.is_none());
    assert!(
        app.pending_login.is_some() || app.display_messages().iter().any(|msg| {
            msg.content.contains("OpenAI")
        }),
        "/login openai should start the allowlisted provider flow"
    );
}

#[test]
fn test_login_openrouter_is_rejected_by_saitec_allowlist() {
    let mut app = create_test_app();
    app.input = "/login openrouter".to_string();
    app.submit_input();

    assert!(app.pending_login.is_none());
    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("SAITEC-TUI only supports these base-model providers"));
}

#[test]
fn test_set_pending_saitec_login_for_tests_uses_form_variant() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.form.email, "");
            assert_eq!(form.form.phone, "");
            assert_eq!(form.form.password, "");
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}

#[test]
fn test_logout_command_clears_saitec_auth_file() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os(crate::subscription_catalog::JCODE_API_KEY_ENV);
    crate::env::set_var("JCODE_HOME", temp.path());

    crate::saitec::auth::save_session(&crate::saitec::auth::SaitecSession {
        auth_token: None,
        api_key: "sk-live-test".to_string(),
        token_type: "Bearer".to_string(),
        user_id: Some("mock-user".to_string()),
        email: None,
        phone: None,
        display_name: None,
        api_key_id: None,
        api_key_name: None,
        api_key_created_at: None,
        api_key_expires_at: None,
        last_validated_at: None,
    })
    .expect("save auth");

    let mut app = create_test_app();
    app.input = "/logout".to_string();
    app.submit_input();

    assert!(crate::saitec::auth::load_session().expect("load").is_none());
    assert_eq!(crate::subscription_catalog::configured_api_key(), None);
    assert!(
        app.display_messages()
            .iter()
            .any(|msg| msg.role == "system"
                && msg.content.contains("Logged out from Saitec")),
        "logout confirmation should be present in system messages"
    );
    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
            assert_eq!(form.form.email, "");
            assert_eq!(form.form.phone, "");
            assert_eq!(form.form.password, "");
        }
        ref other => panic!("logout should reopen the saitec login form: {other:?}"),
    }

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    if let Some(prev_api_key) = prev_api_key {
        crate::env::set_var(crate::subscription_catalog::JCODE_API_KEY_ENV, prev_api_key);
    } else {
        crate::env::remove_var(crate::subscription_catalog::JCODE_API_KEY_ENV);
    }
}

#[test]
fn test_pending_saitec_form_empty_submit_sets_validation_error() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();

    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            let error = form.error.as_deref().expect("validation error");
            assert!(error.contains("password"), "unexpected error: {error}");
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Submit
            );
            assert!(!form.submitting);
        }
        ref other => panic!("login form should stay pending on validation failure: {other:?}"),
    }
}

#[test]
fn test_start_jcode_login_uses_saitec_pending_state() {
    let mut app = create_test_app();
    app.input = "/login jcode".to_string();
    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.form.email, "");
            assert_eq!(form.form.phone, "");
            assert_eq!(form.form.password, "");
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}

#[test]
fn test_account_jcode_login_uses_saitec_pending_state() {
    let mut app = create_test_app();
    app.input = "/account jcode login".to_string();
    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.form.email, "");
            assert_eq!(form.form.phone, "");
            assert_eq!(form.form.password, "");
            assert_eq!(
                form.focus,
                crate::tui::app::auth::SaitecLoginField::Email
            );
        }
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}

#[test]
fn test_account_openai_login_starts_allowed_base_model_login() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/account openai login".to_string();
        app.submit_input();
    });

    assert!(app.pending_login.is_some() || app.display_messages().iter().any(|msg| {
        msg.content.contains("OpenAI")
    }));
}

#[test]
fn test_account_zai_login_starts_allowed_base_model_login() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/account zai login".to_string();
        app.submit_input();
    });

    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::ApiKeyProfile {
            provider,
            provider_id,
            ..
        }) => {
            assert_eq!(provider, "Z.AI");
            assert_eq!(provider_id, "zai");
        }
        other => panic!("expected Z.AI login flow, got: {other:?}"),
    }
}

#[test]
fn test_account_kimi_login_starts_allowed_base_model_login() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/account kimi login".to_string();
        app.submit_input();
    });

    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::ApiKeyProfile {
            provider,
            provider_id,
            ..
        }) => {
            assert_eq!(provider, "Kimi Code");
            assert_eq!(provider_id, "kimi");
        }
        other => panic!("expected Kimi login flow, got: {other:?}"),
    }
}

#[test]
fn test_account_alibaba_login_starts_allowed_base_model_login() {
    let mut app = create_test_app();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        app.input = "/account alibaba-coding-plan login".to_string();
        app.submit_input();
    });

    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::ApiKeyProfile {
            provider,
            provider_id,
            ..
        }) => {
            assert_eq!(provider, "Alibaba Cloud Coding Plan");
            assert_eq!(provider_id, "alibaba-coding-plan");
        }
        other => panic!("expected Alibaba login flow, got: {other:?}"),
    }
}

#[test]
fn test_api_key_login_overlay_keeps_zai_login_visible_and_masks_live_input() {
    let mut app = create_test_app();
    let live_key = "secret-api-key";
    app.input = live_key.to_string();
    app.cursor_pos = live_key.len();
    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");

    let backend = ratatui::backend::TestBackend::new(100, 28);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let buf = terminal.backend().buffer();
    let width = buf.area.width as usize;
    let height = buf.area.height as usize;
    let mut rendered_lines = Vec::with_capacity(height);
    for y in 0..height {
        let mut line = String::with_capacity(width);
        for x in 0..width {
            line.push_str(buf[(x as u16, y as u16)].symbol());
        }
        rendered_lines.push(line);
    }
    let rendered = rendered_lines.join("\n");

    assert!(
        rendered.contains("Z.AI API Key"),
        "api-key login should stay visibly open instead of dropping back to chat: {rendered}"
    );
    assert!(
        !rendered.contains(live_key),
        "api-key login should not leak the live key into the normal composer: {rendered}"
    );
    assert!(
        rendered.contains("**************"),
        "api-key login overlay should mask the live key while typing: {rendered}"
    );
}

#[test]
fn test_api_key_login_overlay_shows_cancel_button_instead_of_cancel_command() {
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");

    let backend = ratatui::backend::TestBackend::new(100, 28);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let text = buffer_to_text(&terminal);
    assert!(
        text.contains("[ Clear ]"),
        "api-key overlay should render a visible clear button, got:\n{text}"
    );
    assert!(
        text.contains("[ Validate ]"),
        "api-key overlay should render a visible validate button, got:\n{text}"
    );
    assert!(
        text.contains("[ Cancel ]"),
        "api-key overlay should render a visible cancel button, got:\n{text}"
    );
    assert!(
        !text.contains("/cancel"),
        "api-key overlay should not instruct users to type /cancel anymore, got:\n{text}"
    );
}

#[test]
fn test_api_key_login_overlay_places_validate_and_cancel_on_one_row() {
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("kimi", "Kimi Code", "KIMI_API_KEY");

    let backend = ratatui::backend::TestBackend::new(100, 28);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let text = buffer_to_text(&terminal);
    assert!(
        text.lines().any(|line| {
            let clear = line.find("[ Clear ]");
            let validate = line.find("[ Validate ]");
            let cancel = line.find("[ Cancel ]");
            matches!((clear, validate, cancel), (Some(c), Some(v), Some(x)) if c < v && v < x)
        }),
        "api-key overlay should place clear, validate, and cancel on one row in order, got:\n{text}"
    );
}

#[test]
fn test_api_key_login_overlay_masks_long_input_without_wrapping() {
    let mut app = create_test_app();
    let live_key = "x".repeat(160);
    app.input = live_key;
    app.cursor_pos = app.input.len();
    app.set_pending_api_key_login_for_tests("kimi", "Kimi Code", "KIMI_API_KEY");

    let backend = ratatui::backend::TestBackend::new(60, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let star_lines = buffer_to_text(&terminal)
        .lines()
        .filter(|line| line.matches('*').count() >= 10)
        .count();
    assert_eq!(
        star_lines, 1,
        "masked API-key input should stay on one visual line"
    );
}

#[test]
fn test_api_key_login_overlay_mouse_click_validate_submits() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");
    app.input = "secret-api-key".to_string();
    app.cursor_pos = app.input.len();

    let backend = ratatui::backend::TestBackend::new(100, 28);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let buf = terminal.backend().buffer();
    let mut validate_hit = None;
    'rows: for y in 0..buf.area.height {
        let mut line = String::with_capacity(buf.area.width as usize);
        for x in 0..buf.area.width {
            line.push_str(buf[(x, y)].symbol());
        }
        if let Some(col) = line.find("[ Validate ]") {
            validate_hit = Some((col as u16 + 2, y));
            break 'rows;
        }
    }
    let (column, row) = validate_hit.expect("validate button should be visible");

    let handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!handled, "overlay click should request an immediate redraw");
    assert!(
        app.pending_login.is_none(),
        "clicking validate should submit the API-key overlay"
    );

    let saved = std::fs::read_to_string(
        crate::storage::app_config_dir()
            .expect("config dir")
            .join("test.env"),
    )
    .expect("saved env file");
    assert!(
        saved.contains("ZAI_API_KEY=secret-api-key"),
        "validate click should save the API key, got:\n{saved}"
    );

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}

#[test]
fn test_api_key_login_overlay_mouse_up_validate_submits() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("kimi", "Kimi Code", "KIMI_API_KEY");
    app.input = "kimi-secret-key".to_string();
    app.cursor_pos = app.input.len();

    let backend = ratatui::backend::TestBackend::new(100, 28);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("api-key login draw should succeed");

    let buf = terminal.backend().buffer();
    let mut validate_hit = None;
    'rows: for y in 0..buf.area.height {
        let mut line = String::with_capacity(buf.area.width as usize);
        for x in 0..buf.area.width {
            line.push_str(buf[(x, y)].symbol());
        }
        if let Some(col) = line.find("[ Validate ]") {
            validate_hit = Some((col as u16 + 2, y));
            break 'rows;
        }
    }
    let (column, row) = validate_hit.expect("validate button should be visible");

    let handled = app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    });

    assert!(!handled, "overlay click should request an immediate redraw");
    assert!(
        app.pending_login.is_none(),
        "mouse-up validate should submit the API-key overlay"
    );

    let saved = std::fs::read_to_string(
        crate::storage::app_config_dir()
            .expect("config dir")
            .join("test.env"),
    )
    .expect("saved env file");
    assert!(
        saved.contains("KIMI_API_KEY=kimi-secret-key"),
        "validate mouse-up should save the API key, got:\n{saved}"
    );

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}

#[test]
fn test_account_openrouter_add_is_blocked_by_saitec_allowlist() {
    let mut app = create_test_app();
    app.input = "/account openrouter add".to_string();
    app.submit_input();

    assert!(app.pending_login.is_none());
    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("SAITEC-TUI only supports these base-model providers"));
}

#[test]
fn test_account_openrouter_settings_is_rejected_by_saitec_allowlist() {
    let mut app = create_test_app();
    app.input = "/account openrouter settings".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("SAITEC-TUI only supports these base-model providers"));
}

#[test]
fn test_auth_status_lists_only_saitec_allowlisted_base_model_providers() {
    let mut app = create_test_app();
    app.input = "/auth".to_string();
    app.submit_input();

    let msg = app.display_messages().last().expect("missing auth status");
    assert!(msg.content.contains("Saitec Subscription"));
    assert!(msg.content.contains("OpenAI"));
    assert!(msg.content.contains("Anthropic/Claude"));
    assert!(msg.content.contains("Z.AI"));
    assert!(msg.content.contains("Kimi"));
    assert!(msg.content.contains("Alibaba Cloud Coding"));
    assert!(!msg.content.contains("GitHub Copilot"));
    assert!(!msg.content.contains("Google"));
}

#[test]
fn test_account_default_provider_command_saves_config() {
    let _guard = crate::storage::lock_test_env();
    let mut app = create_test_app();
    app.input = "/account default-provider openai".to_string();
    app.submit_input();

    let cfg = crate::config::Config::load();
    assert_eq!(cfg.provider.default_provider.as_deref(), Some("openai"));
}

#[test]
fn test_commands_alias_shows_help() {
    let mut app = create_test_app();
    app.input = "/commands".to_string();
    app.submit_input();

    assert!(
        app.help_scroll.is_some(),
        "/commands should open help overlay"
    );
}

#[test]
fn test_help_overlay_hides_git_and_skills_in_saitec_product_mode() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let jcode_home = temp.path().join("jcode-home");
    std::fs::create_dir_all(&jcode_home).expect("create jcode home");
    std::fs::create_dir_all(temp.path().join(".jcode").join("skills").join("alpha"))
        .expect("create local skill dir");
    std::fs::write(
        temp.path()
            .join(".jcode")
            .join("skills")
            .join("alpha")
            .join("SKILL.md"),
        "---\nname: alpha\ndescription: Test skill\n---\n",
    )
    .expect("write local test skill");

    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_cwd = std::env::current_dir().expect("current dir");
    crate::env::set_var("JCODE_HOME", &jcode_home);
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let mut app = create_test_app();
    app.input = "/help".to_string();
    app.submit_input();
    assert!(app.help_scroll.is_some(), "help overlay should be open");

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("help overlay draw should succeed");
    let text = buffer_to_text(&terminal);

    std::env::set_current_dir(prev_cwd).expect("restore current dir");
    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }

    assert!(!text.contains("/git"), "rendered help:\n{text}");
    assert!(!text.contains("Skills"), "rendered help:\n{text}");
    assert!(!text.contains("/alpha"), "rendered help:\n{text}");
}

#[test]
fn test_help_overlay_keeps_login_logout_and_model_commands() {
    let mut app = create_test_app();
    app.input = "/help".to_string();
    app.submit_input();
    assert!(app.help_scroll.is_some(), "help overlay should be open");

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("help overlay draw should succeed");
    let text = buffer_to_text(&terminal);

    assert!(text.contains("/login"), "rendered help:\n{text}");
    assert!(text.contains("/logout"), "rendered help:\n{text}");
    assert!(text.contains("/model"), "rendered help:\n{text}");
    assert!(text.contains("/quit"), "rendered help:\n{text}");
}

#[test]
fn test_login_command_after_prior_message_renders_login_selector_without_provider_item_action_table()
{
    let mut app = create_test_app();
    app.push_display_message(DisplayMessage::user("hello before login"));
    app.input = "/login".to_string();
    app.submit_input();

    assert!(app.is_login_mode_selector_open());

    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &app))
        .expect("login overlay draw should succeed");
    let text = buffer_to_text(&terminal);

    assert!(text.contains("SAITEC"), "rendered:\n{text}");
    assert!(text.contains("Base models"), "rendered:\n{text}");
    assert!(!text.contains("PROVIDER"), "rendered:\n{text}");
    assert!(!text.contains("ITEM"), "rendered:\n{text}");
    assert!(!text.contains("ACTION"), "rendered:\n{text}");
}

#[test]
fn test_improve_command_starts_improvement_loop() {
    let mut app = create_test_app();
    app.input = "/improve".to_string();
    app.submit_input();

    assert_eq!(app.improve_mode, Some(ImproveMode::ImproveRun));
    assert_eq!(
        app.session.improve_mode,
        Some(crate::session::SessionImproveMode::ImproveRun)
    );
    assert!(app.is_processing());

    let msg = app.session.messages.last().expect("missing improve prompt");
    assert!(matches!(
        &msg.content[0],
        ContentBlock::Text { text, .. }
            if text.contains("You are entering improvement mode for this repository")
                && text.contains("write a concise ranked todo list using `todo`")
    ));

    let display = app
        .display_messages()
        .last()
        .expect("missing improve launch notice");
    assert!(display.content.contains("Starting improvement loop"));
}

#[test]
fn test_improve_plan_command_is_plan_only_and_accepts_focus() {
    let mut app = create_test_app();
    app.input = "/improve plan startup performance".to_string();
    app.submit_input();

    assert_eq!(app.improve_mode, Some(ImproveMode::ImprovePlan));
    assert_eq!(
        app.session.improve_mode,
        Some(crate::session::SessionImproveMode::ImprovePlan)
    );
    assert!(app.is_processing());

    let msg = app
        .session
        .messages
        .last()
        .expect("missing improve plan prompt");
    assert!(matches!(
        &msg.content[0],
        ContentBlock::Text { text, .. }
            if text.contains("improvement planning mode")
                && text.contains("This is plan-only mode")
                && text.contains("Focus area: startup performance")
    ));
}

#[test]
fn test_improve_status_summarizes_current_todos() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[
                crate::todo::TodoItem {
                    id: "one".to_string(),
                    content: "Profile startup path".to_string(),
                    status: "in_progress".to_string(),
                    priority: "high".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                },
                crate::todo::TodoItem {
                    id: "two".to_string(),
                    content: "Add regression test".to_string(),
                    status: "completed".to_string(),
                    priority: "medium".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                },
            ],
        )
        .expect("save todos");

        app.improve_mode = Some(ImproveMode::ImproveRun);
        app.input = "/improve status".to_string();
        app.submit_input();

        let msg = app
            .display_messages()
            .last()
            .expect("missing improve status");
        assert!(msg.content.contains("Improve status"));
        assert!(
            msg.content
                .contains("1 incomplete · 1 completed · 0 cancelled")
        );
        assert!(msg.content.contains("Profile startup path"));
    });
}

#[test]
fn test_improve_stop_without_active_run_reports_idle() {
    let mut app = create_test_app();
    app.session.improve_mode = None;
    app.input = "/improve stop".to_string();
    app.submit_input();

    let msg = app
        .display_messages()
        .last()
        .expect("missing improve stop idle message");
    assert!(msg.content.contains("No active improve loop to stop"));
}

#[test]
fn test_improve_stop_queues_stop_prompt_and_clears_mode() {
    let mut app = create_test_app();
    app.improve_mode = Some(ImproveMode::ImproveRun);
    app.session.improve_mode = Some(crate::session::SessionImproveMode::ImproveRun);
    app.input = "/improve stop".to_string();
    app.submit_input();

    assert_eq!(app.improve_mode, None);
    assert_eq!(app.session.improve_mode, None);
    assert!(app.is_processing());

    let msg = app
        .session
        .messages
        .last()
        .expect("missing improve stop prompt");
    assert!(matches!(
        &msg.content[0],
        ContentBlock::Text { text, .. }
            if text.contains("Stop improvement mode after the current safe point")
    ));
}

#[test]
fn test_improve_resume_requires_saved_mode() {
    let mut app = create_test_app();
    app.input = "/improve resume".to_string();
    app.submit_input();

    let msg = app
        .display_messages()
        .last()
        .expect("missing improve resume idle message");
    assert!(msg.content.contains("No saved improve run found"));
}

#[test]
fn test_improve_resume_uses_saved_mode_and_current_todos() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.session.improve_mode = Some(crate::session::SessionImproveMode::ImproveRun);
        app.session.save().expect("save session");
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "resume1".to_string(),
                content: "Refactor command parsing".to_string(),
                status: "in_progress".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        app.input = "/improve resume".to_string();
        app.submit_input();

        assert_eq!(app.improve_mode, Some(ImproveMode::ImproveRun));
        assert_eq!(
            app.session.improve_mode,
            Some(crate::session::SessionImproveMode::ImproveRun)
        );
        assert!(app.is_processing());

        let msg = app
            .session
            .messages
            .last()
            .expect("missing improve resume prompt");
        assert!(matches!(
            &msg.content[0],
            ContentBlock::Text { text, .. }
                if text.contains("Resume improvement mode")
                    && text.contains("Refactor command parsing")
        ));
    });
}
