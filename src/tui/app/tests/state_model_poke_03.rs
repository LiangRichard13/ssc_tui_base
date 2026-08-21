#[test]
fn test_model_picker_preview_arrow_keys_navigate() {
    with_temp_jcode_home(|| {
        save_test_openai_remote_model_validation();
        let mut app = create_test_app();
        configure_test_remote_models(&mut app);

        // Type a filtered /model command to open preview.
        for c in "/model g".chars() {
            app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
                .unwrap();
        }

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker preview should be open");
        assert!(picker.preview);
        let initial_selected = picker.selected;

        // Down arrow should navigate in preview mode
        app.handle_key(KeyCode::Down, KeyModifiers::empty())
            .unwrap();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("picker should still be open");
        assert!(picker.preview, "should remain in preview mode");
        assert_eq!(picker.selected, initial_selected + 1);

        // Up arrow should navigate back
        app.handle_key(KeyCode::Up, KeyModifiers::empty()).unwrap();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("picker should still be open");
        assert!(picker.preview, "should remain in preview mode");
        assert_eq!(picker.selected, initial_selected);

        // Input should be preserved
        assert_eq!(app.input(), "/model g");
    });
}

#[test]
fn test_open_model_picker_without_routes_shows_actionable_guidance() {
    let mut app = create_test_app();

    app.open_model_picker();
    wait_for_model_picker_load(&mut app);

    assert!(app.inline_interactive_state.is_none());
    assert_eq!(app.status_notice(), Some("No models available".to_string()));

    let last = app.display_messages.last().expect("display message");
    assert_eq!(last.role, "system");
    assert!(last.content.contains("/login"));
    assert!(last.content.contains("/account"));
    assert!(last.content.contains("/model"));
}

#[derive(Clone)]
struct CountingModelRoutesProvider {
    calls: StdArc<AtomicUsize>,
    route_count: usize,
    delay: Duration,
}

#[derive(Clone)]
struct UnfilteredSubscriptionModelProvider {
    name: &'static str,
}

#[async_trait::async_trait]
impl Provider for UnfilteredSubscriptionModelProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        unimplemented!("UnfilteredSubscriptionModelProvider")
    }

    fn name(&self) -> &str {
        self.name
    }

    fn model(&self) -> String {
        "gpt-5.4".to_string()
    }

    fn available_models_display(&self) -> Vec<String> {
        vec![
            "gpt-5.4".to_string(),
            "deepseek/deepseek-v4-pro".to_string(),
            "claude-opus-4-7".to_string(),
            "moonshotai/kimi-k2.5".to_string(),
        ]
    }

    fn model_routes(&self) -> Vec<crate::provider::ModelRoute> {
        self.available_models_display()
            .into_iter()
            .map(|model| crate::provider::ModelRoute {
                model,
                provider: "Test".to_string(),
                api_method: "test".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            })
            .collect()
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

#[async_trait::async_trait]
impl Provider for CountingModelRoutesProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        unimplemented!("CountingModelRoutesProvider")
    }

    fn name(&self) -> &str {
        "counting"
    }

    fn model(&self) -> String {
        "counting-a".to_string()
    }

    fn model_routes(&self) -> Vec<crate::provider::ModelRoute> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        (0..self.route_count)
            .map(|idx| crate::provider::ModelRoute {
                model: format!("counting-{}", (b'a' + idx as u8) as char),
                provider: "Counting".to_string(),
                api_method: "test".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            })
            .collect()
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

#[test]
fn test_subscription_model_provider_suggestions_hide_openrouter_pins() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::apply_runtime_env();
        let (app, _set_model_calls) = create_openrouter_spec_capture_test_app();

        let suggestions = app.get_suggestions_for("/model DeepSeek V4 Pro@");

        assert!(
            suggestions.is_empty(),
            "subscription /model suggestions exposed OpenRouter pins: {suggestions:?}"
        );

        crate::subscription_catalog::clear_runtime_env();
    });
}

#[test]
fn test_subscription_remote_model_picker_hides_openrouter_provider_options() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::apply_runtime_env();

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_provider_name = Some("openrouter".to_string());
        app.remote_provider_model = Some("deepseek/deepseek-v4-pro".to_string());
        app.remote_model_options = vec![
            crate::provider::ModelRoute {
                model: "deepseek/deepseek-v4-pro".to_string(),
                provider: "auto".to_string(),
                api_method: "openrouter".to_string(),
                available: true,
                detail: "OpenRouter internal route".to_string(),
                cheapness: None,
            },
            crate::provider::ModelRoute {
                model: "anthropic/claude-sonnet-4".to_string(),
                provider: "OpenRouter".to_string(),
                api_method: "openrouter".to_string(),
                available: true,
                detail: "non-curated OpenRouter route".to_string(),
                cheapness: None,
            },
        ];

        app.open_model_picker();
        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let route_text: Vec<String> = picker
            .entries
            .iter()
            .flat_map(|entry| {
                entry.options.iter().map(move |option| {
                    format!(
                        "{}|{}|{}|{}",
                        entry.name, option.provider, option.api_method, option.detail
                    )
                })
            })
            .collect();

        assert!(
            route_text.iter().all(|route| {
                let lower = route.to_ascii_lowercase();
                !lower.contains("openrouter") && !lower.contains("|auto|")
            }),
            "subscription Provider column leaked OpenRouter routes: {route_text:?}"
        );
        assert!(
            route_text
                .iter()
                .all(|route| route.contains("Ssc Subscription|saitec")),
            "subscription Provider column should only expose Saitec routes: {route_text:?}"
        );

        crate::subscription_catalog::clear_runtime_env();
    });
}

#[test]
fn test_remote_model_picker_hides_openrouter_routed_provider_options_without_subscription_runtime()
{
    with_temp_jcode_home(|| {
        crate::subscription_catalog::clear_runtime_env();
        save_test_provider_validation("openai", &["gpt-5.5"]);

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_provider_name = Some("openai".to_string());
        app.remote_provider_model = Some("gpt-5.5".to_string());
        app.remote_model_options = vec![
            crate::provider::ModelRoute {
                model: "gpt-5.5".to_string(),
                provider: "OpenAI".to_string(),
                api_method: "openrouter".to_string(),
                available: true,
                detail: "OpenRouter routed endpoint".to_string(),
                cheapness: None,
            },
            crate::provider::ModelRoute {
                model: "gpt-5.5".to_string(),
                provider: "OpenAI".to_string(),
                api_method: "openai-oauth".to_string(),
                available: true,
                detail: String::new(),
                cheapness: None,
            },
            crate::provider::ModelRoute {
                model: "claude-sonnet-4-6".to_string(),
                provider: "Anthropic".to_string(),
                api_method: "openrouter".to_string(),
                available: true,
                detail: "OpenRouter fallback provider".to_string(),
                cheapness: None,
            },
        ];

        app.open_model_picker();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let route_text: Vec<String> = picker
            .entries
            .iter()
            .flat_map(|entry| {
                entry.options.iter().map(move |option| {
                    format!(
                        "{}|{}|{}|{}",
                        entry.name, option.provider, option.api_method, option.detail
                    )
                })
            })
            .collect();

        assert!(
            route_text.iter().all(|route| {
                let lower = route.to_ascii_lowercase();
                !lower.contains("openrouter")
            }),
            "non-subscription /model Provider column leaked OpenRouter-routed options: {route_text:?}"
        );
    });
}

#[test]
fn test_model_picker_reuses_cached_entries_until_invalidated() {
    ensure_test_jcode_home_if_unset();
    clear_persisted_test_ui_state();
    crate::tui::ui::clear_test_render_state_for_tests();

    let calls = StdArc::new(AtomicUsize::new(0));
    let provider: Arc<dyn Provider> = Arc::new(CountingModelRoutesProvider {
        calls: StdArc::clone(&calls),
        route_count: 2,
        delay: Duration::ZERO,
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;

    app.open_model_picker();
    wait_for_model_picker_load(&mut app);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(app.model_picker_cache.is_some());

    app.open_model_picker();
    wait_for_model_picker_load(&mut app);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "second open should reuse cached picker entries"
    );

    app.invalidate_model_picker_cache();
    app.open_model_picker();
    wait_for_model_picker_load(&mut app);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "invalidating should force rebuilding provider routes"
    );
}

#[test]
fn test_model_picker_opens_loading_state_before_async_routes_complete() {
    ensure_test_jcode_home_if_unset();
    clear_persisted_test_ui_state();
    crate::tui::ui::clear_test_render_state_for_tests();

    let calls = StdArc::new(AtomicUsize::new(0));
    let provider: Arc<dyn Provider> = Arc::new(CountingModelRoutesProvider {
        calls: StdArc::clone(&calls),
        route_count: 2,
        delay: Duration::from_millis(75),
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;

    app.open_model_picker();

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("loading picker should open immediately");
    assert_eq!(picker.entries.len(), 1);
    assert_eq!(picker.entries[0].name, "counting-a");
    assert!(
        picker.entries[0].options[0]
            .detail
            .contains("updating model list")
    );
    assert!(app.pending_model_picker_load.is_some());
    assert_eq!(
        app.status_notice(),
        Some("Updating model list…".to_string())
    );

    wait_for_model_picker_load(&mut app);
    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("hydrated picker should still be open");
    assert!(picker.entries.len() >= 2);
    assert_eq!(app.status_notice(), Some("Model list updated".to_string()));
}

#[test]
fn test_model_picker_does_not_cache_single_model_fallback() {
    ensure_test_jcode_home_if_unset();
    clear_persisted_test_ui_state();
    crate::tui::ui::clear_test_render_state_for_tests();

    let calls = StdArc::new(AtomicUsize::new(0));
    let provider: Arc<dyn Provider> = Arc::new(CountingModelRoutesProvider {
        calls: StdArc::clone(&calls),
        route_count: 1,
        delay: Duration::ZERO,
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;

    app.open_model_picker();
    wait_for_model_picker_load(&mut app);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        app.model_picker_cache.is_none(),
        "single-model fallback results should not be retained"
    );

    app.open_model_picker();
    wait_for_model_picker_load(&mut app);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "single-model fallback should be rebuilt so a later full catalog can surface"
    );
}

#[test]
fn test_local_model_picker_selection_failure_keeps_picker_open_and_shows_next_steps() {
    with_temp_jcode_home(|| {
        save_test_provider_validation("copilot", &["claude-opus-4.6"]);
        let mut app = create_failing_model_switch_test_app();

        app.open_model_picker();
        wait_for_model_picker_load(&mut app);
        assert!(app.inline_interactive_state.is_some());

        app.handle_key(KeyCode::Enter, KeyModifiers::empty())
            .expect("enter should be handled");

        assert!(
            app.inline_interactive_state.is_some(),
            "picker should remain open so the user can choose another model"
        );
        assert_eq!(app.status_notice(), Some("Model switch failed".to_string()));

        let last = app.display_messages.last().expect("display message");
        assert_eq!(last.role, "error");
        assert!(last.content.contains("credentials expired"));
        assert!(last.content.contains("/model"));
        assert!(last.content.contains("/login"));
        assert!(last.content.contains("/account"));
    });
}

#[test]
fn test_login_completed_spawns_auth_refresh_when_runtime_is_available() {
    ensure_test_jcode_home_if_unset();
    clear_persisted_test_ui_state();
    crate::tui::ui::clear_test_render_state_for_tests();

    let started = StdArc::new(AtomicBool::new(false));
    let completed = StdArc::new(AtomicBool::new(false));
    let provider: Arc<dyn Provider> = Arc::new(AsyncAuthRefreshingMockProvider {
        started: StdArc::clone(&started),
        completed: StdArc::clone(&completed),
        delay: Duration::from_millis(150),
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;

    let _guard = rt.enter();
    let start = Instant::now();
    app.handle_login_completed(crate::bus::LoginCompleted {
        provider: "openrouter".to_string(),
        success: true,
        message: "OpenRouter ready".to_string(),
    });
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(100),
        "login completion should not block on auth refresh, took {:?}",
        elapsed
    );

    let wait_start = Instant::now();
    while !started.load(Ordering::SeqCst) || !completed.load(Ordering::SeqCst) {
        assert!(
            wait_start.elapsed() < Duration::from_secs(2),
            "background auth refresh did not complete"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn test_login_completed_surfaces_new_provider_models_in_local_model_picker() {
    with_temp_jcode_home(|| {
        let mut app = create_auth_refresh_test_app();

        app.handle_login_completed(crate::bus::LoginCompleted {
            provider: "copilot".to_string(),
            success: true,
            message: "Authenticated as **octocat** via GitHub Copilot.\n\nCopilot models are now available in `/model`."
                .to_string(),
        });
        save_test_provider_validation("copilot", &["claude-opus-4.6", "grok-code-fast-1"]);

        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");

        let copilot_entry = picker
            .entries
            .iter()
            .find(|entry| entry.name == "claude-opus-4.6")
            .expect("copilot model should be shown after login");

        assert!(
            picker
                .entries
                .iter()
                .any(|entry| entry.name == "grok-code-fast-1"),
            "all newly available Copilot models should appear in /model"
        );
        assert!(copilot_entry.options.iter().any(|route| {
            route.provider == "Copilot"
                && route.api_method == "copilot"
                && route.available
                && !route.detail.contains("runtime not validated")
        }));

        assert!(
            picker.entries[0].options.iter().any(|route| {
                route.provider == "Copilot"
                    && route.available
                    && route.detail.contains("recently added")
            }),
            "recently authenticated provider should be prioritized and marked in /model"
        );
    });
}

#[test]
fn test_local_model_picker_surfaces_antigravity_models_from_multiprovider() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::clear_runtime_env();
        save_test_provider_validation("antigravity", &["claude-sonnet-4-6", "gpt-oss-120b-medium"]);
        let mut app = create_antigravity_picker_test_app();
        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");

        let antigravity_entry = picker
            .entries
            .iter()
            .find(|entry| entry.name == "claude-sonnet-4-6")
            .expect("antigravity model should be shown after validation");

        assert!(antigravity_entry.options.iter().any(|route| {
            route.provider == "Antigravity" && route.api_method == "https" && route.available
        }));
    });
}

#[test]
fn test_local_antigravity_model_picker_selection_preserves_antigravity_provider() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::clear_runtime_env();
        save_test_provider_validation("antigravity", &["claude-sonnet-4-6", "gpt-oss-120b-medium"]);
        let mut app = create_antigravity_picker_test_app();
        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");

        let model_idx = picker
            .entries
            .iter()
            .position(|entry| entry.name == "claude-sonnet-4-6")
            .expect("antigravity model should be in picker");
        let filtered_pos = picker
            .filtered
            .iter()
            .position(|&i| i == model_idx)
            .expect("antigravity model should be in filtered list");

        app.inline_interactive_state.as_mut().unwrap().selected = filtered_pos;
        app.handle_key(KeyCode::Enter, KeyModifiers::empty())
            .unwrap();

        assert_eq!(app.provider.name(), "Antigravity");
        assert_eq!(app.provider.model(), "claude-sonnet-4-6");
        assert!(app.inline_interactive_state.is_none());
    });
}

#[test]
fn test_local_model_picker_hides_openrouter_bare_openai_route() {
    let (mut app, set_model_calls) = create_openrouter_spec_capture_test_app();
    app.open_model_picker();
    wait_for_model_picker_load(&mut app);

    assert!(
        app.inline_interactive_state.is_none(),
        "OpenRouter-backed OpenAI route should not open the SAITEC model picker"
    );
    assert_eq!(app.status_notice(), Some("No models available".to_string()));
    assert!(
        set_model_calls.lock().unwrap().is_empty(),
        "hidden OpenRouter route should not be selectable"
    );
}

#[test]
fn test_local_model_picker_openrouter_direct_kimi_route_uses_profile_prefix() {
    with_temp_jcode_home(|| {
        save_test_provider_validation("kimi", &["kimi-for-coding"]);
        let (mut app, set_model_calls) = create_openrouter_spec_capture_test_app_with_routes(vec![
            crate::provider::ModelRoute {
                model: "kimi-for-coding".to_string(),
                provider: "Kimi Code".to_string(),
                api_method: "openai-compatible:kimi".to_string(),
                available: true,
                detail: "https://api.kimi.com/coding/v1".to_string(),
                cheapness: None,
            },
        ]);
        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let model_idx = picker
            .entries
            .iter()
            .position(|entry| entry.name == "kimi-for-coding")
            .expect("direct Kimi route should be in picker");
        let filtered_pos = picker
            .filtered
            .iter()
            .position(|&i| i == model_idx)
            .expect("entry should be in filtered list");

        app.inline_interactive_state.as_mut().unwrap().selected = filtered_pos;
        app.handle_key(KeyCode::Enter, KeyModifiers::empty())
            .expect("model picker selection should succeed");

        assert_eq!(
            set_model_calls.lock().unwrap().as_slice(),
            ["kimi:kimi-for-coding"]
        );
    });
}

#[test]
fn test_remote_model_picker_dedupes_generic_and_profile_kimi_routes() {
    with_temp_jcode_home(|| {
        let prev_kimi_key = std::env::var_os("KIMI_API_KEY");
        crate::env::set_var("KIMI_API_KEY", "test-kimi-key");
        save_test_provider_validation("kimi", &["kimi-for-coding"]);

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_provider_name = Some("Kimi Code".to_string());
        app.remote_provider_model = Some("kimi-for-coding".to_string());
        app.remote_model_options = vec![crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "openai-compatible".to_string(),
            available: true,
            detail: "custom endpoint".to_string(),
            cheapness: None,
        }];

        app.open_model_picker();

        match prev_kimi_key {
            Some(value) => crate::env::set_var("KIMI_API_KEY", value),
            None => crate::env::remove_var("KIMI_API_KEY"),
        }
        crate::auth::AuthStatus::invalidate_cache();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let kimi_entries = picker
            .entries
            .iter()
            .filter(|entry| entry.name == "kimi-for-coding")
            .collect::<Vec<_>>();
        assert_eq!(
            kimi_entries.len(),
            1,
            "Kimi picker entries should be de-duplicated: {:?}",
            picker
                .entries
                .iter()
                .map(|entry| {
                    let route = entry.active_option();
                    (
                        entry.name.as_str(),
                        route.map(|route| route.provider.as_str()).unwrap_or(""),
                        route.map(|route| route.api_method.as_str()).unwrap_or(""),
                        route.map(|route| route.detail.as_str()).unwrap_or(""),
                    )
                })
                .collect::<Vec<_>>()
        );
        let route = kimi_entries[0]
            .active_option()
            .expect("Kimi entry should have a route");
        assert_eq!(route.provider, "Kimi Code");
        assert_eq!(route.api_method, "openai-compatible:kimi");
    });
}

#[test]
fn test_remote_model_picker_dedupes_current_and_profile_kimi_routes() {
    with_temp_jcode_home(|| {
        let prev_kimi_key = std::env::var_os("KIMI_API_KEY");
        crate::env::set_var("KIMI_API_KEY", "test-kimi-key");
        save_test_provider_validation("kimi", &["kimi-for-coding"]);

        let mut routes = vec![crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "current".to_string(),
            available: true,
            detail: "active remote model".to_string(),
            cheapness: None,
        }];

        App::append_configured_openai_compatible_routes_for_remote_picker(&mut routes);

        match prev_kimi_key {
            Some(value) => crate::env::set_var("KIMI_API_KEY", value),
            None => crate::env::remove_var("KIMI_API_KEY"),
        }
        crate::auth::AuthStatus::invalidate_cache();

        let kimi_routes = routes
            .iter()
            .filter(|route| route.model == "kimi-for-coding")
            .collect::<Vec<_>>();
        assert_eq!(
            kimi_routes.len(),
            1,
            "Kimi current/API-key routes should be de-duplicated: {:?}",
            routes
                .iter()
                .map(|route| (
                    route.model.as_str(),
                    route.provider.as_str(),
                    route.api_method.as_str(),
                    route.detail.as_str(),
                ))
                .collect::<Vec<_>>()
        );
        let route = kimi_routes[0];
        assert_eq!(route.provider, "Kimi Code");
        assert_eq!(route.api_method, "openai-compatible:kimi");
    });
}

#[test]
fn test_remote_model_picker_shows_validated_current_kimi_model_name() {
    with_temp_jcode_home(|| {
        save_test_provider_validation("kimi", &["kimi-for-coding"]);

        let mut app = create_test_app();
        app.is_remote = true;
        app.remote_provider_name = Some("Kimi Code".to_string());
        app.remote_provider_model = Some("kimi-for-coding".to_string());
        app.remote_model_options = vec![crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "current".to_string(),
            available: true,
            detail: "active remote model".to_string(),
            cheapness: None,
        }];

        app.open_model_picker();

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("/model should show validated current remote models");
        let names: Vec<&str> = picker
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert!(
            names.contains(&"kimi-for-coding"),
            "/model should list validated model names, got: {names:?}"
        );
    });
}


#[test]
fn test_subscription_model_picker_hides_generic_openrouter_routes() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::clear_runtime_env();
        save_test_provider_validation("kimi", &["kimi-for-coding"]);
        let (mut app, _set_model_calls) = create_named_openrouter_spec_capture_test_app_with_routes(
            "OpenRouter",
            vec![
                crate::provider::ModelRoute {
                    model: "anthropic/claude-opus-4".to_string(),
                    provider: "auto".to_string(),
                    api_method: "openrouter".to_string(),
                    available: true,
                    detail: "generic OpenRouter route".to_string(),
                    cheapness: None,
                },
                crate::provider::ModelRoute {
                    model: "gpt-5.4".to_string(),
                    provider: "OpenAI".to_string(),
                    api_method: "openrouter".to_string(),
                    available: true,
                    detail: "OpenRouter endpoint route".to_string(),
                    cheapness: None,
                },
                crate::provider::ModelRoute {
                    model: "kimi-for-coding".to_string(),
                    provider: "Kimi Code".to_string(),
                    api_method: "openai-compatible:kimi".to_string(),
                    available: true,
                    detail: "validated Kimi route".to_string(),
                    cheapness: None,
                },
            ],
        );

        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let names: Vec<&str> = picker
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();

        assert!(names.contains(&"kimi-for-coding"));
        assert!(!names.contains(&"anthropic/claude-opus-4"));
        assert!(!names.contains(&"gpt-5.4"));
        assert!(
            picker
                .entries
                .iter()
                .flat_map(|entry| entry.options.iter())
                .all(|route| route.api_method != "openrouter"),
            "SSC model picker leaked OpenRouter routes: {:?}",
            picker.entries
        );
    });
}

#[test]
fn test_subscription_model_picker_hides_unconfigured_anthropic_routes_when_kimi_available() {
    with_temp_jcode_home(|| {
        let prev_sub_key = std::env::var_os(crate::subscription_catalog::JCODE_API_KEY_ENV);
        crate::env::set_var(
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            "sk-test-saitec",
        );
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
        let (mut app, _set_model_calls) = create_named_openrouter_spec_capture_test_app_with_routes(
            "OpenRouter",
            vec![
                crate::provider::ModelRoute {
                    model: "claude-sonnet-4-6".to_string(),
                    provider: "Anthropic".to_string(),
                    api_method: "api-key".to_string(),
                    available: false,
                    detail: "no credentials".to_string(),
                    cheapness: None,
                },
                crate::provider::ModelRoute {
                    model: "kimi-for-coding".to_string(),
                    provider: "Kimi Code".to_string(),
                    api_method: "openai-compatible:kimi".to_string(),
                    available: true,
                    detail: "validated Kimi route".to_string(),
                    cheapness: None,
                },
            ],
        );

        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        match prev_sub_key {
            Some(value) => {
                crate::env::set_var(crate::subscription_catalog::JCODE_API_KEY_ENV, value)
            }
            None => crate::env::remove_var(crate::subscription_catalog::JCODE_API_KEY_ENV),
        }

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let names: Vec<&str> = picker
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();

        assert!(names.contains(&"kimi-for-coding"));
        assert!(
            !names.contains(&"claude-sonnet-4-6"),
            "unconfigured Anthropic route should not appear beside Kimi routes: {:?}",
            picker.entries
        );
    });
}

#[test]
fn test_subscription_model_command_rejects_generic_openrouter_model() {
    let (mut app, set_model_calls) = create_named_openrouter_spec_capture_test_app_with_routes(
        "OpenRouter",
        vec![crate::provider::ModelRoute {
            model: "anthropic/claude-opus-4".to_string(),
            provider: "auto".to_string(),
            api_method: "openrouter".to_string(),
            available: true,
            detail: "generic OpenRouter route".to_string(),
            cheapness: None,
        }],
    );

    app.input = "/model anthropic/claude-opus-4".to_string();
    app.submit_input();

    assert!(
        set_model_calls.lock().unwrap().is_empty(),
        "unsupported OpenRouter model should not reach provider.set_model"
    );
    let last = app
        .display_messages()
        .last()
        .expect("missing error message");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("SSC-TUI"));
}

#[test]
fn test_agent_model_picker_hides_openrouter_bare_openai_route() {
    let (mut app, _set_model_calls) = create_openrouter_spec_capture_test_app();

    app.open_agent_model_picker(crate::tui::AgentModelTarget::Swarm);

    assert!(
        app.inline_interactive_state.is_none(),
        "OpenRouter-backed OpenAI route should not open the SAITEC agent model picker"
    );
}

#[test]
fn test_local_model_picker_render_shows_antigravity_models_exactly_as_user_sees_them() {
    with_temp_jcode_home(|| {
        crate::subscription_catalog::clear_runtime_env();
        save_test_provider_validation("antigravity", &["claude-sonnet-4-6", "gpt-oss-120b-medium"]);
        let mut app = create_antigravity_picker_test_app();
        let text = render_model_picker_text(&mut app, 90, 24);

        assert!(
            text.contains("MODEL") && text.contains("PROVIDER") && text.contains("METHOD"),
            "rendered /model view should include picker columns, got:
{}",
            text
        );
        assert!(
            text.contains("claude-sonnet-4-6"),
            "rendered /model view should show the Antigravity Claude row, got:
{}",
            text
        );
        assert!(
            text.contains("gpt-oss-120b-medium"),
            "rendered /model view should show the Antigravity GPT row, got:
{}",
            text
        );
        assert!(
            text.contains("Antigravity"),
            "rendered /model view should show the Antigravity provider column, got:
{}",
            text
        );
        assert!(
            text.contains("https"),
            "rendered /model view should show the route transport column, got:
{}",
            text
        );
    });
}

#[test]
fn test_login_smoke_model_picker_renders_unstacked_provider_rows() {
    with_temp_jcode_home(|| {
        save_test_provider_validation("openai", &["gpt-5.4"]);
        save_test_provider_validation("openai-api", &["gpt-5.4"]);
        save_test_provider_validation("copilot", &["claude-opus-4.6"]);
        save_test_provider_validation("comtegra", &["glm-51-nvfp4"]);

        let mut app = create_login_smoke_model_app();
        let text = render_model_picker_text(&mut app, 110, 32);

        assert!(
            text.contains("MODEL") && text.contains("PROVIDER") && text.contains("METHOD"),
            "rendered /model view should include user-visible picker columns, got:\n{}",
            text
        );
        assert!(
            text.contains("gpt-5.4")
                && text.contains("OpenAI")
                && text.contains("oauth")
                && text.contains("api key"),
            "validated OpenAI OAuth and API-key routes should be separately visible, got:\n{}",
            text
        );
        let glm_row = text
            .lines()
            .find(|line| line.contains("glm-51-nvfp4"))
            .unwrap_or("");
        assert!(
            glm_row.contains("Comtegra GPU Cloud")
                && glm_row.contains("api key")
                && !glm_row.contains("copilot"),
            "validated Comtegra GLM row should show its provider and API-key method, got row `{}` in:\n{}",
            glm_row,
            text
        );
        assert!(
            text.contains("claude-opus-4.6") && text.contains("Copilot"),
            "validated Copilot route should be visible, got:\n{}",
            text
        );
        assert!(
            !text.to_ascii_lowercase().contains("openrouter")
                && !text.contains("deepseek/deepseek-v4-pro")
                && !text.contains("moonshotai/kimi-k2.5")
                && !text.contains("openai/gpt-5.5")
                && !text.to_ascii_lowercase().contains("runtime not validated"),
            "rendered /model view should hide OpenRouter and unvalidated routes, got:\n{}",
            text
        );
        assert!(
            !text.contains("(2)"),
            "provider routes should not be hidden behind stacked option counts, got:\n{}",
            text
        );
    });
}

#[test]
fn test_model_picker_hides_unvalidated_routes_and_reports_no_models() {
    with_temp_jcode_home(|| {
        let mut app = create_login_smoke_model_app();

        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        assert!(
            app.inline_interactive_state.is_none(),
            "unvalidated provider routes should not open the model picker"
        );
        assert_eq!(app.status_notice(), Some("No models available".to_string()));
        let display_text = app
            .display_messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        assert!(
            display_text.contains("No models are available"),
            "empty validated route set should explain that no models are available, got: {}",
            display_text
        );
    });
}

#[test]
fn test_model_picker_shows_only_validated_runtime_routes() {
    with_temp_jcode_home(|| {
        save_test_provider_validation("openai", &["gpt-5.4"]);
        save_test_provider_validation("copilot", &["claude-opus-4.6"]);
        let mut app = create_login_smoke_model_app();

        app.open_model_picker();
        wait_for_model_picker_load(&mut app);

        let picker = app
            .inline_interactive_state
            .as_ref()
            .expect("model picker should be open");
        let route_text: Vec<String> = picker
            .entries
            .iter()
            .flat_map(|entry| {
                entry.options.iter().map(move |route| {
                    format!(
                        "{}|{}|{}|{}|{}",
                        entry.name, route.provider, route.api_method, route.available, route.detail
                    )
                })
            })
            .collect();

        assert!(
            route_text
                .iter()
                .any(|route| route == "gpt-5.4|OpenAI|openai-oauth|true|"),
            "validated OpenAI OAuth route should appear, got: {route_text:?}"
        );
        assert!(
            route_text
                .iter()
                .any(|route| route == "claude-opus-4.6|Copilot|copilot|true|"),
            "validated Copilot route should appear, got: {route_text:?}"
        );
        assert!(
            route_text.iter().all(|route| {
                !route.contains("|openai-api-key|")
                    && !route.contains("|openai-compatible:")
                    && !route.to_ascii_lowercase().contains("openrouter")
                    && !route.to_ascii_lowercase().contains("runtime not validated")
                    && route.contains("|true|")
            }),
            "model picker should only show runtime-validated routes, got: {route_text:?}"
        );
    });
}

#[test]
fn test_model_picker_filter_text_includes_provider_and_method() {
    let entry = crate::tui::PickerEntry {
        name: "glm-51-nvfp4".to_string(),
        options: vec![crate::tui::PickerOption {
            provider: "Comtegra GPU Cloud".to_string(),
            api_method: "openai-compatible:comtegra".to_string(),
            available: true,
            detail: "https://llm.comtegra.cloud/v1".to_string(),
            estimated_reference_cost_micros: None,
        }],
        action: crate::tui::PickerAction::Model,
        selected_option: 0,
        is_current: false,
        is_default: false,
        recommended: false,
        recommendation_rank: usize::MAX,
        old: false,
        created_date: None,
        effort: None,
    };

    let filter_text = crate::tui::PickerKind::Model.filter_text(&entry);
    assert!(filter_text.contains("glm-51-nvfp4"));
    assert!(filter_text.contains("Comtegra GPU Cloud"));
    assert!(filter_text.contains("openai-compatible:comtegra"));
}

#[test]
fn test_login_picker_preview_stays_open_and_updates_filter() {
    let mut app = create_test_app();

    for c in "/login jc".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }

    let picker = app
        .inline_interactive_state
        .as_ref()
        .expect("login picker preview should be open");
    assert!(picker.preview);
    assert_eq!(picker.kind, crate::tui::PickerKind::Login);
    assert_eq!(picker.filter, "jc");
    assert!(
        picker
            .filtered
            .iter()
            .any(|&i| picker.entries[i].name == "SSC")
    );
    assert_eq!(app.input(), "/login jc");
}

#[test]
fn test_login_command_opens_login_mode_selector() {
    let mut app = create_test_app();

    for c in "/login".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty())
            .unwrap();
    }
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .unwrap();

    assert!(
        app.is_login_mode_selector_open(),
        "/login should open the base-models login selector"
    );
    assert!(
        app.pending_login.is_none(),
        "/login should not jump directly into a concrete login flow"
    );
}


#[test]
fn test_subagent_model_command_sets_and_resets_session_preference() {
    let mut app = create_test_app();

    assert!(super::commands::handle_session_command(
        &mut app,
        "/subagent-model gpt-5.4"
    ));
    assert_eq!(app.session.subagent_model.as_deref(), Some("gpt-5.4"));

    assert!(super::commands::handle_session_command(
        &mut app,
        "/subagent-model inherit"
    ));
    assert_eq!(app.session.subagent_model, None);
}

#[test]
fn test_autoreview_command_toggles_session_preference() {
    let mut app = create_test_app();

    assert!(super::commands::handle_session_command(
        &mut app,
        "/autoreview on"
    ));
    assert_eq!(app.session.autoreview_enabled, Some(true));
    assert!(app.autoreview_enabled);

    assert!(super::commands::handle_session_command(
        &mut app,
        "/autoreview off"
    ));
    assert_eq!(app.session.autoreview_enabled, Some(false));
    assert!(!app.autoreview_enabled);
}

#[test]
fn test_autojudge_command_toggles_session_preference() {
    let mut app = create_test_app();

    assert!(super::commands::handle_session_command(
        &mut app,
        "/autojudge on"
    ));
    assert_eq!(app.session.autojudge_enabled, Some(true));
    assert!(app.autojudge_enabled);

    assert!(super::commands::handle_session_command(
        &mut app,
        "/autojudge off"
    ));
    assert_eq!(app.session.autojudge_enabled, Some(false));
    assert!(!app.autojudge_enabled);
}

#[test]
fn test_transcript_path_command_reports_current_session_file() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        let expected = crate::session::session_path(&app.session.id).expect("session path");

        assert!(super::commands::handle_session_command(
            &mut app,
            "/transcript path"
        ));

        assert!(app.display_messages().iter().any(|msg| {
            msg.content.contains("Transcript file:")
                && msg.content.contains(&expected.display().to_string())
        }));
    });
}

#[test]
fn test_poke_arms_auto_poke_until_todos_are_done() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "todo-1".to_string(),
                content: "Finish the remaining task".to_string(),
                status: "pending".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        assert!(super::commands::handle_session_command(&mut app, "/poke"));

        assert!(app.auto_poke_incomplete_todos);
        assert!(app.pending_turn);
        assert!(app.display_messages().iter().any(|msg| {
            msg.content.contains("Poking model: 1 incomplete todo")
                && msg.content.contains("/poke off")
        }));
    });
}

#[test]
fn test_poke_status_reports_current_state() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "todo-1".to_string(),
                content: "Finish the remaining task".to_string(),
                status: "pending".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        assert!(super::commands::handle_session_command(
            &mut app,
            "/poke status"
        ));
        assert!(app.display_messages().iter().any(|msg| {
            msg.content
                .contains("Auto-poke: **ON**. 1 incomplete todo.")
        }));

        app.auto_poke_incomplete_todos = true;
        app.is_processing = true;
        app.queued_messages
            .push(super::commands::build_poke_message(
                &super::commands::incomplete_poke_todos(&app),
            ));

        assert!(super::commands::handle_session_command(
            &mut app,
            "/poke status"
        ));
        assert!(app.display_messages().iter().any(|msg| {
            msg.content
                .contains("Auto-poke: **ON**. 1 incomplete todo.")
                && msg.content.contains("A follow-up poke is queued.")
                && msg.content.contains("A turn is currently running.")
        }));
    });
}

#[test]
fn test_poke_off_disarms_and_clears_queued_followup() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "todo-1".to_string(),
                content: "Keep going".to_string(),
                status: "pending".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        app.auto_poke_incomplete_todos = true;
        app.pending_queued_dispatch = true;
        app.queued_messages
            .push(super::commands::build_poke_message(
                &super::commands::incomplete_poke_todos(&app),
            ));

        assert!(super::commands::handle_session_command(
            &mut app,
            "/poke off"
        ));

        assert!(!app.auto_poke_incomplete_todos);
        assert!(!app.pending_queued_dispatch);
        assert!(app.queued_messages().is_empty());
        assert_eq!(app.status_notice(), Some("Poke: OFF".to_string()));
        assert!(app.display_messages().iter().any(|msg| {
            msg.content.contains("Auto-poke disabled.")
                && msg.content.contains("Cleared 1 queued poke follow-up")
        }));
    });
}

#[test]
fn test_poke_queues_when_turn_is_in_progress() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "todo-1".to_string(),
                content: "Finish the remaining task".to_string(),
                status: "pending".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        app.is_processing = true;

        assert!(super::commands::handle_session_command(&mut app, "/poke"));

        assert!(app.auto_poke_incomplete_todos);
        assert!(app.is_processing);
        assert!(!app.cancel_requested);
        assert!(!app.pending_turn);
        assert_eq!(
            app.status_notice(),
            Some("Poke queued after current turn".to_string())
        );
        assert!(app.queued_messages().is_empty());
        assert!(app.display_messages().iter().any(|msg| {
            msg.content
                .contains("/poke queued. Re-checking incomplete todos after this turn")
        }));

        crate::todo::save_todos(
            &app.session.id,
            &[
                crate::todo::TodoItem {
                    id: "todo-1".to_string(),
                    content: "Finish the remaining task".to_string(),
                    status: "pending".to_string(),
                    priority: "high".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                },
                crate::todo::TodoItem {
                    id: "todo-2".to_string(),
                    content: "Pick up the newly discovered task".to_string(),
                    status: "pending".to_string(),
                    priority: "medium".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                },
            ],
        )
        .expect("save updated todos");

        super::local::finish_turn(&mut app);

        assert!(app.pending_queued_dispatch);
        assert_eq!(app.queued_messages().len(), 1);
        assert!(app.queued_messages()[0].contains("You have 2 incomplete todos"));
        assert!(!app.queued_messages()[0].contains("Pick up the newly discovered task"));
        assert!(!app.queued_messages()[0].contains("/poke off"));
    });
}

#[test]
fn test_finish_turn_auto_pokes_again_when_todos_remain() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "todo-1".to_string(),
                content: "Keep going".to_string(),
                status: "in_progress".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        app.auto_poke_incomplete_todos = true;
        app.is_processing = true;
        super::local::finish_turn(&mut app);

        assert!(app.pending_queued_dispatch);
        assert_eq!(app.queued_messages().len(), 1);
        assert!(app.queued_messages()[0].contains("Continue working, or update the todo tool."));
    });
}

#[test]
fn test_finish_turn_auto_poke_preserves_visible_turn_started() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[crate::todo::TodoItem {
                id: "todo-1".to_string(),
                content: "Keep going".to_string(),
                status: "in_progress".to_string(),
                priority: "high".to_string(),
                blocked_by: Vec::new(),
                assigned_to: None,
            }],
        )
        .expect("save todos");

        let started = Instant::now() - Duration::from_secs(45);
        app.auto_poke_incomplete_todos = true;
        app.is_processing = true;
        app.visible_turn_started = Some(started);

        super::local::finish_turn(&mut app);

        assert_eq!(app.visible_turn_started, Some(started));
        assert!(app.pending_queued_dispatch);
    });
}

#[test]
fn test_help_topic_shows_overnight_command_details() {
    let mut app = create_test_app();
    app.input = "/help overnight".to_string();
    app.submit_input();

    let msg = app
        .display_messages()
        .last()
        .expect("missing help response");
    assert_eq!(msg.role, "system");
    assert!(msg.content.contains("`/overnight <hours>[h|m] [mission]`"));
    assert!(msg.content.contains("review HTML page"));
    assert!(msg.content.contains("`/overnight status`"));
}

#[test]
fn test_overnight_status_without_runs_is_handled() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        assert!(super::commands::handle_session_command(
            &mut app,
            "/overnight status"
        ));

        let msg = app
            .display_messages()
            .last()
            .expect("missing overnight status response");
        assert_eq!(msg.role, "system");
        assert!(msg.content.contains("No overnight runs found"));
    });
}

#[test]
fn test_overnight_help_command_is_handled() {
    let mut app = create_test_app();
    assert!(super::commands::handle_session_command(
        &mut app,
        "/overnight help"
    ));

    let msg = app
        .display_messages()
        .last()
        .expect("missing overnight help response");
    assert_eq!(msg.role, "system");
    assert!(msg.content.contains("`/overnight <hours>[h|m] [mission]`"));
    assert!(msg.content.contains("`/overnight review`"));
}

#[test]
fn test_overnight_start_runs_as_visible_local_turn() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        assert!(super::commands::handle_session_command(
            &mut app,
            "/overnight 1m hi"
        ));

        assert!(
            app.pending_turn,
            "local overnight should start a visible turn"
        );
        assert!(
            app.is_processing,
            "local overnight should enter processing state"
        );
        assert!(
            app.queued_messages.is_empty(),
            "local overnight should not use remote queue"
        );
        let last_message = app
            .session
            .messages
            .last()
            .expect("overnight prompt message");
        assert!(last_message.content.iter().any(|block| matches!(
            block,
            crate::message::ContentBlock::Text { text, .. }
                if text.contains("visible Overnight Coordinator")
        )));
    });
}

#[test]
fn test_overnight_start_queues_remote_turn_without_stuck_sending() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.is_remote = true;
        assert!(super::commands::handle_session_command(
            &mut app,
            "/overnight 1m hi"
        ));

        assert!(
            !app.pending_turn,
            "remote overnight should not set local pending_turn"
        );
        assert!(
            !app.is_processing,
            "remote overnight should not get stuck in local Sending"
        );
        assert_eq!(app.queued_messages.len(), 1);
        assert!(app.queued_messages[0].contains("visible Overnight Coordinator"));
    });
}
