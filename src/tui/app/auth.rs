#[path = "auth_account_commands.rs"]
mod auth_account_commands;
#[path = "auth_account_picker.rs"]
mod auth_account_picker;
#[path = "auth_types.rs"]
mod auth_types;
pub(crate) use self::auth_account_commands::{
    handle_account_command_remote, handle_auth_command, resolve_account_provider_descriptor,
    save_openai_fast_setting_local,
};
pub(crate) use self::auth_types::StartupGuideAction;
pub(crate) use self::auth_types::{
    AccountCommand, PendingAccountInput, PendingLogin, SaitecLoginField, SaitecPendingForm,
};

use super::*;
use crossterm::event::{KeyCode, KeyModifiers};
use std::sync::Arc;

struct SuccessfulRemoteValidationTarget {
    provider_id: String,
    provider_display_name: String,
    model: String,
}

fn models_match(left: &str, right: &str) -> bool {
    let left = left.trim();
    let right = right.trim();
    !left.is_empty() && !right.is_empty() && (left == right || left.eq_ignore_ascii_case(right))
}

impl App {
    fn base_model_picker_selection_snapshot(
        &self,
    ) -> Option<crate::tui::login_picker::LoginPickerSelectionSnapshot> {
        let picker_cell = self.login_picker_overlay.as_ref()?;
        let picker = picker_cell.borrow();
        Some(picker.selection_snapshot())
    }

    fn abandon_pending_login_for_new_flow(&mut self) {
        if let Some((provider, method)) = self
            .pending_login
            .as_ref()
            .and_then(PendingLogin::telemetry_context)
        {
            crate::telemetry::record_auth_cancelled(&provider, &method);
        }
        self.pending_login = None;
        self.pending_text_entry_focus = super::PendingTextEntryFocus::Input;
    }

    /// After a login picker close (Esc/cancel), check if we should reopen the
    /// startup guide because the user was in the middle of first-run setup.
    pub(super) fn restore_startup_guide_if_needed(&mut self) {
        if self.display_user_message_count > 0 || !self.streaming_text.is_empty() {
            return;
        }
        let auth_status = crate::auth::AuthStatus::check_fast();
        let has_base_model = auth_status.has_any_base_model();
        let saitec_ok = crate::saitec::auth::ensure_logged_in().is_ok();

        if saitec_ok {
            return; // everything is fine
        }

        if has_base_model {
            // Reminder mode: BM okay but SAITEC missing
            self.begin_pending_login(PendingLogin::StartupGuide {
                focused: StartupGuideAction::LoginSaitec,
                is_reminder: true,
            });
        } else {
            // Setup mode: no BM configured yet
            self.begin_pending_login(PendingLogin::StartupGuide {
                focused: StartupGuideAction::LoginSaitec,
                is_reminder: false,
            });
        }
    }

    pub(crate) fn open_saitec_base_model_login_picker(&mut self) {
        use crate::tui::login_picker::{LoginPicker, LoginPickerItem, LoginPickerSummary};

        let status = crate::auth::AuthStatus::check_fast();
        let validation = crate::auth::validation::load_all();
        let providers = crate::provider_catalog::saitec_visible_base_model_providers();
        let mut summary = LoginPickerSummary::default();
        let items = providers
            .into_iter()
            .enumerate()
            .map(|(index, provider)| {
                let auth_state = status.state_for_provider(provider);
                let validation_record = validation.get(provider.id);
                match auth_state {
                    crate::auth::AuthState::Available => summary.ready_count += 1,
                    crate::auth::AuthState::Expired => summary.attention_count += 1,
                    crate::auth::AuthState::NotConfigured => summary.setup_count += 1,
                }
                if provider.recommended {
                    summary.recommended_count += 1;
                }

                LoginPickerItem::new(
                    index + 1,
                    provider,
                    auth_state,
                    validation_record.map(crate::auth::validation::format_record_label),
                    validation_record.map(|record| record.success),
                    status.method_detail_for_provider(provider),
                )
            })
            .collect();

        self.login_picker_overlay = Some(std::cell::RefCell::new(LoginPicker::with_summary(
            " Base-model Login ",
            items,
            summary,
        )));
        self.abandon_pending_login_for_new_flow();
        self.account_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice("Login: choose a base-model provider");
    }

    pub(crate) fn open_saitec_base_model_logout_picker(&mut self) {
        use crate::tui::login_picker::{LoginPicker, LoginPickerItem, LoginPickerSummary};

        let status = crate::auth::AuthStatus::check_fast();
        let validation = crate::auth::validation::load_all();
        let providers = crate::provider_catalog::saitec_visible_base_model_providers();
        let mut summary = LoginPickerSummary::default();
        let items = providers
            .into_iter()
            .enumerate()
            .map(|(index, provider)| {
                let auth_state = status.state_for_provider(provider);
                let validation_record = validation.get(provider.id);
                match auth_state {
                    crate::auth::AuthState::Available => summary.ready_count += 1,
                    crate::auth::AuthState::Expired => summary.attention_count += 1,
                    crate::auth::AuthState::NotConfigured => summary.setup_count += 1,
                }
                if provider.recommended {
                    summary.recommended_count += 1;
                }

                LoginPickerItem::new(
                    index + 1,
                    provider,
                    auth_state,
                    validation_record.map(crate::auth::validation::format_record_label),
                    validation_record.map(|record| record.success),
                    status.method_detail_for_provider(provider),
                )
            })
            .collect();

        self.login_picker_overlay = Some(std::cell::RefCell::new(
            LoginPicker::with_summary_and_primary_action(
                " Base-model Logout ",
                items,
                summary,
                "logout",
            ),
        ));
        self.abandon_pending_login_for_new_flow();
        self.account_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice("Logout: choose a base-model provider");
    }

    pub(crate) fn is_base_model_logout_picker_open(&self) -> bool {
        let Some(picker_cell) = self.login_picker_overlay.as_ref() else {
            return false;
        };
        picker_cell.borrow().title().trim() == "Base-model Logout"
    }

    pub(crate) fn refresh_open_saitec_base_model_login_picker(&mut self) {
        let snapshot = self.base_model_picker_selection_snapshot();
        self.open_saitec_base_model_login_picker();
        if let Some(snapshot) = snapshot
            && let Some(picker_cell) = self.login_picker_overlay.as_ref()
        {
            picker_cell.borrow_mut().restore_selection(&snapshot);
        }
    }

    pub(crate) fn start_login_picker_provider_validation(
        &mut self,
        provider: crate::provider_catalog::LoginProviderDescriptor,
    ) {
        self.set_status_notice(format!("Validation: checking {}...", provider.display_name));

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let result = crate::cli::auth_test::run_post_login_validation_quiet(provider).await;
                let (success, message) = match result {
                    Ok(()) => (
                        true,
                        format!("Runtime validation passed for {}.", provider.display_name),
                    ),
                    Err(error) => (false, error.to_string()),
                };
                crate::bus::Bus::global().publish(
                    crate::bus::BusEvent::ProviderValidationCompleted(
                        crate::bus::ProviderValidationCompleted {
                            provider: provider.id.to_string(),
                            provider_display_name: provider.display_name.to_string(),
                            success,
                            message,
                        },
                    ),
                );
            });
        } else {
            let message = format!(
                "Validation could not start for {} because no async runtime is available.",
                provider.display_name
            );
            self.push_display_message(DisplayMessage::error(message.clone()));
            self.set_status_notice(format!("Validation: {} failed", provider.display_name));
        }
    }

    pub(crate) fn open_login_mode_selector(&mut self) {
        use crate::tui::account_picker::{AccountPicker, AccountPickerItem};

        let items = vec![
            AccountPickerItem::action(
                "saitec-login",
                "SAITEC",
                "Business account",
                "sign in to SAITEC and unlock the TUI",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/login jcode".to_string(),
                ),
            ),
            AccountPickerItem::action(
                "model-config",
                "Base models",
                "OpenAI / Claude / Z.AI / Kimi / Alibaba",
                "open the filtered base-model login picker",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/login base-models".to_string(),
                ),
            ),
        ];

        self.abandon_pending_login_for_new_flow();
        self.account_picker_overlay = Some(std::cell::RefCell::new(AccountPicker::simple(
            " Login ", items,
        )));
        self.login_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice("Login: choose SAITEC or base models");
    }

    pub(crate) fn open_logout_mode_selector(&mut self) {
        use crate::tui::account_picker::{AccountPicker, AccountPickerItem};

        let items = vec![
            AccountPickerItem::action(
                "base-models",
                "Base models",
                "Base models",
                "manage OpenAI / Claude / Z.AI / Kimi / Alibaba credentials",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/logout base-models".to_string(),
                ),
            ),
            AccountPickerItem::action(
                "jcode",
                "SAITEC",
                "SAITEC",
                "requires confirmation before clearing SAITEC API credentials",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/logout jcode".to_string(),
                ),
            ),
        ];

        self.abandon_pending_login_for_new_flow();
        self.account_picker_overlay = Some(std::cell::RefCell::new(AccountPicker::simple(
            " Logout ", items,
        )));
        self.login_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice("Logout: choose target");
    }

    pub(crate) fn open_saitec_logout_confirmation(&mut self) {
        use crate::tui::account_picker::{AccountPicker, AccountPickerItem};

        let items = vec![
            AccountPickerItem::action(
                "logout-cancel",
                "Cancel",
                "Cancel",
                "keep SAITEC credentials",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/logout cancel".to_string(),
                ),
            ),
            AccountPickerItem::action(
                "logout-saitec",
                "Log out SAITEC",
                "Log out SAITEC",
                "clear ~/.saitec_tui/auth.json and the stored SAITEC API key",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/logout jcode --confirm".to_string(),
                ),
            ),
        ];

        self.abandon_pending_login_for_new_flow();
        self.account_picker_overlay = Some(std::cell::RefCell::new(AccountPicker::simple(
            " Confirm Logout ",
            items,
        )));
        self.login_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice("Logout: confirm SAITEC");
    }

    pub(crate) fn open_base_model_logout_confirmation(
        &mut self,
        provider: crate::provider_catalog::LoginProviderDescriptor,
    ) {
        use crate::tui::account_picker::{AccountPicker, AccountPickerItem};

        let items = vec![
            AccountPickerItem::action(
                "logout-cancel",
                "Cancel",
                "Cancel",
                format!("keep {} credentials", provider.display_name),
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(
                    "/logout cancel".to_string(),
                ),
            ),
            AccountPickerItem::action(
                provider.id,
                format!("Log out {}", provider.display_name),
                format!("Log out {}", provider.display_name),
                "clear only this base-model provider; SAITEC credentials stay unchanged",
                crate::tui::account_picker::AccountPickerCommand::SubmitInput(format!(
                    "/logout base-models {} --confirm",
                    provider.id
                )),
            ),
        ];

        self.abandon_pending_login_for_new_flow();
        self.account_picker_overlay = Some(std::cell::RefCell::new(AccountPicker::simple(
            " Confirm Logout ",
            items,
        )));
        self.login_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.set_status_notice(format!("Logout: confirm {}", provider.display_name));
    }

    pub(crate) fn is_login_mode_selector_open(&self) -> bool {
        let Some(picker_cell) = self.account_picker_overlay.as_ref() else {
            return false;
        };
        let picker = picker_cell.borrow();
        picker.is_simple_mode() && picker.title().trim() == "Login"
    }

    pub(crate) fn next_saitec_focus(current: SaitecLoginField, reverse: bool) -> SaitecLoginField {
        match (current, reverse) {
            (SaitecLoginField::Email, false) => SaitecLoginField::Phone,
            (SaitecLoginField::Phone, false) => SaitecLoginField::Password,
            (SaitecLoginField::Password, false) => SaitecLoginField::Submit,
            (SaitecLoginField::Submit, false) => SaitecLoginField::Cancel,
            (SaitecLoginField::Cancel, false) => SaitecLoginField::Email,
            (SaitecLoginField::Email, true) => SaitecLoginField::Cancel,
            (SaitecLoginField::Phone, true) => SaitecLoginField::Email,
            (SaitecLoginField::Password, true) => SaitecLoginField::Phone,
            (SaitecLoginField::Submit, true) => SaitecLoginField::Password,
            (SaitecLoginField::Cancel, true) => SaitecLoginField::Submit,
        }
    }

    pub(crate) fn stage_saitec_form(&mut self, form: SaitecPendingForm) {
        self.pending_login = Some(PendingLogin::SaitecForm { form });
    }

    pub(crate) fn sync_input_with_pending_saitec_form(&mut self) {
        if let Some(PendingLogin::SaitecForm { form }) = self.pending_login.as_ref() {
            self.input = match form.focus {
                SaitecLoginField::Email => form.form.email.clone(),
                SaitecLoginField::Phone => form.form.phone.clone(),
                SaitecLoginField::Password => form.form.password.clone(),
                SaitecLoginField::Submit | SaitecLoginField::Cancel => String::new(),
            };
            self.cursor_pos = self.input.len();
            self.clear_input_undo_history();
        }
    }

    pub(crate) fn advance_saitec_focus(&mut self, form: &mut SaitecPendingForm, reverse: bool) {
        form.focus = Self::next_saitec_focus(form.focus, reverse);
    }

    pub(crate) fn commit_input_to_pending_saitec_form(&mut self) -> bool {
        let Some(PendingLogin::SaitecForm { form }) = self.pending_login.as_mut() else {
            return false;
        };
        if form.submitting {
            return true;
        }

        match form.focus {
            SaitecLoginField::Email => {
                form.form.email = self.input.trim().to_string();
            }
            SaitecLoginField::Phone => {
                form.form.phone = self.input.trim().to_string();
            }
            SaitecLoginField::Password => {
                form.form.password = self.input.clone();
            }
            SaitecLoginField::Submit | SaitecLoginField::Cancel => {}
        }
        true
    }

    pub(crate) fn cancel_pending_saitec_form(&mut self) {
        if let Some((provider, method)) = self
            .pending_login
            .as_ref()
            .and_then(PendingLogin::telemetry_context)
        {
            crate::telemetry::record_auth_cancelled(&provider, &method);
        }
        self.pending_login = None;
        self.pending_text_entry_focus = super::PendingTextEntryFocus::Input;
        self.input.clear();
        self.cursor_pos = 0;
        self.clear_input_undo_history();
        self.follow_chat_bottom();
        self.push_display_message(DisplayMessage::system("Login cancelled.".to_string()));
        self.set_status_notice("Login cancelled");
    }

    pub(crate) fn handle_saitec_form_submit(&mut self, mut form: SaitecPendingForm) {
        match form.focus {
            SaitecLoginField::Email => form.form.email = self.input.trim().to_string(),
            SaitecLoginField::Phone => form.form.phone = self.input.trim().to_string(),
            SaitecLoginField::Password => form.form.password = self.input.clone(),
            SaitecLoginField::Submit => {}
            SaitecLoginField::Cancel => {
                self.pending_login = Some(PendingLogin::SaitecForm { form });
                self.cancel_pending_saitec_form();
                return;
            }
        }

        if let Err(err) = form.form.validate() {
            form.error = Some(err.to_string());
            form.focus = SaitecLoginField::Submit;
            form.submitting = false;
            self.set_status_notice("Login: validation failed");
            self.stage_saitec_form(form);
            self.sync_input_with_pending_saitec_form();
            return;
        }

        form.error = None;
        form.focus = SaitecLoginField::Submit;
        form.submitting = true;
        self.set_status_notice("Login [saitec]: submitting...");
        let login_form = form.form.clone();
        self.stage_saitec_form(form);
        self.sync_input_with_pending_saitec_form();
        tokio::spawn(async move {
            match crate::saitec::auth::submit_business_login(&login_form).await {
                Ok(session) => match crate::saitec::auth::save_session(&session) {
                    Ok(()) => {
                        crate::auth::AuthStatus::invalidate_cache();
                        Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                            provider: "jcode".to_string(),
                            success: true,
                            message: format!(
                                "**Saitec login successful.**\n\nAuthenticated as `{}` and stored credentials at `~/.saitec_tui/auth.json`.",
                                session.user_id.as_deref().unwrap_or("unknown-user")
                            ),
                        }));
                    }
                    Err(err) => {
                        Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                            provider: "jcode".to_string(),
                            success: false,
                            message: format!("Saitec login failed while saving auth: {}", err),
                        }));
                    }
                },
                Err(err) => {
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "jcode".to_string(),
                        success: false,
                        message: format!("Saitec login failed: {}", err),
                    }));
                }
            }
        });
        self.push_display_message(DisplayMessage::system(
            "Submitting Saitec credentials...".to_string(),
        ));
    }

    fn open_auth_browser(url: &str) -> bool {
        open::that_detached(url).is_ok()
    }

    fn record_oauth_preflight(
        provider_id: &str,
        browser_opened: bool,
        callback_target: Option<&str>,
        callback_available: Option<bool>,
    ) -> String {
        let mut notices = Vec::new();
        if !browser_opened {
            crate::telemetry::record_auth_surface_blocked_reason(
                provider_id,
                "oauth",
                crate::auth::login_diagnostics::AuthFailureReason::BrowserOpenFailed.label(),
            );
            notices.push("This machine could not open a browser automatically.".to_string());
        }
        if matches!(callback_available, Some(false)) {
            crate::telemetry::record_auth_surface_blocked_reason(
                provider_id,
                "oauth",
                crate::auth::login_diagnostics::AuthFailureReason::CallbackPortUnavailable.label(),
            );
            if let Some(target) = callback_target {
                notices.push(format!(
                    "Local callback target `{}` is unavailable, so SAITEC-TUI is using manual-safe paste completion instead.",
                    target
                ));
            } else {
                notices.push(
                    "The local callback listener is unavailable, so SAITEC-TUI is using manual-safe paste completion instead."
                        .to_string(),
                );
            }
        }
        if !notices.is_empty() {
            notices.push(format!(
                "If login still fails, run `jcode auth doctor {}` for a guided diagnosis.",
                provider_id
            ));
        }
        notices.join("\n")
    }

    pub(super) fn show_jcode_subscription_status(&mut self) {
        let configured_key = crate::subscription_catalog::configured_api_key().is_some();
        let core_api_base = crate::saitec::auth::core_api_base();
        let mut message = String::from("**SAITEC MCP Status**\n\n");
        message.push_str(&format!(
            "- Platform credentials: {}\n",
            if configured_key {
                "configured"
            } else {
                "not configured (`/login jcode`)"
            }
        ));
        message.push_str(&format!("- Core API base: `{}`\n", core_api_base));
        message.push_str("- MCP server: `SAITEC-Skills`\n");
        message.push_str(
            "\nSAITEC login grants platform API permission to MCP tools. It does not configure or switch a base model. Use `/login base-models` for model providers.",
        );
        self.push_display_message(DisplayMessage::system(message));
    }

    pub(super) fn show_interactive_login(&mut self) {
        self.open_login_mode_selector();
    }

    pub(super) fn start_login_provider(
        &mut self,
        provider: crate::provider_catalog::LoginProviderDescriptor,
    ) {
        crate::telemetry::record_provider_selected(provider.id);
        match provider.target {
            crate::provider_catalog::LoginProviderTarget::AutoImport => {
                match crate::cli::provider_init::pending_external_auth_review_candidates() {
                    Ok(candidates) if candidates.is_empty() => {
                        self.push_display_message(DisplayMessage::system(
                            "No importable external logins were found.".to_string(),
                        ));
                        self.set_status_notice("Login: no external imports found");
                    }
                    Ok(candidates) => {
                        self.push_display_message(DisplayMessage::system(
                            crate::cli::provider_init::format_external_auth_review_candidates_markdown(
                                &candidates,
                            ),
                        ));
                        self.set_status_notice("Login: choose sources to import");
                        self.pending_login = Some(PendingLogin::AutoImportSelection { candidates });
                    }
                    Err(err) => {
                        self.push_display_message(DisplayMessage::error(format!(
                            "Failed to inspect external login sources: {}",
                            err
                        )));
                        self.set_status_notice("Login: auto import failed");
                    }
                }
            }
            crate::provider_catalog::LoginProviderTarget::Jcode => self.start_jcode_login(),
            crate::provider_catalog::LoginProviderTarget::Claude => self.start_claude_login(),
            crate::provider_catalog::LoginProviderTarget::OpenAi => self.start_openai_login(),
            crate::provider_catalog::LoginProviderTarget::OpenAiApiKey => {
                self.start_openai_api_key_login()
            }
            crate::provider_catalog::LoginProviderTarget::OpenRouter => {
                self.start_openrouter_login()
            }
            crate::provider_catalog::LoginProviderTarget::Bedrock => self.start_bedrock_login(),
            crate::provider_catalog::LoginProviderTarget::Azure => {
                crate::telemetry::record_auth_surface_blocked(
                    provider.id,
                    provider.auth_kind.label(),
                );
                self.push_display_message(DisplayMessage::error(
                    "Azure OpenAI login is currently CLI-only. Run `jcode login --provider azure`."
                        .to_string(),
                ));
            }
            crate::provider_catalog::LoginProviderTarget::OpenAiCompatible(profile) => {
                self.start_openai_compatible_profile_login(profile)
            }
            crate::provider_catalog::LoginProviderTarget::Cursor => self.start_cursor_login(),
            crate::provider_catalog::LoginProviderTarget::Copilot => self.start_copilot_login(),
            crate::provider_catalog::LoginProviderTarget::Gemini => self.start_gemini_login(),
            crate::provider_catalog::LoginProviderTarget::Antigravity => {
                self.start_antigravity_login()
            }
            crate::provider_catalog::LoginProviderTarget::Google => {
                crate::telemetry::record_auth_surface_blocked(
                    provider.id,
                    provider.auth_kind.label(),
                );
                self.push_display_message(DisplayMessage::error(
                    "Google/Gmail login is only available from the CLI right now. Run `jcode login --provider google`."
                        .to_string(),
                ));
            }
        }
    }

    pub(super) fn begin_pending_login(&mut self, pending: PendingLogin) {
        if let Some((provider, method)) = pending.telemetry_context() {
            crate::telemetry::record_auth_started(&provider, &method);
        }
        self.pending_login = Some(pending);
        self.pending_text_entry_focus = super::PendingTextEntryFocus::Input;
    }

    fn load_saved_api_key_for_prefill(key_name: &str, env_file: &str) -> Option<String> {
        crate::provider_catalog::load_env_value_from_env_or_config(key_name, env_file).or_else(
            || {
                if key_name == "ZHIPU_API_KEY" {
                    crate::provider_catalog::load_env_value_from_env_or_config(
                        "ZAI_API_KEY",
                        env_file,
                    )
                } else {
                    None
                }
            },
        )
    }

    fn clear_saved_api_key_value(key_name: &str, env_file: &str) -> anyhow::Result<()> {
        crate::provider_catalog::save_env_value_to_env_file(key_name, env_file, None)?;
        if key_name == "ZHIPU_API_KEY" {
            crate::provider_catalog::save_env_value_to_env_file("ZAI_API_KEY", env_file, None)?;
        }
        Ok(())
    }

    pub(super) fn clear_pending_api_key_login_value(&mut self) {
        let Some(PendingLogin::ApiKeyProfile {
            provider,
            env_file,
            key_name,
            ..
        }) = self.pending_login.as_ref()
        else {
            return;
        };
        let provider = provider.clone();
        let env_file = env_file.clone();
        let key_name = key_name.clone();

        match Self::clear_saved_api_key_value(&key_name, &env_file) {
            Ok(()) => {
                crate::auth::AuthStatus::invalidate_cache();
                self.input.clear();
                self.cursor_pos = 0;
                self.clear_input_undo_history();
                self.pending_text_entry_focus = super::PendingTextEntryFocus::Input;
                self.push_display_message(DisplayMessage::system(format!(
                    "{} API key cleared. Paste a new key or choose Cancel.",
                    provider
                )));
                self.set_status_notice(format!("Login: cleared {} key", provider));
            }
            Err(error) => {
                self.pending_text_entry_focus = super::PendingTextEntryFocus::Input;
                self.push_display_message(DisplayMessage::error(format!(
                    "Failed to clear {} key: {}",
                    provider, error
                )));
                self.set_status_notice(format!("Login: failed to clear {} key", provider));
            }
        }
    }

    fn start_claude_login(&mut self) {
        let label = crate::auth::claude::login_target_label(None)
            .unwrap_or_else(|_| crate::auth::claude::primary_account_label());
        self.start_claude_login_for_account(&label);
    }

    pub(super) fn start_jcode_login(&mut self) {
        crate::logging::info("login-debug: start_jcode_login opened Saitec form");
        self.abandon_pending_login_for_new_flow();
        self.login_picker_overlay = None;
        self.account_picker_overlay = None;
        self.inline_interactive_state = None;
        self.input.clear();
        self.cursor_pos = 0;
        self.clear_input_undo_history();
        self.push_display_message(DisplayMessage::system(
            "**Saitec Login**\n\nEnter your email or phone plus password to continue.".to_string(),
        ));
        self.set_status_notice("Login: credentials required");
        self.begin_pending_login(PendingLogin::SaitecForm {
            form: SaitecPendingForm {
                form: crate::saitec::auth::SaitecLoginForm::new(
                    "".to_string(),
                    "".to_string(),
                    "".to_string(),
                ),
                focus: SaitecLoginField::Email,
                error: None,
                submitting: false,
            },
        });
    }

    pub(super) fn start_claude_login_for_account(&mut self, label: &str) {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        use sha2::{Digest, Sha256};

        let verifier: String = {
            use rand::Rng;
            const CHARSET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            let mut rng = rand::rng();
            (0..64)
                .map(|_| {
                    let idx = rng.random_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        };

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = URL_SAFE_NO_PAD.encode(hash);

        let auth_url = crate::auth::oauth::claude_auth_url(
            crate::auth::oauth::claude::REDIRECT_URI,
            &challenge,
            &verifier,
        );
        let qr_section = crate::login_qr::markdown_section_for_tui(
            &auth_url,
            "Scan this on another device if this machine has no browser:",
        )
        .map(|section| format!("\n\n{section}"))
        .unwrap_or_default();

        let browser_opened = Self::open_auth_browser(&auth_url);
        let preflight = Self::record_oauth_preflight("claude", browser_opened, None, None);

        self.push_display_message(DisplayMessage::system(format!(
            "**Claude OAuth Login** (account: `{}`)\n\n\
             Opening browser for authentication...\n\n\
             If the browser didn't open, visit:\n{}\n\n\
             {}{}{}After logging in, copy the callback URL or authorization code and **paste it here**. Type `/cancel` to abort.{}",
            label,
            auth_url,
            if preflight.is_empty() { "" } else { &preflight },
            if preflight.is_empty() { "" } else { "\n\n" },
            if preflight.is_empty() {
                ""
            } else {
                "Manual-safe fallback is already available here.\n\n"
            },
            qr_section
        )));
        self.set_status_notice(format!("Login [{}]: paste code...", label));
        self.begin_pending_login(PendingLogin::ClaudeAccount {
            verifier,
            label: label.to_string(),
            redirect_uri: None,
        });
    }

    pub(super) fn switch_account(&mut self, label: &str) {
        match crate::auth::claude::set_active_account(label) {
            Ok(()) => {
                {
                    let provider = self.provider.clone();
                    let label_owned = label.to_string();
                    tokio::spawn(async move {
                        provider.invalidate_credentials().await;
                        crate::logging::info(&format!(
                            "Switched to Anthropic account '{}'",
                            label_owned
                        ));
                    });
                }
                self.push_display_message(DisplayMessage::system(format!(
                    "Switched to Anthropic account `{}`.",
                    label
                )));
                // Keep account-sensitive UI state in sync immediately.
                crate::auth::AuthStatus::invalidate_cache();
                self.context_limit = self.provider.context_window() as u64;
                self.context_warning_shown = false;
            }
            Err(e) => {
                self.push_display_message(DisplayMessage::error(format!(
                    "Failed to switch account: {}",
                    e
                )));
            }
        }
    }

    pub(super) fn switch_account_by_label(&mut self, label: &str) {
        let has_anthropic = crate::auth::claude::list_accounts()
            .unwrap_or_default()
            .iter()
            .any(|account| account.label == label);
        let has_openai = crate::auth::codex::list_accounts()
            .unwrap_or_default()
            .iter()
            .any(|account| account.label == label);

        match (has_anthropic, has_openai) {
            (true, false) => self.switch_account(label),
            (false, true) => self.switch_openai_account(label),
            (true, true) => self.push_display_message(DisplayMessage::error(format!(
                "Account label `{}` exists for both Anthropic and OpenAI. Use `/account switch {}` or `/account openai switch {}` explicitly.",
                label, label, label
            ))),
            (false, false) => self.push_display_message(DisplayMessage::error(format!(
                "No Anthropic or OpenAI account with label `{}` found.",
                label
            ))),
        }
    }

    pub(super) fn remove_account(&mut self, label: &str) {
        match crate::auth::claude::remove_account(label) {
            Ok(()) => {
                self.push_display_message(DisplayMessage::system(format!(
                    "Removed Anthropic account `{}`.",
                    label
                )));
            }
            Err(e) => {
                self.push_display_message(DisplayMessage::error(format!(
                    "Failed to remove account: {}",
                    e
                )));
            }
        }
    }

    pub(super) fn switch_openai_account(&mut self, label: &str) {
        match crate::auth::codex::set_active_account(label) {
            Ok(()) => {
                {
                    let provider = self.provider.clone();
                    let label_owned = label.to_string();
                    tokio::spawn(async move {
                        provider.invalidate_credentials().await;
                        crate::logging::info(&format!(
                            "Switched to OpenAI account '{}'",
                            label_owned
                        ));
                    });
                }
                self.push_display_message(DisplayMessage::system(format!(
                    "Switched to OpenAI account `{}`.",
                    label
                )));
                crate::auth::AuthStatus::invalidate_cache();
                self.context_limit = self.provider.context_window() as u64;
                self.context_warning_shown = false;
            }
            Err(e) => {
                self.push_display_message(DisplayMessage::error(format!(
                    "Failed to switch OpenAI account: {}",
                    e
                )));
            }
        }
    }

    pub(super) fn remove_openai_account(&mut self, label: &str) {
        match crate::auth::codex::remove_account(label) {
            Ok(()) => {
                self.push_display_message(DisplayMessage::system(format!(
                    "Removed OpenAI account `{}`.",
                    label
                )));
            }
            Err(e) => {
                self.push_display_message(DisplayMessage::error(format!(
                    "Failed to remove OpenAI account: {}",
                    e
                )));
            }
        }
    }

    fn start_openai_login(&mut self) {
        let label = crate::auth::codex::login_target_label(None)
            .unwrap_or_else(|_| crate::auth::codex::primary_account_label());
        self.start_openai_login_for_account(&label);
    }

    pub(super) fn start_openai_login_for_account(&mut self, label: &str) {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        use sha2::{Digest, Sha256};

        let verifier: String = {
            use rand::Rng;
            const CHARSET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            let mut rng = rand::rng();
            (0..64)
                .map(|_| {
                    let idx = rng.random_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        };

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = URL_SAFE_NO_PAD.encode(hash);

        let state: String = {
            let bytes: [u8; 16] = rand::random();
            hex::encode(bytes)
        };

        let port = crate::auth::oauth::openai::DEFAULT_PORT;
        let redirect_uri = crate::auth::oauth::openai::redirect_uri(port);
        let auth_url = crate::auth::oauth::openai_auth_url_with_prompt(
            &redirect_uri,
            &challenge,
            &state,
            Some("login"),
        );
        let qr_section = crate::login_qr::markdown_section_for_tui(
            &auth_url,
            "Scan this on another device if this machine has no browser, then paste the full callback URL here:",
        )
        .map(|section| format!("\n\n{section}"))
        .unwrap_or_default();

        let callback_listener = crate::auth::oauth::bind_callback_listener(port).ok();
        let callback_available = callback_listener.is_some();
        let browser_opened = Self::open_auth_browser(&auth_url);
        let label_owned = label.to_string();

        if let Some(listener) = callback_listener {
            let verifier_clone = verifier.clone();
            let state_clone = state.clone();
            let label_clone = label_owned.clone();
            tokio::spawn(async move {
                match Self::openai_login_callback(
                    verifier_clone,
                    state_clone,
                    Some(label_clone),
                    listener,
                )
                .await
                {
                    Ok(msg) => {
                        crate::logging::info(&format!("OpenAI login: {}", msg));
                        Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                            provider: "openai".to_string(),
                            success: true,
                            message: msg,
                        }));
                    }
                    Err(e) => {
                        crate::logging::info(&format!(
                            "OpenAI automatic callback did not complete: {}",
                            e
                        ));
                    }
                }
            });
        }

        let callback_line = if callback_available {
            format!(
                "Waiting for callback on `localhost:{}`... (this will complete automatically)\n",
                port
            )
        } else {
            format!(
                "Local callback port `localhost:{}` is unavailable, so finish in any browser and paste the full callback URL here.\n",
                port
            )
        };
        let preflight = Self::record_oauth_preflight(
            "openai",
            browser_opened,
            Some(&format!("localhost:{}", port)),
            Some(callback_available),
        );

        self.push_display_message(DisplayMessage::system(format!(
            "**OpenAI OAuth Login** (account: `{}`)\n\n\
             Opening browser for authentication...\n\n\
             If the browser didn't open, visit:\n{}\n\n\
             **Note:** Wait a few seconds for the page to fully load before clicking Continue. \
             OpenAI's verification system may briefly disable the button.\n\n\
             {}{}{}\
             Or paste the full callback URL or query string here to finish from another device. Type `/cancel` to abort.{}",
            label,
            auth_url,
            if preflight.is_empty() {
                String::new()
            } else {
                format!("{}\n", preflight)
            },
            callback_line,
            if preflight.is_empty() {
                String::new()
            } else {
                "Manual-safe fallback is already active here.\n".to_string()
            },
            qr_section
        )));
        self.set_status_notice(format!("Login [{}]: waiting...", label));
        self.begin_pending_login(PendingLogin::OpenAiAccount {
            verifier,
            label: label.to_string(),
            expected_state: state,
            redirect_uri,
        });
    }

    async fn openai_login_callback(
        verifier: String,
        expected_state: String,
        label: Option<String>,
        listener: tokio::net::TcpListener,
    ) -> Result<String, String> {
        let port = crate::auth::oauth::openai::DEFAULT_PORT;
        let redirect_uri = crate::auth::oauth::openai::redirect_uri(port);
        let code = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            crate::auth::oauth::wait_for_callback_async_on_listener(listener, &expected_state),
        )
        .await
        .map_err(|_| "Login timed out after 5 minutes. Please try again.".to_string())?
        .map_err(|e| format!("Callback failed: {}", e))?;

        Self::openai_token_exchange(verifier, code, label, None, &redirect_uri).await
    }

    async fn openai_token_exchange(
        verifier: String,
        input: String,
        label: Option<String>,
        expected_state: Option<String>,
        redirect_uri: &str,
    ) -> Result<String, String> {
        let oauth_tokens = if let Some(expected_state) = expected_state {
            crate::auth::oauth::exchange_openai_callback_input(
                &verifier,
                input.trim(),
                &expected_state,
                redirect_uri,
            )
            .await
            .map_err(|e| e.to_string())?
        } else {
            crate::auth::oauth::exchange_openai_code(&input, &verifier, redirect_uri)
                .await
                .map_err(|e| e.to_string())?
        };

        let label = label.unwrap_or_else(crate::auth::codex::primary_account_label);
        crate::auth::oauth::save_openai_tokens_for_account(&oauth_tokens, &label)
            .map_err(|e| format!("Failed to save tokens: {}", e))?;

        Ok(format!(
            "Successfully logged in to OpenAI! (account: {})",
            label
        ))
    }

    fn start_gemini_login(&mut self) {
        let (verifier, challenge) = crate::auth::oauth::generate_pkce_public();
        let state = crate::auth::oauth::generate_state_public();

        let callback_listener = crate::auth::oauth::bind_callback_listener(0).ok();
        let maybe_redirect_uri = callback_listener
            .as_ref()
            .and_then(|listener| listener.local_addr().ok())
            .map(|addr| format!("http://127.0.0.1:{}/oauth2callback", addr.port()));

        let auth_setup: anyhow::Result<(String, Option<String>, String)> =
            if let Some(redirect_uri) = maybe_redirect_uri {
                crate::auth::gemini::build_web_auth_url(&redirect_uri, &challenge, &state)
                    .map(|auth_url| (auth_url, Some(state.clone()), redirect_uri))
            } else {
                crate::auth::gemini::build_manual_auth_url(
                    "https://codeassist.google.com/authcode",
                    &challenge,
                    &state,
                )
                .map(|auth_url| {
                    (
                        auth_url,
                        None,
                        "https://codeassist.google.com/authcode".to_string(),
                    )
                })
            };

        let (auth_url, pending_state, redirect_uri) = match auth_setup {
            Ok(values) => values,
            Err(e) => {
                self.push_display_message(DisplayMessage::error(format!(
                    "Gemini login is unavailable: {}",
                    e
                )));
                self.set_status_notice("Login: failed");
                return;
            }
        };

        let qr_section = crate::login_qr::markdown_section_for_tui(
            &auth_url,
            "Scan this on another device if this machine has no browser, then paste the callback URL or authorization code here:",
        )
        .map(|section| format!("\n\n{section}"))
        .unwrap_or_default();

        let browser_opened = Self::open_auth_browser(&auth_url);
        let callback_available = callback_listener.is_some() && pending_state.is_some();

        if let (Some(listener), Some(expected_state)) = (callback_listener, pending_state.clone()) {
            let redirect_clone = redirect_uri.clone();
            let verifier_clone = verifier.clone();
            tokio::spawn(async move {
                let code = tokio::time::timeout(
                    std::time::Duration::from_secs(300),
                    crate::auth::oauth::wait_for_callback_async_on_listener(
                        listener,
                        &expected_state,
                    ),
                )
                .await
                .map_err(|_| "Login timed out after 5 minutes. Please try again.".to_string())
                .and_then(|result| result.map_err(|e| format!("Callback failed: {}", e)));

                match code {
                    Ok(code) => {
                        match crate::auth::gemini::exchange_callback_code(
                            &code,
                            &verifier_clone,
                            &redirect_clone,
                        )
                        .await
                        {
                            Ok(tokens) => {
                                let msg = if let Some(email) = tokens.email {
                                    format!(
                                        "Successfully logged in to Gemini! (account: {})",
                                        email
                                    )
                                } else {
                                    "Successfully logged in to Gemini!".to_string()
                                };
                                Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                    provider: "gemini".to_string(),
                                    success: true,
                                    message: msg,
                                }));
                            }
                            Err(e) => {
                                let message = format!("Gemini login failed: {}", e);
                                crate::logging::info(&format!(
                                    "Gemini automatic callback did not complete: {}",
                                    e
                                ));
                                Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                    provider: "gemini".to_string(),
                                    success: false,
                                    message,
                                }));
                            }
                        }
                    }
                    Err(e) => {
                        crate::logging::info(&format!(
                            "Gemini automatic callback did not complete: {}",
                            e
                        ));
                        Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                            provider: "gemini".to_string(),
                            success: false,
                            message: format!("Gemini login failed: {}", e),
                        }));
                    }
                }
            });
        }

        let callback_line = if callback_available {
            format!(
                "Waiting for callback on `{}`... (this will complete automatically)\n",
                redirect_uri
            )
        } else {
            "Finish login in any browser, then paste the callback URL or authorization code here.\n"
                .to_string()
        };
        let preflight = Self::record_oauth_preflight(
            "gemini",
            browser_opened,
            Some(&redirect_uri),
            Some(callback_available),
        );

        self.push_display_message(DisplayMessage::system(format!(
            "**Gemini OAuth Login**\n\n\
             Opening browser for authentication...\n\n\
             If the browser didn't open, visit:\n{}\n\n\
             {}{}{}\
             Or paste the full callback URL, query string, or authorization code here to finish. Type `/cancel` to abort.{}",
            auth_url,
            if preflight.is_empty() {
                String::new()
            } else {
                format!("{}\n", preflight)
            },
            callback_line,
            if preflight.is_empty() {
                String::new()
            } else {
                "Manual-safe fallback is already active here.\n".to_string()
            },
            qr_section
        )));
        self.set_status_notice("Login: waiting...");
        self.begin_pending_login(PendingLogin::Gemini {
            verifier,
            expected_state: pending_state,
            redirect_uri,
        });
    }

    fn start_openrouter_login(&mut self) {
        self.start_api_key_login(
            "OpenRouter",
            "https://openrouter.ai/keys",
            "openrouter.env",
            "OPENROUTER_API_KEY",
            None,
            None,
            false,
            None,
        );
    }

    fn start_bedrock_login(&mut self) {
        self.start_api_key_login(
            "AWS Bedrock",
            "https://console.aws.amazon.com/bedrock/home#/api-keys",
            crate::provider::bedrock::ENV_FILE,
            crate::provider::bedrock::API_KEY_ENV,
            Some("us.amazon.nova-micro-v1:0"),
            Some(
                "Region: us-east-2 (default for TUI onboarding; use CLI login for another region)",
            ),
            false,
            None,
        );
    }

    fn start_openai_api_key_login(&mut self) {
        self.start_api_key_login(
            "OpenAI API",
            "https://platform.openai.com/api-keys",
            "openai.env",
            "OPENAI_API_KEY",
            Some("gpt-5.5"),
            Some("https://api.openai.com/v1"),
            false,
            None,
        );
    }

    fn start_openai_compatible_profile_login(
        &mut self,
        profile: crate::provider_catalog::OpenAiCompatibleProfile,
    ) {
        if profile.id == crate::provider_catalog::OPENAI_COMPAT_PROFILE.id {
            let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
            self.push_display_message(DisplayMessage::system(format!(
                "**{} Endpoint**\n\n\
                 Setup docs: {}\n\
                 Current API base: `{}`\n\n\
                 **Paste the API base below**. Press Enter to keep the current value, or use Up/Down to select Validate or Cancel.",
                resolved.display_name, resolved.setup_url, resolved.api_base
            )));
            self.set_status_notice("Login: API base...");
            self.pending_login = Some(PendingLogin::OpenAiCompatibleApiBase { profile });
            return;
        }

        self.start_openai_compatible_key_login(profile);
    }

    fn start_openai_compatible_key_login(
        &mut self,
        profile: crate::provider_catalog::OpenAiCompatibleProfile,
    ) {
        let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
        self.start_api_key_login(
            &resolved.display_name,
            &resolved.setup_url,
            &resolved.env_file,
            &resolved.api_key_env,
            resolved.default_model.as_deref(),
            Some(&resolved.api_base),
            !resolved.requires_api_key,
            Some(profile),
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "API-key login setup passes provider-specific metadata assembled at call sites"
    )]
    fn start_api_key_login(
        &mut self,
        provider: &str,
        docs_url: &str,
        env_file: &str,
        key_name: &str,
        default_model: Option<&str>,
        endpoint: Option<&str>,
        api_key_optional: bool,
        openai_compatible_profile: Option<crate::provider_catalog::OpenAiCompatibleProfile>,
    ) {
        let model_hint = default_model
            .map(|m| format!("Suggested default model: `{}`\n\n", m))
            .unwrap_or_default();
        let endpoint_hint = endpoint
            .map(|endpoint| format!("Endpoint: `{}`\n", endpoint))
            .unwrap_or_default();
        let prompt = if api_key_optional {
            "**Paste your API key below** if your endpoint requires one. Press Enter to skip, or use Up/Down to select Validate or Cancel."
        } else {
            "**Paste your API key below** (it will be saved securely), or use Up/Down to select Validate or Cancel."
        };
        self.push_display_message(DisplayMessage::system(format!(
            "**{} {}**\n\n\
             Setup docs: {}\n\
             Stored variable: `{}`\n\
             {}\
             {}\n\
             {}",
            provider,
            if api_key_optional {
                "Local Endpoint"
            } else {
                "API Key"
            },
            docs_url,
            key_name,
            endpoint_hint,
            model_hint,
            prompt,
        )));
        self.set_status_notice(if api_key_optional {
            "Login: optional key..."
        } else {
            "Login: paste key..."
        });
        let provider_id = openai_compatible_profile
            .map(|profile| profile.id.to_string())
            .unwrap_or_else(|| match key_name {
                crate::subscription_catalog::JCODE_API_KEY_ENV => "jcode".to_string(),
                "OPENROUTER_API_KEY" => "openrouter".to_string(),
                _ => provider.to_ascii_lowercase().replace(' ', "-"),
            });
        let auth_method = if api_key_optional {
            "local_endpoint"
        } else {
            "api_key"
        };
        let saved_key = Self::load_saved_api_key_for_prefill(key_name, env_file);
        self.begin_pending_login(PendingLogin::ApiKeyProfile {
            provider_id,
            provider: provider.to_string(),
            auth_method: auth_method.to_string(),
            docs_url: docs_url.to_string(),
            env_file: env_file.to_string(),
            key_name: key_name.to_string(),
            default_model: default_model.map(|m| m.to_string()),
            endpoint: endpoint.map(|value| value.to_string()),
            api_key_optional,
            openai_compatible_profile,
        });
        self.input = saved_key.unwrap_or_default();
        self.cursor_pos = self.input.len();
        self.clear_input_undo_history();
    }

    fn start_cursor_login(&mut self) {
        crate::telemetry::record_auth_started("cursor", "api_key");

        self.push_display_message(DisplayMessage::system(
            "**Cursor API Key**\n\n\
             Get your API key from: https://cursor.com/settings\n\
             (Dashboard > Integrations > User API Keys)\n\n\
             SAITEC-TUI will save it securely and use the native Cursor HTTPS transport.\n\n\
             **Paste your API key below**, or use Up/Down to select Validate or Cancel."
                .to_string(),
        ));
        self.set_status_notice("Login: paste cursor key...");
        self.begin_pending_login(PendingLogin::CursorApiKey);
    }

    fn start_copilot_login(&mut self) {
        self.set_status_notice("Login: copilot device flow...");
        self.begin_pending_login(PendingLogin::Copilot);

        tokio::spawn(async move {
            let client = reqwest::Client::new();

            let device_resp = match crate::auth::copilot::initiate_device_flow(&client).await {
                Ok(resp) => resp,
                Err(e) => {
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "copilot".to_string(),
                        success: false,
                        message: format!("Copilot device flow failed: {}", e),
                    }));
                    return;
                }
            };

            let user_code = device_resp.user_code.clone();
            let verification_uri = device_resp.verification_uri.clone();

            let clipboard_ok = copy_to_clipboard(&user_code);
            let clipboard_msg = if clipboard_ok {
                " (copied to clipboard - just paste it!)"
            } else {
                ""
            };

            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                provider: "copilot_code".to_string(),
                success: true,
                message: {
                    let qr_section = crate::login_qr::markdown_section_for_tui(
                        &verification_uri,
                        "Scan this on another device to open the GitHub verification page:",
                    )
                    .map(|section| format!("\n\n{section}"))
                    .unwrap_or_default();
                    format!(
                        "**GitHub Copilot Login**\n\n\
                         Your code: **{}**{}\n\n\
                         Opening browser to {} ...\n\
                         Paste the code there and authorize.{}\n\n\
                         Waiting for authorization... (type `/cancel` to abort)",
                        user_code, clipboard_msg, verification_uri, qr_section
                    )
                },
            }));

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = open::that_detached(&verification_uri);

            let token = match crate::auth::copilot::poll_for_access_token(
                &client,
                &device_resp.device_code,
                device_resp.interval,
            )
            .await
            {
                Ok(t) => t,
                Err(e) => {
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "copilot".to_string(),
                        success: false,
                        message: format!("Copilot login failed: {}", e),
                    }));
                    return;
                }
            };

            let username = crate::auth::copilot::fetch_github_username(&client, &token)
                .await
                .unwrap_or_else(|_| "unknown".to_string());

            match crate::auth::copilot::save_github_token(&token, &username) {
                Ok(()) => {
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "copilot".to_string(),
                        success: true,
                        message: format!(
                            "Authenticated as **{}** via GitHub Copilot.\n\n\
                             Copilot models are now available in `/model`.",
                            username
                        ),
                    }));
                }
                Err(e) => {
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "copilot".to_string(),
                        success: false,
                        message: format!("Failed to save Copilot token: {}", e),
                    }));
                }
            }
        });

        self.push_display_message(DisplayMessage::system(
            "**GitHub Copilot Login**\n\n\
             Starting device flow... please wait. Type `/cancel` to abort."
                .to_string(),
        ));
    }

    fn start_antigravity_login(&mut self) {
        let (verifier, challenge) = crate::auth::oauth::generate_pkce_public();
        let expected_state = crate::auth::oauth::generate_state_public();
        let port = crate::auth::antigravity::DEFAULT_PORT;
        let redirect_uri = crate::auth::antigravity::redirect_uri(port);

        let auth_url = match crate::auth::antigravity::build_auth_url(
            &redirect_uri,
            &challenge,
            &expected_state,
        ) {
            Ok(url) => url,
            Err(e) => {
                self.push_display_message(DisplayMessage::error(format!(
                    "Antigravity login is unavailable: {}",
                    e
                )));
                self.set_status_notice("Login: failed");
                return;
            }
        };

        let qr_section = crate::login_qr::markdown_section_for_tui(
            &auth_url,
            "Scan this on another device if this machine has no browser, then paste the full callback URL or query string here:",
        )
        .map(|section| format!("\n\n{section}"))
        .unwrap_or_default();

        let callback_listener = crate::auth::oauth::bind_callback_listener(port).ok();
        let callback_available = callback_listener.is_some();
        let browser_opened = Self::open_auth_browser(&auth_url);

        if let Some(listener) = callback_listener {
            let verifier_clone = verifier.clone();
            let expected_state_clone = expected_state.clone();
            let redirect_clone = redirect_uri.clone();
            tokio::spawn(async move {
                let code = tokio::time::timeout(
                    std::time::Duration::from_secs(300),
                    crate::auth::oauth::wait_for_callback_async_on_listener(
                        listener,
                        &expected_state_clone,
                    ),
                )
                .await
                .map_err(|_| "Login timed out after 5 minutes. Please try again.".to_string())
                .and_then(|result| result.map_err(|e| format!("Callback failed: {}", e)));

                match code {
                    Ok(code) => {
                        match Self::antigravity_token_exchange(
                            verifier_clone,
                            code,
                            Some(expected_state_clone),
                            redirect_clone,
                        )
                        .await
                        {
                            Ok(msg) => {
                                Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                    provider: "antigravity".to_string(),
                                    success: true,
                                    message: msg,
                                }));
                            }
                            Err(e) => {
                                Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                    provider: "antigravity".to_string(),
                                    success: false,
                                    message: format!("Antigravity login failed: {}", e),
                                }));
                            }
                        }
                    }
                    Err(e) => {
                        crate::logging::info(&format!(
                            "Antigravity automatic callback did not complete: {}",
                            e
                        ));
                    }
                }
            });
        }

        let callback_line = if callback_available {
            format!(
                "Waiting for callback on `{}`... (this will complete automatically)\n",
                redirect_uri
            )
        } else {
            format!(
                "Local callback port `{}` is unavailable, so finish in any browser and paste the full callback URL or query string here.\n",
                redirect_uri
            )
        };
        let preflight = Self::record_oauth_preflight(
            "antigravity",
            browser_opened,
            Some(&redirect_uri),
            Some(callback_available),
        );
        let manual_hint = "If the browser ends on a loopback/callback error page, copy the full URL from the address bar and paste it here immediately.\n";

        self.push_display_message(DisplayMessage::system(format!(
            "**Antigravity OAuth Login**\n\n\
             Opening browser for authentication...\n\n\
             If the browser didn't open, visit:\n{}\n\n\
             {}{}{}{}\
             Or paste the full callback URL or query string here to finish. Type `/cancel` to abort.{}",
            auth_url,
            if preflight.is_empty() {
                String::new()
            } else {
                format!("{}\n", preflight)
            },
            callback_line,
            manual_hint,
            if preflight.is_empty() {
                String::new()
            } else {
                "Manual-safe fallback is already active here.\n".to_string()
            },
            qr_section
        )));
        self.set_status_notice("Login: antigravity waiting...");
        self.begin_pending_login(PendingLogin::Antigravity {
            verifier,
            expected_state,
            redirect_uri,
        });
    }

    async fn antigravity_token_exchange(
        verifier: String,
        input: String,
        expected_state: Option<String>,
        redirect_uri: String,
    ) -> Result<String, String> {
        let trimmed = input.trim();
        let tokens =
            if antigravity_input_requires_state_validation(trimmed, expected_state.as_deref()) {
                crate::auth::antigravity::exchange_callback_input(
                    &verifier,
                    trimmed,
                    expected_state.as_deref(),
                    &redirect_uri,
                )
                .await
            } else {
                crate::auth::antigravity::exchange_callback_code(trimmed, &verifier, &redirect_uri)
                    .await
            }
            .map_err(|e| e.to_string())?;

        let mut msg = if let Some(email) = tokens.email.as_deref() {
            format!(
                "Successfully logged in to Antigravity! (account: {})",
                email
            )
        } else {
            "Successfully logged in to Antigravity!".to_string()
        };
        if let Some(project_id) = tokens.project_id.as_deref() {
            msg.push_str(&format!(" (project: {})", project_id));
        }
        Ok(msg)
    }

    pub(super) fn handle_login_input(&mut self, pending: PendingLogin, input: String) {
        let trimmed = input.trim();
        if trimmed == "/cancel" {
            if let Some((provider, method)) = pending.telemetry_context() {
                crate::telemetry::record_auth_cancelled(&provider, &method);
            }
            self.push_display_message(DisplayMessage::system("Login cancelled.".to_string()));
            return;
        }

        if super::auth::handle_auth_command(self, trimmed, None) {
            return;
        }

        if trimmed.is_empty()
            && !matches!(
                pending,
                PendingLogin::SaitecForm { .. } | PendingLogin::StartupGuide { .. }
            )
        {
            let help = match &pending {
                PendingLogin::AutoImportSelection { .. } => {
                    "Auto import is waiting for your selection. Reply with `a` to approve all, `1,3` to approve specific sources, or `/cancel` to abort.".to_string()
                }
                PendingLogin::SaitecForm { .. } => {
                    "Saitec login form is open. Fill in email or phone plus password, then submit. Type `/cancel` to abort.".to_string()
                }
                _ => "Login still in progress. Complete it in your browser, or paste the callback URL / authorization code here. Type `/cancel` to abort.".to_string(),
            };
            self.push_display_message(DisplayMessage::system(help));
            self.pending_login = Some(pending);
            return;
        }

        match &pending {
            PendingLogin::OpenAiAccount { .. } if !looks_like_oauth_callback_input(trimmed) => {
                self.push_display_message(DisplayMessage::system(
                    "Still waiting for the browser callback. Paste the full callback URL or query string if you want to finish manually, or keep waiting for the automatic redirect.".to_string(),
                ));
                self.pending_login = Some(pending);
                return;
            }
            PendingLogin::Antigravity { .. } if !looks_like_oauth_callback_input(trimmed) => {
                self.push_display_message(DisplayMessage::system(
                    "Still waiting for the browser callback. Paste the full callback URL or query string if you want to finish manually, or keep waiting for the automatic redirect.".to_string(),
                ));
                self.pending_login = Some(pending);
                return;
            }
            _ => {}
        }

        match pending {
            PendingLogin::StartupGuide {
                focused,
                is_reminder,
            } => {
                match focused {
                    StartupGuideAction::LoginSaitec => {
                        self.start_jcode_login();
                    }
                    StartupGuideAction::SetupBaseModel => {
                        self.open_saitec_base_model_login_picker();
                    }
                    StartupGuideAction::SkipSaitec => {
                        self.push_display_message(DisplayMessage::system(
                            "SAITEC login skipped. You can log in anytime via `/login jcode`."
                                .to_string(),
                        ));
                        self.set_status_notice("SAITEC skipped");
                        // If this was the last thing preventing startup, let the
                        // branded startup surface collapse and show the conversation.
                        if is_reminder && self.display_user_message_count == 0 {
                            self.pending_login = None;
                        }
                    }
                }
            }
            PendingLogin::OpenAiCompatibleModelName {
                provider,
                provider_id,
                env_file,
                profile,
            } => {
                let model_name = input.trim().to_string();
                if !model_name.is_empty() {
                    // Save the model name so resolve_openai_compatible_profile picks it up
                    if let Err(err) = crate::provider_catalog::save_env_value_to_env_file(
                        "JCODE_OPENAI_COMPAT_DEFAULT_MODEL",
                        &env_file,
                        Some(&model_name),
                    ) {
                        crate::logging::warn(&format!(
                            "Failed to save model name for {}: {}",
                            provider, err
                        ));
                    }
                    crate::env::set_var("JCODE_OPENAI_COMPAT_DEFAULT_MODEL", &model_name);
                    crate::env::set_var("JCODE_OPENROUTER_MODEL", &model_name);
                }
                // Re-resolve the profile (env override for default_model is now active).
                // The profile env is already set from the key-save step — re-apply
                // is not needed here since we only need the resolved default_model
                // to be picked up by start_openai_compatible_post_login_activation.
                let _resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
                // apply_openai_compatible_profile_env is already called during the
                // key-save step; calling it again is a no-op for the env vars.
                crate::cli::provider_init::lock_model_provider("openrouter");
                self.start_openai_compatible_post_login_activation(provider);
            }
            PendingLogin::SaitecForm { mut form } => {
                if form.submitting {
                    self.stage_saitec_form(form);
                    return;
                }
                self.input = input;
                self.cursor_pos = self.input.len();
                self.handle_saitec_form_submit(form);
            }
            PendingLogin::ClaudeAccount {
                verifier,
                label,
                redirect_uri,
            } => {
                self.set_status_notice(format!("Login [{}]: exchanging...", label));
                let input_owned = input.clone();
                let label_clone = label.clone();
                tokio::spawn(async move {
                    match Self::claude_token_exchange(
                        verifier,
                        input_owned,
                        &label_clone,
                        redirect_uri,
                    )
                    .await
                    {
                        Ok(msg) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "claude".to_string(),
                                success: true,
                                message: msg,
                            }));
                        }
                        Err(e) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "claude".to_string(),
                                success: false,
                                message: format!("Claude login [{}] failed: {}", label_clone, e),
                            }));
                        }
                    }
                });
                self.push_display_message(DisplayMessage::system(format!(
                    "Exchanging authorization code for account `{}`...",
                    label
                )));
            }
            PendingLogin::OpenAiAccount {
                verifier,
                label,
                expected_state,
                redirect_uri,
            } => {
                self.set_status_notice(format!("Login [{}]: exchanging...", label));
                let input_owned = input.clone();
                let label_clone = label.clone();
                tokio::spawn(async move {
                    match Self::openai_token_exchange(
                        verifier,
                        input_owned,
                        Some(label_clone.clone()),
                        Some(expected_state),
                        &redirect_uri,
                    )
                    .await
                    {
                        Ok(msg) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "openai".to_string(),
                                success: true,
                                message: msg,
                            }));
                        }
                        Err(e) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "openai".to_string(),
                                success: false,
                                message: format!("OpenAI login [{}] failed: {}", label_clone, e),
                            }));
                        }
                    }
                });
                self.push_display_message(DisplayMessage::system(format!(
                    "Exchanging OpenAI callback for account `{}`...",
                    label
                )));
            }
            PendingLogin::Gemini {
                verifier,
                expected_state,
                redirect_uri,
            } => {
                self.set_status_notice("Login: exchanging...");
                let input_owned = input.clone();
                tokio::spawn(async move {
                    match crate::auth::gemini::exchange_callback_input(
                        &verifier,
                        input_owned.trim(),
                        expected_state.as_deref(),
                        &redirect_uri,
                    )
                    .await
                    {
                        Ok(tokens) => {
                            let msg = if let Some(email) = tokens.email {
                                format!("Successfully logged in to Gemini! (account: {})", email)
                            } else {
                                "Successfully logged in to Gemini!".to_string()
                            };
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "gemini".to_string(),
                                success: true,
                                message: msg,
                            }));
                        }
                        Err(e) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "gemini".to_string(),
                                success: false,
                                message: format!("Gemini login failed: {}", e),
                            }));
                        }
                    }
                });
                self.push_display_message(DisplayMessage::system(
                    "Exchanging Gemini callback for tokens...".to_string(),
                ));
            }
            PendingLogin::Antigravity {
                verifier,
                expected_state,
                redirect_uri,
            } => {
                self.set_status_notice("Login: exchanging...");
                let input_owned = input.clone();
                tokio::spawn(async move {
                    match Self::antigravity_token_exchange(
                        verifier,
                        input_owned,
                        Some(expected_state),
                        redirect_uri,
                    )
                    .await
                    {
                        Ok(msg) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "antigravity".to_string(),
                                success: true,
                                message: msg,
                            }));
                        }
                        Err(e) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "antigravity".to_string(),
                                success: false,
                                message: format!("Antigravity login failed: {}", e),
                            }));
                        }
                    }
                });
                self.push_display_message(DisplayMessage::system(
                    "Exchanging Antigravity callback for tokens...".to_string(),
                ));
            }
            PendingLogin::ApiKeyProfile {
                provider_id,
                provider,
                auth_method,
                docs_url,
                env_file,
                key_name,
                default_model,
                endpoint,
                api_key_optional,
                openai_compatible_profile,
            } => {
                let key = input.trim().to_string();
                if key.is_empty() && !api_key_optional {
                    self.push_display_message(DisplayMessage::error(
                        "API key cannot be empty.".to_string(),
                    ));
                    self.pending_login = Some(PendingLogin::ApiKeyProfile {
                        provider_id,
                        provider,
                        auth_method,
                        docs_url,
                        env_file,
                        key_name,
                        default_model,
                        endpoint,
                        api_key_optional,
                        openai_compatible_profile,
                    });
                    return;
                }
                if key_name == "OPENROUTER_API_KEY" && !key.starts_with("sk-or-") {
                    self.push_display_message(DisplayMessage::system(
                        "OpenRouter keys typically start with `sk-or-`. Saving anyway..."
                            .to_string(),
                    ));
                }

                let resolved_openai_compatible = openai_compatible_profile
                    .map(crate::provider_catalog::resolve_openai_compatible_profile);

                let save_result: anyhow::Result<()> =
                    if let Some(resolved) = resolved_openai_compatible.as_ref() {
                        (|| {
                            if resolved.requires_api_key {
                                crate::provider_catalog::save_env_value_to_env_file(
                                    crate::provider_catalog::OPENAI_COMPAT_LOCAL_ENABLED_ENV,
                                    &resolved.env_file,
                                    None,
                                )?;
                                crate::provider_catalog::save_env_value_to_env_file(
                                    &resolved.api_key_env,
                                    &resolved.env_file,
                                    Some(key.trim()),
                                )
                            } else {
                                crate::provider_catalog::save_env_value_to_env_file(
                                    crate::provider_catalog::OPENAI_COMPAT_LOCAL_ENABLED_ENV,
                                    &resolved.env_file,
                                    Some("1"),
                                )?;
                                crate::provider_catalog::save_env_value_to_env_file(
                                    &resolved.api_key_env,
                                    &resolved.env_file,
                                    if key.trim().is_empty() {
                                        None
                                    } else {
                                        Some(key.trim())
                                    },
                                )
                            }
                        })()
                    } else if key_name == crate::subscription_catalog::JCODE_API_KEY_ENV {
                        (|| {
                            let mut content = format!("{}={}\n", key_name, key);
                            if let Some(base) = crate::subscription_catalog::configured_api_base() {
                                content.push_str(&format!(
                                    "{}={}\n",
                                    crate::subscription_catalog::JCODE_API_BASE_ENV,
                                    base
                                ));
                            }

                            let config_dir = crate::storage::app_config_dir()?;
                            std::fs::create_dir_all(&config_dir)?;
                            crate::platform::set_directory_permissions_owner_only(&config_dir)?;

                            let file_path = config_dir.join(&env_file);
                            std::fs::write(&file_path, content)?;
                            crate::platform::set_permissions_owner_only(&file_path)?;
                            crate::env::set_var(&key_name, &key);
                            Ok(())
                        })()
                    } else if key_name == crate::provider::bedrock::API_KEY_ENV {
                        (|| {
                            Self::save_named_api_key(&env_file, &key_name, &key)?;
                            crate::provider_catalog::save_env_value_to_env_file(
                                crate::provider::bedrock::REGION_ENV,
                                &env_file,
                                Some("us-east-2"),
                            )
                        })()
                    } else {
                        Self::save_named_api_key(&env_file, &key_name, &key)
                    };

                match save_result {
                    Ok(()) => {
                        crate::auth::AuthStatus::invalidate_cache();
                        if key_name == crate::provider::bedrock::API_KEY_ENV {
                            crate::cli::provider_init::lock_model_provider("bedrock");
                            if let Some(default_model) = default_model.as_deref() {
                                crate::env::set_var("JCODE_BEDROCK_MODEL", default_model);
                            }
                        }

                        if let Some(profile) = openai_compatible_profile {
                            crate::provider_catalog::force_apply_openai_compatible_profile_env(
                                Some(profile),
                            );
                            crate::cli::provider_init::lock_model_provider("openrouter");
                            let effective_model = resolved_openai_compatible
                                .as_ref()
                                .and_then(|resolved| resolved.default_model.as_deref())
                                .or(default_model.as_deref());
                            if let Some(default_model) = effective_model {
                                crate::env::set_var("JCODE_OPENROUTER_MODEL", default_model);
                                self.start_openai_compatible_post_login_activation(
                                    provider.clone(),
                                );
                            } else {
                                // No default model — ask the user for a model name first.
                                let resolved = resolved_openai_compatible.clone();
                                self.pending_login =
                                    Some(PendingLogin::OpenAiCompatibleModelName {
                                        provider: provider.clone(),
                                        provider_id: resolved.as_ref().map_or_else(
                                            || profile.id.to_string(),
                                            |r| r.id.clone(),
                                        ),
                                        env_file: resolved.as_ref().map_or_else(
                                            || profile.env_file.to_string(),
                                            |r| r.env_file.clone(),
                                        ),
                                        profile,
                                    });
                                self.set_status_notice(format!(
                                    "{}: enter model name (optional)",
                                    provider
                                ));
                                return;
                            }
                        }

                        let effective_default_model = resolved_openai_compatible
                            .as_ref()
                            .and_then(|resolved| resolved.default_model.as_deref())
                            .or(default_model.as_deref());
                        let model_hint = effective_default_model
                            .map(|m| format!("\nSuggested default model: `{}`", m))
                            .unwrap_or_default();
                        let guidance = if key_name == crate::subscription_catalog::JCODE_API_KEY_ENV
                        {
                            format!(
                                "SAITEC credentials are saved for MCP permissions only. They do not configure or switch a base model. Use `/login base-models` to configure model providers.\nDocs: {}",
                                docs_url
                            )
                        } else if let Some(resolved) = resolved_openai_compatible.as_ref() {
                            if resolved.requires_api_key {
                                "Fetching models now. SAITEC-TUI will switch to an accessible model in the background. If you want to browse models afterward, open `/model`. If the model list looks stale, run `/refresh-model-list`.".to_string()
                            } else {
                                format!(
                                    "Local endpoint configured at `{}`. Fetching models now; SAITEC-TUI will switch to an accessible model in the background. If you want to browse models afterward, open `/model`. If the model list looks stale, run `/refresh-model-list`.",
                                    endpoint.as_deref().unwrap_or(resolved.api_base.as_str()),
                                )
                            }
                        } else if key_name == crate::provider::bedrock::API_KEY_ENV {
                            "You can now use `/model` to switch to Bedrock models. TUI onboarding saved region `us-east-2`; for a different region, run `jcode login --provider bedrock` from a terminal.".to_string()
                        } else if key_name == "OPENROUTER_API_KEY" {
                            "You can now use `/model` to switch to OpenRouter models. If the model list looks stale, run `/refresh-model-list`.".to_string()
                        } else {
                            "API key saved. Run `/refresh-model-list` to refresh model discovery, then use `/model` to pick an accessible model.".to_string()
                        };
                        let saved_label = if let Some(resolved) =
                            resolved_openai_compatible.as_ref()
                        {
                            if resolved.requires_api_key {
                                format!("{} API key saved", provider)
                            } else if key.trim().is_empty() {
                                format!("{} local endpoint saved", provider)
                            } else {
                                format!("{} local endpoint and optional API key saved", provider)
                            }
                        } else {
                            format!("{} API key saved", provider)
                        };
                        Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                            provider: provider.clone(),
                            success: true,
                            message: format!(
                                "**{}.**\n\n\
                                 Stored at `~/.config/jcode/{}`.\n\
                                 {}{}",
                                saved_label, env_file, guidance, model_hint
                            ),
                        }));
                        self.input.clear();
                        self.cursor_pos = 0;
                        self.clear_input_undo_history();
                        self.reset_tab_completion();
                        self.sync_model_picker_preview_from_input();
                    }
                    Err(e) => {
                        let reason = crate::auth::login_diagnostics::classify_auth_failure_message(
                            &e.to_string(),
                        );
                        crate::telemetry::record_auth_failed_reason(
                            &provider_id,
                            &auth_method,
                            reason.label(),
                        );
                        self.push_display_message(DisplayMessage::error(format!(
                            "Failed to save {} key: {}",
                            provider, e
                        )));
                        self.pending_login = Some(PendingLogin::ApiKeyProfile {
                            provider_id,
                            provider,
                            auth_method,
                            docs_url,
                            env_file,
                            key_name,
                            default_model,
                            endpoint,
                            api_key_optional,
                            openai_compatible_profile,
                        });
                    }
                }
            }
            PendingLogin::OpenAiCompatibleApiBase { profile } => {
                let api_base = input.trim();
                if !api_base.is_empty() {
                    let normalized = match crate::provider_catalog::normalize_api_base(api_base) {
                        Some(value) => value,
                        None => {
                            self.push_display_message(DisplayMessage::error(
                                "OpenAI-compatible API base must be https://... or http://localhost."
                                    .to_string(),
                            ));
                            self.pending_login =
                                Some(PendingLogin::OpenAiCompatibleApiBase { profile });
                            return;
                        }
                    };
                    if let Err(err) = crate::provider_catalog::save_env_value_to_env_file(
                        "JCODE_OPENAI_COMPAT_API_BASE",
                        crate::provider_catalog::OPENAI_COMPAT_PROFILE.env_file,
                        Some(&normalized),
                    ) {
                        self.push_display_message(DisplayMessage::error(format!(
                            "Failed to save OpenAI-compatible API base: {}",
                            err
                        )));
                        self.pending_login =
                            Some(PendingLogin::OpenAiCompatibleApiBase { profile });
                        return;
                    }
                }

                // Clear stale generic-override entries from the env file so that
                // resolve_openai_compatible_profile (called during the key-login
                // flow) does not pick up values left over from a previous provider
                // when the user switches directly via /login without logging out.
                let env_file = crate::provider_catalog::OPENAI_COMPAT_PROFILE.env_file;
                let _ = crate::provider_catalog::save_env_value_to_env_file(
                    "JCODE_OPENAI_COMPAT_API_KEY_NAME",
                    env_file,
                    None,
                );
                let _ = crate::provider_catalog::save_env_value_to_env_file(
                    "JCODE_OPENAI_COMPAT_ENV_FILE",
                    env_file,
                    None,
                );
                let _ = crate::provider_catalog::save_env_value_to_env_file(
                    "JCODE_OPENAI_COMPAT_DEFAULT_MODEL",
                    env_file,
                    None,
                );

                self.start_openai_compatible_key_login(profile);
            }
            PendingLogin::CursorApiKey => {
                let key = input.trim().to_string();
                if key.is_empty() {
                    self.push_display_message(DisplayMessage::error(
                        "API key cannot be empty.".to_string(),
                    ));
                    self.pending_login = Some(PendingLogin::CursorApiKey);
                    return;
                }

                match crate::auth::cursor::save_api_key(&key) {
                    Ok(()) => {
                        crate::auth::AuthStatus::invalidate_cache();
                        Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                            provider: "cursor".to_string(),
                            success: true,
                            message: "**Cursor API key saved.**\n\n\
                             Stored at `~/.config/jcode/cursor.env`.\n\
                             SAITEC-TUI will use it with the native Cursor HTTPS transport."
                                .to_string(),
                        }));
                    }
                    Err(e) => {
                        let reason = crate::auth::login_diagnostics::classify_auth_failure_message(
                            &e.to_string(),
                        );
                        crate::telemetry::record_auth_failed_reason(
                            "cursor",
                            "api_key",
                            reason.label(),
                        );
                        self.push_display_message(DisplayMessage::error(format!(
                            "Failed to save Cursor API key: {}",
                            e
                        )));
                        self.pending_login = Some(PendingLogin::CursorApiKey);
                    }
                }
            }
            PendingLogin::Copilot => {
                self.push_display_message(DisplayMessage::system(
                    "Copilot login is waiting for browser authorization.\n\
                     Complete the login in your browser, or type `/cancel` to abort."
                        .to_string(),
                ));
                self.pending_login = Some(PendingLogin::Copilot);
            }
            PendingLogin::AutoImportSelection { candidates } => {
                let selected = match crate::cli::provider_init::parse_external_auth_review_selection(
                    &input,
                    candidates.len(),
                ) {
                    Ok(selected) => selected,
                    Err(err) => {
                        self.push_display_message(DisplayMessage::error(err.to_string()));
                        self.pending_login = Some(PendingLogin::AutoImportSelection { candidates });
                        return;
                    }
                };

                self.set_status_notice("Login: importing approved sources...");
                tokio::spawn(async move {
                    match crate::cli::provider_init::run_external_auth_auto_import_candidates(
                        &candidates,
                        &selected,
                    )
                    .await
                    {
                        Ok(outcome) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "auto-import".to_string(),
                                success: outcome.imported > 0,
                                message: outcome.render_markdown(),
                            }));
                        }
                        Err(err) => {
                            Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                                provider: "auto-import".to_string(),
                                success: false,
                                message: format!("Auto import failed: {}", err),
                            }));
                        }
                    }
                });
            }
        }
    }

    fn trigger_provider_auth_changed(&self) {
        let provider = Arc::clone(&self.provider);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                provider.on_auth_changed();
            });
        } else {
            provider.on_auth_changed();
        }
    }

    fn start_openai_compatible_post_login_activation(&mut self, provider_label: String) {
        self.set_status_notice(format!("{}: fetching models...", provider_label));
        self.invalidate_model_picker_cache();

        // Make the newly saved OpenAI-compatible credentials usable in this
        // session immediately. The normal LoginCompleted path also calls this,
        // but doing it here lets the refresh task see the hot-added provider
        // without requiring a restart or a second user action.
        self.provider.on_auth_changed();

        if self.is_remote {
            if let Some(profile_id) =
                crate::provider_catalog::openai_compatible_profile_id_for_display_name(
                    &provider_label,
                )
                && let Some(profile) =
                    crate::provider_catalog::openai_compatible_profile_by_id(profile_id)
                && let Some(default_model) =
                    crate::provider_catalog::resolve_openai_compatible_profile(profile)
                        .default_model
            {
                self.pending_model_switch = Some(format!("{profile_id}:{default_model}"));
            }
            return;
        }

        if let Some((default_model, default_spec)) =
            Self::documented_openai_compatible_default_for_label(&provider_label)
            && let Err(error) = Self::set_provider_model_if_needed(
                self.provider.as_ref(),
                &default_model,
                &default_spec,
            )
        {
            crate::logging::warn(&format!(
                "Failed to preselect documented {} default `{}`: {}",
                provider_label, default_model, error
            ));
        }

        let provider = Arc::clone(&self.provider);
        let session_id = self.session.id.clone();
        let provider_descriptor = crate::provider_catalog::saitec_auth_status_login_providers()
            .into_iter()
            .find(|candidate| candidate.display_name == provider_label);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let result = provider.refresh_model_catalog().await;
                match result {
                    Ok(summary) => {
                        let routes = provider.model_routes();
                        let provider_profile_id =
                            crate::provider_catalog::openai_compatible_profile_id_for_display_name(
                                &provider_label,
                            );
                        let provider_api_method = provider_profile_id
                            .map(|profile_id| format!("openai-compatible:{profile_id}"));
                        let documented_default = Self::documented_openai_compatible_default_for_label(&provider_label);
                        let selected = routes
                            .iter()
                            .find(|route| {
                                route.available
                                    && route.api_method.starts_with("openai-compatible")
                                    && (route.provider == provider_label
                                        || provider_api_method.as_deref()
                                            == Some(route.api_method.as_str()))
                                    && documented_default
                                        .as_ref()
                                        .map(|(default_model, _)| {
                                            route.model == default_model.as_str()
                                        })
                                        .unwrap_or(true)
                                    && crate::provider::is_listable_model_name(&route.model)
                            })
                            .or_else(|| {
                                if documented_default.is_some() {
                                    return None;
                                }
                                routes.iter().find(|route| {
                                    route.available
                                        && route.provider == provider_label
                                        && crate::provider::is_listable_model_name(&route.model)
                                })
                            })
                            .or_else(|| {
                                if documented_default.is_some() {
                                    return None;
                                }
                                routes.iter().find(|route| {
                                    route.available
                                        && route.api_method.starts_with("openai-compatible")
                                        && crate::provider::is_listable_model_name(&route.model)
                                })
                            })
                            .map(|route| {
                                (
                                    route.model.clone(),
                                    Self::post_login_model_spec_for_route(route, &provider_label),
                                )
                            });

                        if let Some((model, model_spec)) = selected {
                            match Self::set_provider_model_if_needed(
                                provider.as_ref(),
                                &model,
                                &model_spec,
                            ) {
                                Ok(()) => {
                                    if let Some(provider_descriptor) = provider_descriptor
                                        && let Err(error) = crate::cli::auth_test::run_post_login_validation_quiet(provider_descriptor).await
                                    {
                                        publish_openai_compatible_validation_result(
                                            &provider_label,
                                            false,
                                            error.to_string(),
                                        );
                                        return;
                                    }
                                    crate::bus::Bus::global().publish_models_updated();
                                    crate::bus::Bus::global().publish(
                                        crate::bus::BusEvent::ProviderModelActivated {
                                            session_id,
                                            model: model.clone(),
                                            message: format!(
                                                "**{} is ready.**\n\nFetched model catalog: +{} models, +{} routes, ~{} changed.\nSwitched to `{}`.\n\nIf you want to browse other accessible models, open `/model`. If the model list ever looks stale, run `/refresh-model-list`.",
                                                provider_label,
                                                summary.models_added,
                                                summary.routes_added,
                                                summary.routes_changed,
                                                model
                                            ),
                                            open_picker: false,
                                        },
                                    );
                                }
                                Err(error) => {
                                    publish_openai_compatible_validation_result(
                                        &provider_label,
                                        false,
                                        format!(
                                            "Fetched models, but failed to switch to `{}`: {}\n\nYou can run `/refresh-model-list` to retry model discovery.",
                                            model, error
                                        ),
                                    );
                                }
                            }
                        } else if let Some((default_model, default_spec)) = documented_default
                        {
                            match Self::set_provider_model_if_needed(
                                provider.as_ref(),
                                &default_model,
                                &default_spec,
                            ) {
                                Ok(()) => {
                                    crate::bus::Bus::global().publish_models_updated();
                                    crate::bus::Bus::global().publish(
                                        crate::bus::BusEvent::ProviderModelActivated {
                                            session_id,
                                            model: default_model.clone(),
                                            message: format!(
                                                "**{} is ready.**\n\nThe live model catalog did not produce a selectable route yet, so SAITEC-TUI selected the documented default `{}`. Open `/model` if you want to inspect the current choices, or run `/refresh-model-list` later to retry live discovery.",
                                                provider_label,
                                                default_model
                                            ),
                                            open_picker: false,
                                        },
                                    );
                                }
                                Err(error) => {
                                    publish_openai_compatible_validation_result(
                                        &provider_label,
                                        false,
                                        format!(
                                            "Fetched the model catalog, but it contained no selectable {} models and failed to switch to the documented default `{}`: {}\n\nRun `/refresh-model-list` to retry model discovery, then `jcode auth status` and `jcode auth doctor` for a structured diagnosis.",
                                            provider_label,
                                            default_model,
                                            error
                                        ),
                                    );
                                }
                            }
                        } else {
                            publish_openai_compatible_validation_result(
                                &provider_label,
                                false,
                                format!(
                                    "Fetched the model catalog, but it contained no selectable {} models. Run `/refresh-model-list` to retry model discovery, then `jcode auth status` and `jcode auth doctor` for a structured diagnosis.",
                                    provider_label
                                ),
                            );
                        }
                    }
                    Err(error) => {
                        publish_openai_compatible_validation_result(
                            &provider_label,
                            false,
                            format!(
                                "Saved the API key, but failed to refresh the model catalog:\n\n{}\n\nRun `/refresh-model-list` to retry model discovery after checking the provider settings.",
                                error
                            ),
                        );
                    }
                }
            });
        }
    }

    fn documented_openai_compatible_default_for_label(
        provider_label: &str,
    ) -> Option<(String, String)> {
        let profile_id =
            crate::provider_catalog::openai_compatible_profile_id_for_display_name(provider_label)?;
        let profile = crate::provider_catalog::openai_compatible_profile_by_id(profile_id)?;
        let default_model =
            crate::provider_catalog::resolve_openai_compatible_profile(profile).default_model?;
        let default_spec = format!("{profile_id}:{default_model}");
        Some((default_model, default_spec))
    }

    fn set_provider_model_if_needed(
        provider: &dyn Provider,
        model: &str,
        model_spec: &str,
    ) -> Result<()> {
        let current = provider.model();
        if current == model || current == model_spec {
            return Ok(());
        }
        provider.set_model(model_spec)
    }

    fn post_login_model_spec_for_route(
        route: &crate::provider::ModelRoute,
        provider_label: &str,
    ) -> String {
        let model = route.model.trim();
        if model.is_empty() {
            return String::new();
        }

        if route.api_method == "openrouter" {
            if let Some(normalized) = crate::provider::openrouter_catalog_model_id(model) {
                if route.provider.eq_ignore_ascii_case("OpenAI") {
                    return format!("{normalized}@OpenAI");
                }
                return normalized;
            }
        }

        if let Some(profile_id) = route.api_method.strip_prefix("openai-compatible:") {
            let profile_id = profile_id.trim();
            if !profile_id.is_empty() {
                return format!("{profile_id}:{model}");
            }
        }

        if let Some(profile_id) =
            crate::provider_catalog::openai_compatible_profile_id_for_display_name(provider_label)
        {
            return format!("{profile_id}:{model}");
        }

        model.to_string()
    }

    pub(super) fn handle_login_completed(&mut self, login: LoginCompleted) {
        if login.provider == "copilot_code" {
            self.push_display_message(DisplayMessage::system(login.message.clone()));
            if let Some(code) = login
                .message
                .split("Enter code: **")
                .nth(1)
                .and_then(|s| s.split("**").next())
            {
                self.set_status_notice(format!("Login: enter {} at GitHub", code));
            }
            return;
        }
        crate::auth::AuthStatus::invalidate_cache();
        if let Some((provider, method)) = self
            .pending_login
            .as_ref()
            .and_then(PendingLogin::telemetry_context)
        {
            if login.success {
                crate::telemetry::record_auth_success(&provider, &method);
            } else {
                let reason =
                    crate::auth::login_diagnostics::classify_auth_failure_message(&login.message);
                crate::telemetry::record_auth_failed_reason(&provider, &method, reason.label());
            }
        }
        if login.success {
            self.recent_authenticated_provider = Some((login.provider.clone(), Instant::now()));
            self.invalidate_model_picker_cache();
            self.push_display_message(DisplayMessage::system(login.message));
            self.set_status_notice(format!("Login: {} ready", login.provider));
            if login.provider == "jcode" {
                crate::subscription_catalog::clear_runtime_env();
                // Reconnect SAITEC-Skills MCP with the newly saved API key
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let mcp_manager = Arc::clone(&self.mcp_manager);
                    let registry = self.registry.clone();
                    handle.spawn(async move {
                        // 1. Pool 层：重建连接（新 transport + 新 API key）
                        crate::saitec::mcp::reconnect_saitec_mcp().await;

                        // 2. Manager 层：从 pool 重新获取 handle
                        let mgr = mcp_manager.read().await;
                        mgr.reacquire_pool_handle("SAITEC-Skills").await;
                        drop(mgr);

                        // 3. Registry 层：重新注册 SAITEC-Skills 工具
                        let tools = crate::mcp::create_mcp_tools(mcp_manager).await;
                        for (name, tool) in tools {
                            if name.starts_with("mcp__SAITEC-Skills__") {
                                registry.register(name, tool).await;
                            }
                        }
                    });
                }
            } else {
                self.trigger_provider_auth_changed();
            }
            if self.pending_login.is_some() {
                self.pending_login = None;
            }
            // After a successful non-jcode login, if SAITEC is still not configured
            // and we are still in startup state, transition to the Reminder guide.
            if login.provider != "jcode"
                && self.display_user_message_count == 0
                && self.streaming_text.is_empty()
                && self.pending_login.is_none()
                && crate::saitec::auth::ensure_logged_in().is_err()
            {
                self.begin_pending_login(PendingLogin::StartupGuide {
                    focused: StartupGuideAction::LoginSaitec,
                    is_reminder: true,
                });
            }
        } else {
            let message = crate::auth::login_diagnostics::augment_auth_error_message(
                &login.provider,
                &login.message,
            );
            if login.provider == "jcode" {
                match self.pending_login.take() {
                    Some(PendingLogin::SaitecForm { mut form }) => {
                        form.submitting = false;
                        form.focus = if form.form.password.trim().is_empty() {
                            if form.form.email.trim().is_empty()
                                && form.form.phone.trim().is_empty()
                            {
                                SaitecLoginField::Email
                            } else {
                                SaitecLoginField::Password
                            }
                        } else {
                            SaitecLoginField::Password
                        };
                        form.error = Some(message);
                        self.set_status_notice("Login: SAITEC-TUI failed");
                        self.stage_saitec_form(form);
                        self.sync_input_with_pending_saitec_form();
                    }
                    other => {
                        self.pending_login = other;
                        self.push_display_message(DisplayMessage::error(message));
                        self.set_status_notice(format!("Login: {} failed", login.provider));
                    }
                }
            } else if let Some(provider_descriptor) =
                resolve_openai_compatible_provider_descriptor(&login.provider)
            {
                self.push_display_message(DisplayMessage::error(message));
                if is_openai_compatible_post_save_failure_message(&login.message) {
                    self.set_status_notice(format!(
                        "Validation: {} failed",
                        provider_descriptor.display_name
                    ));
                } else {
                    self.set_status_notice(format!("Login: {} failed", login.provider));
                    self.start_login_provider(provider_descriptor);
                }
            } else {
                self.push_display_message(DisplayMessage::error(message));
                self.set_status_notice(format!("Login: {} failed", login.provider));
                if self.pending_login.is_some() {
                    self.pending_login = None;
                }
            }
        }
    }

    pub(crate) fn handle_provider_validation_completed(
        &mut self,
        event: crate::bus::ProviderValidationCompleted,
    ) {
        if self.login_picker_overlay.is_some() {
            self.refresh_open_saitec_base_model_login_picker();
        }
        if event.success {
            self.push_display_message(DisplayMessage::system(event.message));
            self.set_status_notice(format!("Validation: {} ready", event.provider_display_name));
        } else {
            self.push_display_message(DisplayMessage::error(event.message));
            self.set_status_notice(format!(
                "Validation: {} failed",
                event.provider_display_name
            ));
        }
    }

    pub(in crate::tui::app) fn mark_successful_remote_turn_runtime_validated(&mut self) {
        let Some(target) = self.successful_remote_openai_compatible_validation_target() else {
            return;
        };

        let existing = crate::auth::validation::get(&target.provider_id);
        let already_validated = existing.as_ref().is_some_and(|record| {
            record.success
                && record.provider_smoke_ok == Some(true)
                && record
                    .validated_models
                    .iter()
                    .any(|model| models_match(model, &target.model))
        });
        if already_validated {
            return;
        }

        let mut validated_models = existing
            .as_ref()
            .filter(|record| record.success)
            .map(|record| record.validated_models.clone())
            .unwrap_or_default();
        if !validated_models
            .iter()
            .any(|model| models_match(model, &target.model))
        {
            validated_models.push(target.model.clone());
        }

        let record = crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: existing.as_ref().and_then(|record| record.tool_smoke_ok),
            validated_models,
            summary: format!(
                "Validated by successful remote turn using `{}`.",
                target.model
            ),
        };

        match crate::auth::validation::save(&target.provider_id, record) {
            Ok(()) => {
                crate::auth::AuthStatus::invalidate_cache();
                self.invalidate_model_picker_cache();
                if self.login_picker_overlay.is_some() {
                    self.refresh_open_saitec_base_model_login_picker();
                }
                self.set_status_notice(format!(
                    "Validation: {} ready",
                    target.provider_display_name
                ));
                crate::bus::Bus::global().publish_models_updated();
            }
            Err(error) => {
                crate::logging::warn(&format!(
                    "Failed to persist successful remote validation for {}: {}",
                    target.provider_id, error
                ));
            }
        }
    }

    fn successful_remote_openai_compatible_validation_target(
        &self,
    ) -> Option<SuccessfulRemoteValidationTarget> {
        let raw_model = self
            .remote_provider_model
            .as_deref()
            .or(self.session.model.as_deref())?
            .trim();
        if raw_model.is_empty() {
            return None;
        }

        let explicit_profile = raw_model.split_once(':').and_then(|(prefix, model)| {
            let prefix = prefix.trim();
            let model = model.trim();
            if model.is_empty() {
                return None;
            }
            crate::provider_catalog::openai_compatible_profile_by_id(prefix)
                .map(|profile| (profile, model.to_string()))
        });

        let (profile, model) = if let Some((profile, model)) = explicit_profile {
            (profile, model)
        } else if let Some(route) = self
            .remote_model_options
            .iter()
            .find(|route| {
                models_match(&route.model, raw_model)
                    && route.api_method.starts_with("openai-compatible:")
            })
            .cloned()
            .or_else(|| App::remote_openai_compatible_route_for_model(raw_model))
        {
            let provider_id = route.api_method.strip_prefix("openai-compatible:")?.trim();
            let profile = crate::provider_catalog::openai_compatible_profile_by_id(provider_id)?;
            (profile, route.model)
        } else {
            return None;
        };

        if !crate::provider_catalog::openai_compatible_profile_is_configured(profile) {
            return None;
        }

        let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
        if !resolved.requires_api_key {
            return None;
        }

        Some(SuccessfulRemoteValidationTarget {
            provider_id: resolved.id,
            provider_display_name: resolved.display_name,
            model,
        })
    }

    pub(super) fn handle_update_status(&mut self, status: crate::bus::UpdateStatus) {
        use crate::bus::UpdateStatus;
        crate::logging::info(&format!(
            "[tui-update] handle_update_status received: {:?}",
            match &status {
                UpdateStatus::Available { latest, .. } => format!("Available(v={})", latest),
                UpdateStatus::Downloading { version } => format!("Downloading({})", version),
                UpdateStatus::DownloadProgress { version, downloaded, total } => {
                    format!("DownloadProgress({}/{})", downloaded, total)
                }
                UpdateStatus::Downloaded { version, path } => {
                    format!("Downloaded(v={}, path={})", version, path.display())
                }
                UpdateStatus::Checking => "Checking".to_string(),
                UpdateStatus::UpToDate => "UpToDate".to_string(),
                UpdateStatus::Installed { version } => format!("Installed({})", version),
                UpdateStatus::Error(e) => format!("Error({})", e),
            }
        ));
        match status {
            UpdateStatus::Checking => {
                self.set_status_notice("Checking for updates...");
            }
            UpdateStatus::Available {
                current,
                latest,
                payload,
            } => {
                crate::logging::info(&format!(
                    "[tui-update] setting pending_tui_update: latest={}, size={}MB",
                    latest,
                    payload.size_bytes / 1_000_000,
                ));
                // 写入 App 字段 + 全局静态（兜底 &dyn TuiState trait dispatch）。
                self.pending_tui_update = Some(payload.clone());
                crate::saitec::tui_update::set_global_pending_update(payload.clone());
                let size_mb = payload.size_bytes / 1_000_000;
                self.set_status_notice(format!(
                    "🆕 SAITEC-TUI v{} available ({} MB) — press U to download",
                    latest, size_mb
                ));
                let _ = (current, latest);
            }
            UpdateStatus::Downloading { version } => {
                // 下载任务启动：publish 进度初始值。
                self.tui_update_progress = Some(super::TuiUpdateProgress {
                    version: version.clone(),
                    downloaded: 0,
                    total: self
                        .pending_tui_update
                        .as_ref()
                        .map(|p| p.size_bytes)
                        .unwrap_or(0),
                });
                // 保留 pending_tui_update（用来取 download_url/fallback size_bytes 等）。
                self.set_status_notice(format!("⬇️  Downloading {} ...", version));
            }
            UpdateStatus::DownloadProgress {
                version,
                downloaded,
                total,
            } => {
                // 后端可能没有 Content-Length（chunked），total=0。保留 Downloading init
                // 时从 payload.size_bytes 设的正确 total，避免被 0 覆盖。
                let prev_total = self.tui_update_progress.as_ref().map(|p| p.total).unwrap_or(0);
                let merged_total = if total > 0 {
                    total
                } else {
                    prev_total.max(
                        self.pending_tui_update
                            .as_ref()
                            .map(|p| p.size_bytes)
                            .unwrap_or(0),
                    )
                };
                self.tui_update_progress = Some(super::TuiUpdateProgress {
                    version,
                    downloaded,
                    total: merged_total,
                });
                // 不写 notice —— 避免每 ~256KB 进度回调都覆盖状态栏。
            }
            UpdateStatus::Downloaded { version, path } => {
                // 清理下载任务 + 释放 pending payload（banner 不再渲染）。
                self.tui_update_progress = None;
                self.tui_update_download_cancel = None;
                self.pending_tui_update = None;
                crate::saitec::tui_update::clear_global_pending_update();
                self.set_status_notice(format!(
                    "✅ SAITEC-TUI v{} downloaded to {}\n   Exit TUI and run that exe to install.",
                    version,
                    path.display()
                ));
            }
            UpdateStatus::Installed { version } => {
                self.set_status_notice(format!("✅ Updated to {} — restarting", version));
            }
            UpdateStatus::UpToDate => {}
            UpdateStatus::Error(e) => {
                // 清进度 & cancel handle，但保留 pending payload 让用户可重试。
                self.tui_update_progress = None;
                self.tui_update_download_cancel = None;
                self.set_status_notice(format!("❌ Update failed: {}. Type /download-latest to retry.", e));
            }
        }
    }

    /// TUI banner 上按 [U] 触发：把 pending payload 取出，async spawn 下载任务并 publish 进度事件。
    /// 调用方先在 input handler 检查 `pending_tui_update.is_some()`。
    /// Returns `true` if a download was kicked off (用于测试与 debug 日志)。
    pub(super) fn start_tui_update_download(&mut self) -> bool {
        let Some(payload) = self.pending_tui_update.clone() else {
            return false;
        };

        // 未登录：直接报错，留在 pending 状态等用户登录。
        let api_key = crate::saitec::tui_update::current_api_key();
        let Some(api_key) = api_key else {
            self.set_status_notice(
                "⚠️  Cannot download: not logged in. Run /login (or /login jcode) first.",
            );
            return false;
        };

        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        self.tui_update_download_cancel = Some(cancel_tx);

        let version = payload.latest_version.clone();
        let download_url = payload.download_url.clone();

        // 先 publish Downloading 让 banner 进入 progress 模式。
        crate::bus::Bus::global().publish(crate::bus::BusEvent::UpdateStatus(
            crate::bus::UpdateStatus::Downloading {
                version: version.clone(),
            },
        ));

        tokio::spawn(async move {
            let dest = match crate::saitec::tui_update::available_dest_path(&version) {
                Ok(p) => p,
                Err(e) => {
                    crate::bus::Bus::global().publish(crate::bus::BusEvent::UpdateStatus(
                        crate::bus::UpdateStatus::Error(format!(
                            "cannot resolve destination: {}",
                            e
                        )),
                    ));
                    return;
                }
            };

            let url_for_closure = download_url.clone();
            let version_for_progress = version.clone();

            let result = crate::saitec::tui_update::download_tui_update(
                &url_for_closure,
                &dest,
                Some(&api_key),
                move |downloaded, total| {
                    crate::bus::Bus::global().publish(crate::bus::BusEvent::UpdateStatus(
                        crate::bus::UpdateStatus::DownloadProgress {
                            version: version_for_progress.clone(),
                            downloaded,
                            total,
                        },
                    ));
                },
                cancel_rx,
            )
            .await;

            match result {
                Ok(path) => {
                    crate::bus::Bus::global().publish(crate::bus::BusEvent::UpdateStatus(
                        crate::bus::UpdateStatus::Downloaded { version, path },
                    ));
                }
                Err(e) => {
                    crate::bus::Bus::global().publish(crate::bus::BusEvent::UpdateStatus(
                        crate::bus::UpdateStatus::Error(format!("download: {}", e)),
                    ));
                }
            }
        });

        true
    }

    /// Esc in-flight: cancel 当前下载任务。`None` 表示无活跃任务。
    /// 同步清 `tui_update_progress` + 全局静态避免 stale UI（async Error 事件到达前的窗口）。
    pub(super) fn cancel_tui_update_download(&mut self) {
        if let Some(tx) = self.tui_update_download_cancel.take() {
            // notify download loop to bail；receiver 在 spawn 内 take，关闭后 task 退出。
            let _ = tx.send(true);
            // 同步清理进度（avoid stale UI），但保留 pending payload 让用户可重试。
            self.tui_update_progress = None;
            self.set_status_notice("❌ Update cancelled. Type /download-latest to retry.");
        }
    }

    async fn claude_token_exchange(
        verifier: String,
        input: String,
        label: &str,
        redirect_uri: Option<String>,
    ) -> Result<String, String> {
        let fallback_redirect_uri =
            redirect_uri.unwrap_or_else(|| crate::auth::oauth::claude::REDIRECT_URI.to_string());
        let redirect_uri =
            crate::auth::oauth::claude_redirect_uri_for_input(input.trim(), &fallback_redirect_uri);
        let oauth_tokens =
            crate::auth::oauth::exchange_claude_code(&verifier, input.trim(), &redirect_uri)
                .await
                .map_err(|e| e.to_string())?;

        crate::auth::oauth::save_claude_tokens_for_account(&oauth_tokens, label)
            .map_err(|e| format!("Failed to save tokens: {}", e))?;

        let profile_suffix = match crate::auth::oauth::update_claude_account_profile(
            label,
            &oauth_tokens.access_token,
        )
        .await
        {
            Ok(Some(email)) => format!(" (email: {})", mask_email(&email)),
            Ok(None) => String::new(),
            Err(e) => {
                crate::logging::warn(&format!(
                    "Claude login [{}] profile fetch failed: {}",
                    label, e
                ));
                String::new()
            }
        };

        Ok(format!(
            "Successfully logged in to Claude! (account: {}){}",
            label, profile_suffix
        ))
    }

    fn save_named_api_key(env_file: &str, key_name: &str, key: &str) -> anyhow::Result<()> {
        if !crate::provider_catalog::is_safe_env_key_name(key_name) {
            anyhow::bail!("Invalid API key variable name: {}", key_name);
        }
        if !crate::provider_catalog::is_safe_env_file_name(env_file) {
            anyhow::bail!("Invalid env file name: {}", env_file);
        }

        let config_dir = crate::storage::app_config_dir()?;
        let file_path = config_dir.join(env_file);
        crate::storage::upsert_env_file_value(&file_path, key_name, Some(key))?;
        crate::env::set_var(key_name, key);
        Ok(())
    }
}

#[cfg(test)]
fn save_tui_openai_compatible_api_base(
    api_base: &str,
) -> anyhow::Result<crate::provider_catalog::ResolvedOpenAiCompatibleProfile> {
    let trimmed = api_base.trim();
    if !trimmed.is_empty() {
        let normalized = crate::provider_catalog::normalize_api_base(trimmed).ok_or_else(|| {
            anyhow::anyhow!("OpenAI-compatible API base must be https://... or http://localhost.")
        })?;
        crate::provider_catalog::save_env_value_to_env_file(
            "JCODE_OPENAI_COMPAT_API_BASE",
            crate::provider_catalog::OPENAI_COMPAT_PROFILE.env_file,
            Some(&normalized),
        )?;
    }
    Ok(crate::provider_catalog::resolve_openai_compatible_profile(
        crate::provider_catalog::OPENAI_COMPAT_PROFILE,
    ))
}

#[cfg(test)]
fn save_tui_openai_compatible_key(
    profile: crate::provider_catalog::OpenAiCompatibleProfile,
    key: &str,
) -> anyhow::Result<crate::provider_catalog::ResolvedOpenAiCompatibleProfile> {
    let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
    if resolved.requires_api_key {
        crate::provider_catalog::save_env_value_to_env_file(
            crate::provider_catalog::OPENAI_COMPAT_LOCAL_ENABLED_ENV,
            &resolved.env_file,
            None,
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            &resolved.api_key_env,
            &resolved.env_file,
            Some(key.trim()),
        )?;
    } else {
        crate::provider_catalog::save_env_value_to_env_file(
            crate::provider_catalog::OPENAI_COMPAT_LOCAL_ENABLED_ENV,
            &resolved.env_file,
            Some("1"),
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            &resolved.api_key_env,
            &resolved.env_file,
            if key.trim().is_empty() {
                None
            } else {
                Some(key.trim())
            },
        )?;
    }
    Ok(resolved)
}

fn looks_like_oauth_callback_input(input: &str) -> bool {
    let input = input.trim();
    input.starts_with("http://")
        || input.starts_with("https://")
        || input.starts_with('?')
        || input.contains("code=")
        || input.contains("state=")
}

fn looks_like_saitec_callback_input(input: &str) -> bool {
    let input = input.trim();
    input.starts_with("http://")
        || input.starts_with("https://")
        || input.starts_with('?')
        || input.contains("auth_token=")
}

fn antigravity_input_requires_state_validation(input: &str, expected_state: Option<&str>) -> bool {
    expected_state.is_some() && looks_like_oauth_callback_input(input)
}

fn is_openai_compatible_post_save_failure_message(message: &str) -> bool {
    let lower = message.trim().to_ascii_lowercase();
    lower.contains("credentials were saved")
        || lower.contains("saved the api key")
        || lower.contains("fetched the model catalog")
        || lower.contains("fetched models, but failed to switch")
}

fn resolve_openai_compatible_provider_descriptor(
    provider_label: &str,
) -> Option<crate::provider_catalog::LoginProviderDescriptor> {
    crate::provider_catalog::resolve_login_provider(provider_label)
        .filter(|provider| {
            matches!(
                provider.target,
                crate::provider_catalog::LoginProviderTarget::OpenAiCompatible(_)
            )
        })
        .or_else(|| {
            crate::provider_catalog::saitec_auth_status_login_providers()
                .into_iter()
                .find(|provider| {
                    matches!(
                        provider.target,
                        crate::provider_catalog::LoginProviderTarget::OpenAiCompatible(_)
                    ) && provider
                        .display_name
                        .eq_ignore_ascii_case(provider_label.trim())
                })
        })
}

fn publish_openai_compatible_validation_result(
    provider_label: &str,
    success: bool,
    message: String,
) {
    if let Some(provider_descriptor) = resolve_openai_compatible_provider_descriptor(provider_label)
    {
        crate::bus::Bus::global().publish(crate::bus::BusEvent::ProviderValidationCompleted(
            crate::bus::ProviderValidationCompleted {
                provider: provider_descriptor.id.to_string(),
                provider_display_name: provider_descriptor.display_name.to_string(),
                success,
                message,
            },
        ));
    } else {
        crate::bus::Bus::global().publish(crate::bus::BusEvent::LoginCompleted(
            crate::bus::LoginCompleted {
                provider: provider_label.to_string(),
                success,
                message,
            },
        ));
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
