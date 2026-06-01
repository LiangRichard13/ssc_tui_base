use super::*;

/// Update cost calculation based on token usage (for API-key providers)
impl App {
    pub(crate) fn is_kimi_code_subscription_route(&self) -> bool {
        let model = if self.uses_server_or_replay_metadata() {
            self.remote_provider_model
                .as_deref()
                .or(self.session.model.as_deref())
                .map(str::to_string)
        } else {
            Some(self.provider.model())
        };

        if model
            .as_deref()
            .map(is_kimi_code_subscription_model)
            .unwrap_or(false)
        {
            return true;
        }

        if self
            .remote_provider_name
            .as_deref()
            .map(is_kimi_code_provider_label)
            .unwrap_or(false)
        {
            return true;
        }

        active_kimi_code_profile()
    }

    pub(super) fn current_streaming_tps_elapsed(&self) -> Duration {
        let mut elapsed = self.streaming_tps_elapsed;
        if let Some(start) = self.streaming_tps_start {
            elapsed += start.elapsed();
        }
        elapsed
    }

    pub(super) fn snapshot_streaming_tps(&mut self) {
        self.streaming_tps_observed_output_tokens = self.streaming_total_output_tokens;
        self.streaming_tps_observed_elapsed = self.current_streaming_tps_elapsed();
    }

    pub(super) fn resume_streaming_tps(&mut self) {
        self.streaming_tps_collect_output = true;
        if self.streaming_tps_start.is_none() {
            self.streaming_tps_start = Some(Instant::now());
        }
    }

    pub(super) fn pause_streaming_tps(&mut self, keep_collecting_output: bool) {
        if let Some(start) = self.streaming_tps_start.take() {
            self.streaming_tps_elapsed += start.elapsed();
        }
        self.streaming_tps_collect_output = keep_collecting_output;
    }

    pub(super) fn reset_streaming_tps(&mut self) {
        self.streaming_tps_start = None;
        self.streaming_tps_elapsed = Duration::ZERO;
        self.streaming_tps_collect_output = false;
        self.streaming_total_output_tokens = 0;
        self.streaming_tps_observed_output_tokens = 0;
        self.streaming_tps_observed_elapsed = Duration::ZERO;
    }

    pub(super) fn open_usage_inline_loading(&mut self) {
        self.push_usage_loading_card();
        self.inline_interactive_state = None;
        self.inline_view_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice("Usage → refreshing");
    }

    pub(super) fn request_usage_report(&mut self) {
        use crate::bus::{Bus, BusEvent};

        if self.usage_report_refreshing {
            return;
        }
        self.usage_report_refreshing = true;

        let publish = || async move {
            let results = crate::usage::fetch_all_provider_usage_progressive(|progress| {
                Bus::global().publish(BusEvent::UsageReportProgress(progress));
            })
            .await;
            Bus::global().publish(BusEvent::UsageReport(results));
        };

        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(publish());
        } else {
            std::thread::spawn(move || {
                if let Ok(runtime) = tokio::runtime::Runtime::new() {
                    runtime.block_on(publish());
                }
            });
        }
    }

    pub(super) fn update_cost_impl(&mut self) {
        let provider_name = self.provider.name().to_lowercase();

        if provider_name.contains("openrouter") && self.is_kimi_code_subscription_route() {
            return;
        }

        // Only calculate cost for API-key providers
        if !provider_name.contains("openrouter")
            && !provider_name.contains("anthropic")
            && !provider_name.contains("openai")
        {
            return;
        }

        // For OAuth providers, cost is already tracked in subscription
        let is_oauth = (provider_name.contains("anthropic") || provider_name.contains("claude"))
            && std::env::var("ANTHROPIC_API_KEY").is_err();
        if is_oauth {
            return;
        }

        // Default pricing (will be cached after first turn)
        let prompt_price = *self.cached_prompt_price.get_or_insert(15.0); // $15/1M tokens default
        let completion_price = *self.cached_completion_price.get_or_insert(60.0); // $60/1M tokens default

        // Calculate cost for this turn
        let prompt_cost = (self.streaming_input_tokens as f32 * prompt_price) / 1_000_000.0;
        let completion_cost =
            (self.streaming_output_tokens as f32 * completion_price) / 1_000_000.0;
        self.total_cost += prompt_cost + completion_cost;
    }

    pub(super) fn compute_streaming_tps(&self) -> Option<f32> {
        let elapsed_secs = self.streaming_tps_observed_elapsed.as_secs_f32();
        let total_tokens = self.streaming_tps_observed_output_tokens;
        if elapsed_secs > 0.1 && total_tokens > 0 {
            Some(total_tokens as f32 / elapsed_secs)
        } else {
            None
        }
    }

    pub(super) fn handle_changelog_key(&mut self, code: KeyCode) -> Result<()> {
        let scroll = self.changelog_scroll.unwrap_or(0);
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.changelog_scroll = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.changelog_scroll = Some(scroll.saturating_add(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.changelog_scroll = Some(scroll.saturating_sub(1));
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.changelog_scroll = Some(scroll.saturating_add(20));
            }
            KeyCode::PageUp => {
                self.changelog_scroll = Some(scroll.saturating_sub(20));
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.changelog_scroll = Some(0);
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.changelog_scroll = Some(usize::MAX);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn handle_help_key(&mut self, code: KeyCode) -> Result<()> {
        let scroll = self.help_scroll.unwrap_or(0);
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.help_scroll = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_scroll = Some(scroll.saturating_add(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_scroll = Some(scroll.saturating_sub(1));
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.help_scroll = Some(scroll.saturating_add(20));
            }
            KeyCode::PageUp => {
                self.help_scroll = Some(scroll.saturating_sub(20));
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.help_scroll = Some(0);
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.help_scroll = Some(usize::MAX);
            }
            _ => {}
        }
        Ok(())
    }
}

fn is_kimi_code_subscription_model(model: &str) -> bool {
    let trimmed = model.trim();
    trimmed.eq_ignore_ascii_case("kimi-for-coding")
        || trimmed.eq_ignore_ascii_case("kimi:kimi-for-coding")
}

fn is_kimi_code_provider_label(provider: &str) -> bool {
    matches!(
        provider.trim().to_ascii_lowercase().as_str(),
        "kimi" | "kimi code" | "kimi-code"
    )
}

fn active_kimi_code_profile() -> bool {
    for key in [
        "JCODE_OPENROUTER_CACHE_NAMESPACE",
        "JCODE_NAMED_PROVIDER_PROFILE",
        "JCODE_PROVIDER_PROFILE_NAME",
    ] {
        if std::env::var(key)
            .ok()
            .as_deref()
            .map(is_kimi_code_provider_label)
            .unwrap_or(false)
        {
            return true;
        }
    }

    let kimi_base =
        crate::provider_catalog::normalize_api_base(crate::provider_catalog::KIMI_PROFILE.api_base);
    for key in ["JCODE_OPENROUTER_API_BASE", "JCODE_OPENAI_COMPAT_API_BASE"] {
        let Some(base) = std::env::var(key)
            .ok()
            .and_then(|value| crate::provider_catalog::normalize_api_base(&value))
        else {
            continue;
        };
        if kimi_base.as_deref() == Some(base.as_str()) {
            return true;
        }
    }

    false
}
