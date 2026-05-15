use super::box_utils::render_rounded_box;
use super::changelog::get_unseen_changelog_entries;
use super::{
    TuiState, binary_age, dim_color, header_name_color, header_session_color,
    is_running_stable_release, semver, shorten_model_name,
};
use crate::auth::{AuthState, AuthStatus};
use crate::tui::color_support::rgb;
use crate::tui::connection_type_icon;
use ratatui::prelude::*;
#[cfg(test)]
use std::sync::OnceLock;
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
fn unseen_changelog_entries_override() -> &'static std::sync::Mutex<Option<Vec<String>>> {
    static OVERRIDE: OnceLock<std::sync::Mutex<Option<Vec<String>>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| std::sync::Mutex::new(None))
}

fn unseen_changelog_entries() -> Vec<String> {
    #[cfg(test)]
    {
        if let Ok(guard) = unseen_changelog_entries_override().lock()
            && let Some(entries) = guard.clone()
        {
            return entries;
        }
    }
    get_unseen_changelog_entries().clone()
}

#[cfg(test)]
pub(crate) fn set_unseen_changelog_entries_override_for_tests(entries: Option<Vec<String>>) {
    let mut guard = unseen_changelog_entries_override()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = entries;
}

pub(crate) fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

fn format_model_name(short: &str) -> String {
    if short.contains('/') {
        return format!("OpenRouter: {}", short);
    }
    if short.contains("opus") {
        if short.contains("4.5") {
            return "Claude 4.5 Opus".to_string();
        }
        return "Claude Opus".to_string();
    }
    if short.contains("sonnet") {
        if short.contains("3.5") {
            return "Claude 3.5 Sonnet".to_string();
        }
        return "Claude Sonnet".to_string();
    }
    if short.contains("haiku") {
        return "Claude Haiku".to_string();
    }
    if short.starts_with("gpt") {
        return format_gpt_name(short);
    }
    short.to_string()
}

fn format_gpt_name(short: &str) -> String {
    let rest = short.trim_start_matches("gpt");
    if rest.is_empty() {
        return "GPT".to_string();
    }

    if let Some(idx) = rest.find("codex") {
        let version = &rest[..idx];
        if version.is_empty() {
            return "GPT Codex".to_string();
        }
        return format!("GPT-{} Codex", version);
    }

    format!("GPT-{}", rest)
}

pub(super) fn build_auth_status_line(auth: &AuthStatus, max_width: usize) -> Line<'static> {
    fn dot_color(state: AuthState) -> Color {
        match state {
            AuthState::Available => rgb(100, 200, 100),
            AuthState::Expired => rgb(255, 200, 100),
            AuthState::NotConfigured => rgb(80, 80, 80),
        }
    }

    fn dot_char(state: AuthState) -> &'static str {
        match state {
            AuthState::Available => "●",
            AuthState::Expired => "◐",
            AuthState::NotConfigured => "○",
        }
    }

    fn rendered_width(entries: &[&str]) -> usize {
        if entries.is_empty() {
            return 0;
        }

        entries.iter().map(|label| label.len() + 3).sum::<usize>() + (entries.len() - 1)
    }

    fn provider_label(name: &str, state: AuthState, method: Option<&str>) -> String {
        match (state, method) {
            (AuthState::NotConfigured, _) => name.to_string(),
            (_, Some(method)) if !method.is_empty() => format!("{}({})", name, method),
            _ => name.to_string(),
        }
    }

    let anthropic_label = if auth.anthropic.has_oauth && auth.anthropic.has_api_key {
        provider_label("anthropic", auth.anthropic.state, Some("oauth+key"))
    } else if auth.anthropic.has_oauth {
        provider_label("anthropic", auth.anthropic.state, Some("oauth"))
    } else if auth.anthropic.has_api_key {
        provider_label("anthropic", auth.anthropic.state, Some("key"))
    } else {
        provider_label("anthropic", auth.anthropic.state, None)
    };

    let openai_label = if auth.openai_has_oauth && auth.openai_has_api_key {
        provider_label("openai", auth.openai, Some("oauth+key"))
    } else if auth.openai_has_oauth {
        provider_label("openai", auth.openai, Some("oauth"))
    } else if auth.openai_has_api_key {
        provider_label("openai", auth.openai, Some("key"))
    } else {
        provider_label("openai", auth.openai, None)
    };

    let gemini_label = if auth.gemini != AuthState::NotConfigured {
        provider_label("gemini", auth.gemini, Some("oauth"))
    } else {
        provider_label("gemini", auth.gemini, None)
    };

    let gemini_compact_label = if auth.gemini != AuthState::NotConfigured {
        provider_label("ge", auth.gemini, Some("oauth"))
    } else {
        provider_label("ge", auth.gemini, None)
    };

    let full_specs: Vec<(String, AuthState)> = vec![
        (anthropic_label, auth.anthropic.state),
        ("openrouter".to_string(), auth.openrouter),
        (openai_label, auth.openai),
        (provider_label("cursor", auth.cursor, None), auth.cursor),
        (provider_label("copilot", auth.copilot, None), auth.copilot),
        (gemini_label, auth.gemini),
        (
            provider_label("antigravity", auth.antigravity, None),
            auth.antigravity,
        ),
    ]
    .into_iter()
    .filter(|(_, state)| *state != AuthState::NotConfigured)
    .collect();

    let compact_specs: Vec<(String, AuthState)> = vec![
        (
            provider_label("an", auth.anthropic.state, None),
            auth.anthropic.state,
        ),
        ("or".to_string(), auth.openrouter),
        (provider_label("oa", auth.openai, None), auth.openai),
        (provider_label("cu", auth.cursor, None), auth.cursor),
        (provider_label("cp", auth.copilot, None), auth.copilot),
        (gemini_compact_label, auth.gemini),
        (
            provider_label("ag", auth.antigravity, None),
            auth.antigravity,
        ),
    ]
    .into_iter()
    .filter(|(_, state)| *state != AuthState::NotConfigured)
    .collect();

    let full: Vec<&str> = full_specs.iter().map(|(label, _)| label.as_str()).collect();
    let compact: Vec<&str> = compact_specs
        .iter()
        .map(|(label, _)| label.as_str())
        .collect();

    let provider_specs: Vec<&(String, AuthState)> = if rendered_width(&full) <= max_width {
        full_specs.iter().collect()
    } else if rendered_width(&compact) <= max_width {
        compact_specs.iter().collect()
    } else {
        compact_specs.iter().take(4).collect()
    };

    let mut spans = Vec::new();
    for (i, (label, state)) in provider_specs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ", Style::default().fg(dim_color())));
        }

        spans.push(Span::styled(
            dot_char(*state),
            Style::default().fg(dot_color(*state)),
        ));
        spans.push(Span::styled(
            format!(" {} ", label),
            Style::default().fg(dim_color()),
        ));
    }

    Line::from(spans)
}

fn header_provider_auth_tag(name: &str, auth: &AuthStatus) -> &'static str {
    match name {
        "anthropic" => {
            if auth.anthropic.has_oauth {
                "oauth"
            } else if std::env::var("ANTHROPIC_API_KEY").is_ok() || auth.anthropic.has_api_key {
                "api-key"
            } else {
                ""
            }
        }
        "openai" => match (auth.openai_has_oauth, auth.openai_has_api_key) {
            (true, true) => "oauth+key",
            (true, false) => "oauth",
            (false, true) => "api-key",
            (false, false) => "",
        },
        "copilot" => {
            if auth.copilot_has_api_token {
                "oauth"
            } else {
                ""
            }
        }
        "openrouter" | "openai-compatible" => "api-key",
        other
            if crate::provider_catalog::resolve_openai_compatible_profile_selection(other)
                .is_some()
                || crate::provider_catalog::openai_compatible_profile_id_for_display_name(
                    other,
                )
                .is_some() =>
        {
            "api-key"
        }
        _ => "",
    }
}

pub(crate) fn abbreviate_home(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        if path == home_str {
            return "~".to_string();
        }
        if let Some(rest) = path.strip_prefix(&home_str) {
            return format!("~{}", rest);
        }
    }
    path.to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LogoFrameOptions {
    animated: bool,
    startup_surface: bool,
}

impl LogoFrameOptions {
    const fn static_startup() -> Self {
        Self {
            animated: false,
            startup_surface: true,
        }
    }

    fn animated_startup() -> Self {
        Self {
            animated: brand_animation_enabled(),
            startup_surface: true,
        }
    }
}

pub(crate) fn brand_animation_enabled() -> bool {
    crate::perf::tui_policy().enable_decorative_animations
        || crate::saitec::product_profile::use_fixed_shell_layout()
}

fn startup_logo_text_lines(width: usize) -> Vec<String> {
    const B: &str = "\u{2588}\u{2588}";
    const S: &str = "  ";
    let full = vec![
        format!("{S}{B}{B}{B}{S}{S}{B}{B}{S}{S}{B}{S}{B}{B}{B}{S}{B}{B}{B}{S}{B}{B}{B}{S}"),
        format!("{S}{B}{S}{S}{S}{B}{S}{S}{B}{S}{B}{S}{S}{B}{S}{S}{B}{S}{S}{S}{B}{S}{S}{S}"),
        format!("{S}{B}{B}{B}{S}{B}{B}{B}{B}{S}{B}{S}{S}{B}{S}{S}{B}{B}{S}{S}{B}{S}{S}{S}"),
        format!("{S}{S}{S}{B}{S}{B}{S}{S}{B}{S}{B}{S}{S}{B}{S}{S}{B}{S}{S}{S}{B}{S}{S}{S}"),
        format!("{S}{B}{B}{B}{S}{B}{S}{S}{B}{S}{B}{S}{S}{B}{S}{S}{B}{B}{B}{S}{B}{B}{B}{S}"),
    ];
    let full_width = full
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default();
    if width >= full_width {
        return full;
    }

    let compact = vec![
        format!("{B}{B}{B}  {B}{B}   {B}{B}{B} {B}{B}{B} {B}{B}{B}  {B}{B}{B}"),
        format!("{B}       {B}   {B}    {B}       {B}    {B}        {B}"),
        format!("{B}{B}{B} {B}{B}{B}    {B}       {B}    {B}{B}     {B}"),
    ];
    let compact_width = compact
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default();
    if width >= compact_width {
        return compact;
    }

    vec!["SAITEC".to_string()]
}

pub(crate) fn startup_logo_lines(width: usize) -> Vec<String> {
    startup_logo_text_lines(width)
}

fn pulse_mix(base: u8, peak: u8, progress: f32) -> u8 {
    let progress = progress.clamp(0.0, 1.0);
    let delta = peak as f32 - base as f32;
    (base as f32 + delta * progress).round().clamp(0.0, 255.0) as u8
}

fn logo_block_style(row: usize, col: usize, elapsed: f32, animated: bool) -> Style {
    if !animated {
        return Style::default().fg(header_name_color());
    }

    let phase = elapsed * 0.9 + row as f32 * 0.55 + col as f32 * 0.18;
    let wave = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let shimmer = ((elapsed * 0.45 + row as f32 * 0.21 - col as f32 * 0.08).cos() * 0.5 + 0.5)
        .clamp(0.0, 1.0);

    Style::default().fg(rgb(
        pulse_mix(132, 205, wave),
        pulse_mix(88, 165, shimmer),
        pulse_mix(74, 138, wave * 0.65 + shimmer * 0.35),
    ))
}

fn logo_block_glyph(
    row: usize,
    col: usize,
    elapsed: f32,
    animated: bool,
    edge: bool,
) -> &'static str {
    if !animated || edge {
        return "█";
    }

    let wave = elapsed * 0.85 + row as f32 * 0.63 + col as f32 * 0.22;
    let alt = elapsed * 0.35 - row as f32 * 0.17 + col as f32 * 0.11;
    let density = wave.sin() * 0.72 + alt.cos() * 0.28;

    if density > 0.58 {
        "█"
    } else if density > -0.12 {
        "▓"
    } else {
        "▒"
    }
}

fn logo_block_positions(line: &str) -> Vec<usize> {
    line.char_indices()
        .filter_map(|(idx, ch)| (ch == '█').then_some(idx))
        .collect()
}

fn animated_startup_logo_lines_for(
    width: usize,
    elapsed: f32,
    options: LogoFrameOptions,
) -> Vec<Line<'static>> {
    startup_logo_text_lines(width)
        .into_iter()
        .enumerate()
        .map(|(row_idx, line)| {
            let positions = logo_block_positions(&line);
            let first_visible = positions.first().copied();
            let last_visible = positions.last().copied();
            let chars = line.chars().collect::<Vec<_>>();
            let mut spans = Vec::new();
            let mut current_text = String::new();
            let mut current_style = Style::default().fg(dim_color());
            let mut has_style = false;

            for (char_idx, ch) in chars.iter().enumerate() {
                let style = if *ch == '█' {
                    logo_block_style(row_idx, char_idx, elapsed, options.animated)
                } else {
                    Style::default().fg(dim_color())
                };

                let piece = if *ch == '█' {
                    let edge = Some(char_idx) == first_visible || Some(char_idx) == last_visible;
                    logo_block_glyph(row_idx, char_idx, elapsed, options.animated, edge)
                } else {
                    "·"
                };

                if !has_style {
                    current_style = style;
                    has_style = true;
                }

                if style == current_style {
                    current_text.push_str(piece);
                } else {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                    current_text.push_str(piece);
                    current_style = style;
                }
            }

            if !current_text.is_empty() {
                spans.push(Span::styled(current_text, current_style));
            }

            Line::from(spans).alignment(Alignment::Center)
        })
        .collect()
}

pub(crate) fn animated_startup_logo_lines(width: usize, elapsed: f32) -> Vec<Line<'static>> {
    animated_startup_logo_lines_for(width, elapsed, LogoFrameOptions::animated_startup())
}

fn animated_brand_header_line_for(elapsed: f32, animated: bool) -> Line<'static> {
    if !animated {
        return Line::from(Span::styled(
            crate::saitec::product_profile::brand_header_label().to_string(),
            Style::default().fg(header_name_color()),
        ))
        .alignment(Alignment::Center);
    }

    let grape_pulse = (elapsed * 1.1).sin() * 0.5 + 0.5;
    let label_pulse = (elapsed * 0.8 + 0.55).sin() * 0.5 + 0.5;
    let grape_style = Style::default().fg(rgb(
        pulse_mix(170, 230, grape_pulse),
        pulse_mix(96, 172, grape_pulse * 0.7 + 0.2),
        pulse_mix(138, 204, grape_pulse),
    ));
    let label_style = Style::default().fg(rgb(
        pulse_mix(158, 225, label_pulse),
        pulse_mix(134, 198, label_pulse * 0.55 + 0.2),
        pulse_mix(132, 206, label_pulse * 0.65 + 0.15),
    ));

    Line::from(vec![
        Span::styled("🍇", grape_style),
        Span::styled(" ", Style::default().fg(dim_color())),
        Span::styled("SAITEC-TUI", label_style),
    ])
    .alignment(Alignment::Center)
}

pub(crate) fn animated_brand_header_line(elapsed: f32) -> Line<'static> {
    animated_brand_header_line_for(elapsed, brand_animation_enabled())
}
fn overlay_segment(chars: &mut [char], start: usize, text: &str) {
    for (idx, ch) in text.chars().enumerate() {
        let pos = start + idx;
        if pos >= chars.len() {
            break;
        }
        chars[pos] = ch;
    }
}

pub(crate) fn startup_login_status_label(auth: &AuthStatus) -> &'static str {
    match auth.jcode {
        AuthState::Available => "Logged In",
        AuthState::Expired => "Login Expired",
        AuthState::NotConfigured => "Not Logged In",
    }
}

pub(crate) fn startup_model_login_state(auth: &AuthStatus) -> AuthState {
    [
        auth.anthropic.state,
        auth.openrouter,
        auth.azure,
        auth.bedrock,
        auth.openai,
        auth.cursor,
        auth.copilot,
        auth.gemini,
        auth.antigravity,
        auth.google,
    ]
    .into_iter()
    .fold(
        AuthState::NotConfigured,
        |state, candidate| match candidate {
            AuthState::Available => AuthState::Available,
            AuthState::Expired if state == AuthState::NotConfigured => AuthState::Expired,
            _ => state,
        },
    )
}

pub(crate) fn startup_model_login_label(auth: &AuthStatus) -> &'static str {
    match startup_model_login_state(auth) {
        AuthState::Available => "Model Configured",
        AuthState::Expired => "Model Auth Expired",
        AuthState::NotConfigured => "Model Not Configured",
    }
}

pub(crate) fn startup_status_indicator(state: AuthState) -> (&'static str, Color) {
    match state {
        AuthState::Available => ("●", rgb(100, 200, 100)),
        AuthState::Expired => ("●", rgb(255, 200, 100)),
        AuthState::NotConfigured => ("●", rgb(210, 80, 80)),
    }
}

pub(crate) fn startup_footer_segments(app: &dyn TuiState) -> (String, String, String, String) {
    let version = semver().to_string();
    let working_dir = app
        .working_dir()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|dir| dir.display().to_string())
        })
        .map(|dir| abbreviate_home(&dir))
        .unwrap_or_default();
    let auth = app.auth_status();
    let model_login = startup_model_login_label(&auth).to_string();
    let login_status = startup_login_status_label(&auth).to_string();
    (working_dir, model_login, login_status, version)
}

fn startup_status_layout(
    width: usize,
    model_login: &str,
    login_status: &str,
    version: &str,
) -> (String, usize, String, usize, String, usize) {
    if width == 0 {
        return (String::new(), 0, String::new(), 0, String::new(), 0);
    }

    let left_source = format!("● {model_login}");
    let right = if UnicodeWidthStr::width(version) >= width {
        truncate_to_width(version, width)
    } else {
        version.to_string()
    };
    let right_width = UnicodeWidthStr::width(right.as_str());
    let right_start = width.saturating_sub(right_width);

    let center_source = format!("● {login_status}");
    let max_center_width = right_start.saturating_sub(1).max(1);
    let center = if UnicodeWidthStr::width(center_source.as_str()) > max_center_width {
        truncate_to_width(center_source.as_str(), max_center_width)
    } else {
        center_source
    };
    let center_width = UnicodeWidthStr::width(center.as_str());
    let center_start = width.saturating_sub(center_width) / 2;

    let max_left_width = center_start.saturating_sub(1);
    let left = if max_left_width == 0 {
        String::new()
    } else if UnicodeWidthStr::width(left_source.as_str()) > max_left_width {
        truncate_to_width(left_source.as_str(), max_left_width)
    } else {
        left_source
    };

    (left, 0, center, center_start, right, right_start)
}

pub(crate) fn startup_status_line_text(
    width: usize,
    model_login: &str,
    login_status: &str,
    version: &str,
) -> String {
    if width == 0 {
        return String::new();
    }

    let (left, left_start, center, center_start, right, right_start) =
        startup_status_layout(width, model_login, login_status, version);
    let mut chars = vec![' '; width];
    overlay_segment(&mut chars, left_start, left.as_str());
    overlay_segment(&mut chars, center_start, center.as_str());
    overlay_segment(&mut chars, right_start, right.as_str());

    chars.into_iter().collect()
}

pub(crate) fn startup_status_line(
    width: usize,
    model_login: &str,
    model_state: AuthState,
    login_status: &str,
    login_state: AuthState,
    version: &str,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }

    let (left, left_start, center, center_start, right, right_start) =
        startup_status_layout(width, model_login, login_status, version);

    let mut chars = vec![' '; width];
    let mut colors = vec![None; width];

    overlay_segment(&mut chars, left_start, left.as_str());
    if !left.is_empty() && left_start < colors.len() {
        colors[left_start] = Some(startup_status_indicator(model_state).1);
    }
    overlay_segment(&mut chars, center_start, center.as_str());
    if center_start < colors.len() {
        colors[center_start] = Some(startup_status_indicator(login_state).1);
    }
    overlay_segment(&mut chars, right_start, right.as_str());

    let dim = dim_color();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_color = colors.first().copied().flatten();

    for (idx, ch) in chars.into_iter().enumerate() {
        let color = colors[idx];
        if idx == 0 || color == current_color {
            current.push(ch);
        } else {
            spans.push(Span::styled(
                std::mem::take(&mut current),
                Style::default().fg(current_color.unwrap_or(dim)),
            ));
            current.push(ch);
            current_color = color;
        }
    }

    if !current.is_empty() {
        spans.push(Span::styled(
            current,
            Style::default().fg(current_color.unwrap_or(dim)),
        ));
    }

    Line::from(spans)
}

pub(crate) fn startup_working_dir_line(width: usize, working_dir: &str) -> String {
    if width == 0 {
        return String::new();
    }

    truncate_to_width(working_dir, width)
}

pub(crate) fn build_startup_header(app: &dyn TuiState, width: u16) -> Vec<Line<'static>> {
    let w = width as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.extend(animated_startup_logo_lines(w, app.animation_elapsed()));

    lines.push(Line::from(""));

    let (working_dir, model_login, login_status, version) = startup_footer_segments(app);
    let auth = app.auth_status();
    let working_dir_line = startup_working_dir_line(w, &working_dir);
    let footer = startup_status_line(
        w,
        &model_login,
        startup_model_login_state(&auth),
        &login_status,
        auth.jcode,
        &version,
    );
    lines.push(Line::from(Span::styled(
        working_dir_line,
        Style::default().fg(dim_color()),
    )));
    lines.push(footer);

    lines
}

fn truncate_to_width(text: &str, width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }

    let mut truncated = text
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
fn choose_header_candidate(width: usize, candidates: Vec<String>) -> String {
    let mut last_non_empty = String::new();
    for candidate in candidates
        .into_iter()
        .filter(|candidate| !candidate.trim().is_empty())
    {
        if candidate.chars().count() <= width {
            return candidate;
        }
        last_non_empty = candidate;
    }

    truncate_to_width(&last_non_empty, width)
}

#[cfg(test)]
fn semver_core() -> String {
    semver()
        .split('-')
        .next()
        .unwrap_or_else(semver)
        .to_string()
}

#[cfg(test)]
fn semver_minor() -> String {
    let core = semver_core();
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        core
    }
}

#[cfg(test)]
fn version_display_candidates() -> Vec<String> {
    let full = format!("jcode {}", semver());
    let core = format!("jcode {}", semver_core());
    let minor = format!("jcode {}", semver_minor());
    let shortest = semver_minor();
    vec![full, core, minor, shortest]
}

#[cfg(test)]
fn configured_auth_count(auth: &AuthStatus) -> usize {
    [
        auth.jcode,
        auth.anthropic.state,
        auth.openrouter,
        auth.azure,
        auth.openai,
        auth.cursor,
        auth.copilot,
        auth.gemini,
        auth.antigravity,
        auth.google,
    ]
    .into_iter()
    .filter(|state| *state != AuthState::NotConfigured)
    .count()
}

pub(super) fn build_persistent_header(app: &dyn TuiState, width: u16) -> Vec<Line<'static>> {
    let model = app.provider_model();
    let session_name = app.session_display_name().unwrap_or_default();
    let server_name = app.server_display_name();
    let short_model = shorten_model_name(&model);
    let icon = connection_type_icon(app.connection_type().as_deref())
        .unwrap_or_else(|| crate::id::session_icon(&session_name));
    let nice_model = format_model_name(&short_model);
    let build_info = binary_age().unwrap_or_else(|| "unknown".to_string());
    let align = Alignment::Center;
    let mut lines: Vec<Line> = Vec::new();
    let w = width as usize;

    let is_canary = app.is_canary();
    let is_remote = app.is_remote_mode();
    let server_update = app.server_update_available() == Some(true);
    let client_update = app.client_update_available();
    let mut status_items: Vec<&str> = Vec::new();
    if app.is_replay() {
        status_items.push("replay");
    } else if is_remote {
        status_items.push("client");
    }
    if is_canary {
        status_items.push("dev");
    }
    if server_update {
        status_items.push("srv↑");
    }
    if client_update {
        status_items.push("cli↑");
    }
    if let Some(badge) = crate::perf::profile().tier.badge() {
        status_items.push(badge);
    }

    if !status_items.is_empty() {
        let badge_text = format!("⟨{}⟩", status_items.join("·"));
        lines.push(
            Line::from(Span::styled(badge_text, Style::default().fg(dim_color()))).alignment(align),
        );
    } else {
        lines.push(Line::from(""));
    }

    if let Some(server_name) = server_name.as_deref() {
        let server_icon = app.server_display_icon().unwrap_or_default();
        let server_text = if server_icon.is_empty() {
            format!("server: {}", capitalize(server_name))
        } else {
            format!("server: {} {}", capitalize(server_name), server_icon)
        };
        lines.push(
            Line::from(Span::styled(
                server_text,
                Style::default().fg(header_name_color()),
            ))
            .alignment(align),
        );
    }

    if !session_name.is_empty() {
        let client_text = format!("client: {} {}", capitalize(&session_name), icon);
        lines.push(
            Line::from(Span::styled(
                client_text,
                Style::default().fg(header_name_color()),
            ))
            .alignment(align),
        );
    } else {
        lines.push(animated_brand_header_line(app.animation_elapsed()).alignment(align));
    }

    lines.push(
        Line::from(Span::styled(
            nice_model,
            Style::default().fg(header_session_color()),
        ))
        .alignment(align),
    );

    let version_text = if is_running_stable_release() {
        let tag = env!("JCODE_GIT_TAG");
        if tag.is_empty() || tag.contains('-') {
            let full = format!("{} · release · built {}", semver(), build_info);
            if full.chars().count() <= w {
                full
            } else {
                format!("{} · release", semver())
            }
        } else {
            let full = format!("{} · release {} · built {}", semver(), tag, build_info);
            if full.chars().count() <= w {
                full
            } else {
                format!("{} · {}", semver(), tag)
            }
        }
    } else {
        let full = format!("{} · built {}", semver(), build_info);
        if full.chars().count() <= w {
            full
        } else {
            semver().to_string()
        }
    };
    lines.push(
        Line::from(Span::styled(version_text, Style::default().fg(dim_color()))).alignment(align),
    );

    if let Some(dir) = app.working_dir() {
        let display_dir = abbreviate_home(&dir);
        lines.push(
            Line::from(Span::styled(display_dir, Style::default().fg(dim_color())))
                .alignment(align),
        );
    }

    lines
}

pub(crate) fn build_header_lines(app: &dyn TuiState, width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    let align = ratatui::layout::Alignment::Center;
    let model = app.provider_model();
    let provider_name = app.provider_name();
    let upstream = app.upstream_provider();
    let auth = app.auth_status();
    let w = width as usize;
    let model = model.trim().to_string();
    let provider_label = {
        let trimmed = provider_name.trim();
        if trimmed.is_empty() {
            String::new()
        } else {
            let name = trimmed.to_lowercase();
            let auth_tag = header_provider_auth_tag(&name, &auth);
            if auth_tag.is_empty() {
                name
            } else {
                format!("{}:{}", auth_tag, name)
            }
        }
    };

    let suppress_placeholder_detail = provider_label.is_empty()
        && upstream.is_none()
        && matches!(model.as_str(), "" | "connecting to server…" | "connected");

    let model_info = if suppress_placeholder_detail || model.is_empty() {
        String::new()
    } else if let Some(ref provider) = upstream {
        if provider_label.is_empty() {
            let full = format!("{} via {} · /model to switch", model, provider);
            if full.chars().count() <= w {
                full
            } else {
                format!("{} via {}", model, provider)
            }
        } else {
            let full = format!(
                "({}) {} via {} · /model to switch",
                provider_label, model, provider
            );
            if full.chars().count() <= w {
                full
            } else {
                let short = format!("({}) {} via {}", provider_label, model, provider);
                if short.chars().count() <= w {
                    short
                } else {
                    format!("({}) {}", provider_label, model)
                }
            }
        }
    } else if provider_label.is_empty() {
        let full = format!("{} · /model to switch", model);
        if full.chars().count() <= w {
            full
        } else {
            model.clone()
        }
    } else {
        let full = format!("({}) {} · /model to switch", provider_label, model);
        if full.chars().count() <= w {
            full
        } else {
            format!("({}) {}", provider_label, model)
        }
    };
    if !model_info.is_empty() {
        lines.push(
            Line::from(Span::styled(model_info, Style::default().fg(dim_color()))).alignment(align),
        );
    }

    let auth_line = build_auth_status_line(&auth, w);
    if !auth_line.spans.is_empty() {
        lines.push(auth_line.alignment(align));
    }

    if let Some(goal_badge) = crate::goal::header_badge(
        app.working_dir().as_deref().map(std::path::Path::new),
        app.side_panel(),
    ) {
        lines.push(
            Line::from(Span::styled(
                goal_badge,
                Style::default().fg(rgb(170, 200, 120)),
            ))
            .alignment(align),
        );
    }

    let new_entries = unseen_changelog_entries();
    if !new_entries.is_empty() && w > 20 {
        const MAX_LINES: usize = 8;
        let available_width = w.saturating_sub(2);
        let display_count = new_entries.len().min(MAX_LINES);
        let has_more = new_entries.len() > MAX_LINES;

        let mut content: Vec<Line> = Vec::new();
        for entry in new_entries.iter().take(display_count) {
            content.push(
                Line::from(Span::styled(
                    format!("• {}", entry),
                    Style::default().fg(dim_color()),
                ))
                .alignment(align),
            );
        }
        if has_more {
            content.push(
                Line::from(Span::styled(
                    format!(
                        "  …{} more · /changelog to see all",
                        new_entries.len() - MAX_LINES
                    ),
                    Style::default().fg(dim_color()),
                ))
                .alignment(align),
            );
        }

        let boxed = render_rounded_box(
            "Updates",
            content,
            available_width,
            Style::default().fg(dim_color()),
        );
        for line in boxed {
            lines.push(line.alignment(align));
        }
    }

    let mcps = app.mcp_servers();
    let mcp_text = if mcps.is_empty() {
        "mcp: (none)".to_string()
    } else {
        let full_parts: Vec<String> = mcps
            .iter()
            .map(|(name, count)| {
                if *count > 0 {
                    format!("{} ({} tools)", name, count)
                } else {
                    format!("{} (...)", name)
                }
            })
            .collect();
        let full = format!("mcp: {}", full_parts.join(", "));
        if full.chars().count() <= w {
            full
        } else {
            let short_parts: Vec<String> = mcps
                .iter()
                .map(|(name, count)| {
                    if *count > 0 {
                        format!("{}({})", name, count)
                    } else {
                        format!("{}(…)", name)
                    }
                })
                .collect();
            let short = format!("mcp: {}", short_parts.join(" "));
            if short.chars().count() <= w {
                short
            } else {
                format!("mcp: {} servers", mcps.len())
            }
        }
    };
    lines.push(
        Line::from(Span::styled(mcp_text, Style::default().fg(dim_color()))).alignment(align),
    );

    let skills = app.available_skills();
    if crate::saitec::product_profile::show_skills_in_ui() && !skills.is_empty() {
        let full = format!(
            "skills: {}",
            skills
                .iter()
                .map(|s| format!("/{}", s))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let skills_text = if full.chars().count() <= w {
            full
        } else {
            format!("skills: {} loaded", skills.len())
        };
        lines.push(
            Line::from(Span::styled(skills_text, Style::default().fg(dim_color())))
                .alignment(align),
        );
    }

    let client_count = app.connected_clients().unwrap_or(0);
    let session_count = app.server_sessions().len();
    if client_count > 0 || session_count > 1 {
        let mut parts = Vec::new();
        if client_count > 0 {
            parts.push(format!(
                "{} client{}",
                client_count,
                if client_count == 1 { "" } else { "s" }
            ));
        }
        if session_count > 1 {
            parts.push(format!("{} sessions", session_count));
        }
        lines.push(
            Line::from(Span::styled(
                format!("server: {}", parts.join(", ")),
                Style::default().fg(dim_color()),
            ))
            .alignment(align),
        );
    }

    lines.push(Line::from(""));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthState, AuthStatus, ProviderAuth};
    use crate::message::Message;
    use crate::provider::{EventStream, Provider};
    use crate::tool::Registry;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::sync::OnceLock;

    struct MockProvider;

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[crate::message::ToolDefinition],
            _system: &str,
            _resume_session_id: Option<&str>,
        ) -> Result<EventStream> {
            Err(anyhow::anyhow!(
                "Mock provider should not be used for streaming completions in ui header tests"
            ))
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn fork(&self) -> Arc<dyn Provider> {
            Arc::new(MockProvider)
        }
    }

    fn ensure_test_jcode_home_if_unset() {
        static TEST_HOME: OnceLock<std::path::PathBuf> = OnceLock::new();

        if std::env::var_os("JCODE_HOME").is_some() {
            return;
        }

        let path = TEST_HOME.get_or_init(|| {
            let path = std::env::temp_dir().join(format!("jcode-test-home-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&path);
            path
        });
        crate::env::set_var("JCODE_HOME", path);
    }

    fn create_test_app() -> crate::tui::app::App {
        ensure_test_jcode_home_if_unset();

        let provider: Arc<dyn Provider> = Arc::new(MockProvider);
        let rt = tokio::runtime::Runtime::new().expect("test runtime");
        let registry = rt.block_on(Registry::new(provider.clone()));
        crate::tui::app::App::new_for_test_harness(provider, registry)
    }

    fn write_test_skill(root: &std::path::Path, name: &str) {
        let dir = root.join(".jcode").join("skills").join(name);
        std::fs::create_dir_all(&dir).expect("create skill dir");
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test skill {name}\n---\n\nUse {name}.\n"),
        )
        .expect("write skill");
    }

    #[test]
    fn left_aligned_mode_keeps_persistent_header_centered() {
        let mut app = create_test_app();
        app.set_centered(false);

        let lines = build_persistent_header(&app, 80);
        let non_empty: Vec<&Line<'_>> = lines
            .iter()
            .filter(|line| !line.spans.iter().all(|span| span.content.trim().is_empty()))
            .collect();

        assert!(!non_empty.is_empty(), "expected persistent header lines");
        assert!(
            non_empty
                .iter()
                .all(|line| line.alignment == Some(Alignment::Center)),
            "persistent header should remain centered in left-aligned mode: {non_empty:?}"
        );
    }

    #[test]
    fn left_aligned_mode_keeps_secondary_header_centered() {
        let mut app = create_test_app();
        app.set_centered(false);

        let lines = build_header_lines(&app, 80);
        let non_empty: Vec<&Line<'_>> = lines
            .iter()
            .filter(|line| !line.spans.iter().all(|span| span.content.trim().is_empty()))
            .collect();

        assert!(!non_empty.is_empty(), "expected header detail lines");
        assert!(
            non_empty
                .iter()
                .all(|line| line.alignment == Some(Alignment::Center)),
            "header detail lines should remain centered in left-aligned mode: {non_empty:?}"
        );
    }

    #[test]
    fn version_display_candidates_compact_for_narrow_width() {
        let rendered = choose_header_candidate(8, version_display_candidates());
        assert_eq!(rendered, "v0.9");
    }

    #[test]
    fn configured_auth_count_includes_non_model_auth_surfaces() {
        let auth = AuthStatus {
            jcode: AuthState::Available,
            anthropic: ProviderAuth {
                state: AuthState::Expired,
                has_oauth: true,
                has_api_key: false,
            },
            azure: AuthState::Available,
            google: AuthState::Available,
            ..AuthStatus::default()
        };

        assert_eq!(configured_auth_count(&auth), 4);
    }

    #[test]
    fn header_provider_auth_tag_reports_openai_oauth_and_api_key() {
        let auth = AuthStatus {
            openai: AuthState::Available,
            openai_has_oauth: true,
            openai_has_api_key: true,
            ..AuthStatus::default()
        };

        assert_eq!(header_provider_auth_tag("openai", &auth), "oauth+key");
    }

    #[test]
    fn build_persistent_header_prefers_configured_model_during_remote_connect() {
        let _guard = crate::storage::lock_test_env();
        let prev_model = std::env::var_os("JCODE_MODEL");
        let prev_provider = std::env::var_os("JCODE_PROVIDER");
        crate::env::set_var("JCODE_MODEL", "gpt-5.4");
        crate::env::set_var("JCODE_PROVIDER", "openai");

        let app = crate::tui::app::App::new_for_remote(None);
        let lines = build_persistent_header(&app, 80);
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("GPT-5.4"));
        assert!(!rendered.contains("connecting to server…"));

        if let Some(prev_model) = prev_model {
            crate::env::set_var("JCODE_MODEL", prev_model);
        } else {
            crate::env::remove_var("JCODE_MODEL");
        }
        if let Some(prev_provider) = prev_provider {
            crate::env::set_var("JCODE_PROVIDER", prev_provider);
        } else {
            crate::env::remove_var("JCODE_PROVIDER");
        }
    }

    #[test]
    fn build_header_lines_show_saitec_mcp_status_and_tool_count() {
        let mut app = create_test_app();
        app.set_mcp_server_names_for_tests(vec![
            ("SAITEC-Skills".to_string(), 0),
            ("helper".to_string(), 3),
        ]);

        let lines = build_header_lines(&app, 80);
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("mcp:"));
        assert!(rendered.contains("SAITEC-Skills"));
        assert!(rendered.contains("helper (3 tools)") || rendered.contains("helper(3)"));
    }

    #[test]
    fn build_header_lines_omits_placeholder_provider_label_when_unknown() {
        let mut app = crate::tui::app::App::new_for_remote(None);
        app.set_remote_startup_phase(crate::tui::app::RemoteStartupPhase::LoadingSession);

        let lines = build_header_lines(&app, 80);
        let rendered = lines
            .first()
            .expect("header line")
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("loading session…"));
        assert!(!rendered.contains("(unknown)"));
        assert!(!rendered.contains("(remote)"));
    }

    #[test]
    fn build_persistent_header_shows_saitec_brand_when_session_name_missing() {
        let mut app = crate::tui::app::App::new_for_remote(None);
        app.set_remote_startup_phase(crate::tui::app::RemoteStartupPhase::LoadingSession);
        let lines = build_persistent_header(&app, 80);
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("🍇 SAITEC-TUI"), "rendered: {rendered}");
        assert!(!rendered.contains("JCode-TUI"), "rendered: {rendered}");
    }

    #[test]
    fn build_header_lines_hides_secondary_placeholder_during_brief_connecting_phase() {
        let app = crate::tui::app::App::new_for_remote(None);

        let lines = build_header_lines(&app, 80);
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(
            !rendered.contains("connecting to server…"),
            "brief connecting placeholder should not render the secondary detail line"
        );
        assert!(!rendered.contains("(remote)"));
    }

    #[test]
    fn build_header_lines_hides_skills_line_in_saitec_product_mode() {
        let _guard = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let jcode_home = temp.path().join("jcode-home");
        std::fs::create_dir_all(&jcode_home).expect("create jcode home");
        write_test_skill(temp.path(), "alpha");

        let prev_home = std::env::var_os("JCODE_HOME");
        let prev_cwd = std::env::current_dir().expect("current dir");
        crate::env::set_var("JCODE_HOME", &jcode_home);
        std::env::set_current_dir(temp.path()).expect("set current dir");

        let app = create_test_app();
        let skills = app.available_skills();
        let lines = build_header_lines(&app, 120);
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        std::env::set_current_dir(prev_cwd).expect("restore current dir");
        if let Some(prev_home) = prev_home {
            crate::env::set_var("JCODE_HOME", prev_home);
        } else {
            crate::env::remove_var("JCODE_HOME");
        }

        assert!(
            skills.iter().any(|skill| skill == "alpha"),
            "expected local test skill to load, got {skills:?}"
        );
        assert!(!rendered.contains("skills:"), "rendered: {rendered}");
    }

    #[test]
    fn build_startup_header_hides_runtime_noise_and_keeps_footer() {
        let _guard = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let prev_cwd = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(temp.path()).expect("set current dir");

        let mut app = create_test_app();
        app.set_mcp_server_names_for_tests(vec![("SAITEC-Skills".to_string(), 8)]);
        let lines = build_startup_header(&app, 80);

        std::env::set_current_dir(prev_cwd).expect("restore current dir");

        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("\n");
        let working_dir = abbreviate_home(&temp.path().display().to_string());

        assert!(rendered.contains('█'), "rendered: {rendered}");
        assert!(!rendered.contains("JCode"), "rendered: {rendered}");
        assert!(rendered.contains(semver()), "rendered: {rendered}");
        assert!(rendered.contains(&working_dir), "rendered: {rendered}");
        assert!(!rendered.contains("mcp:"), "rendered: {rendered}");
        assert!(!rendered.contains("server:"), "rendered: {rendered}");
        assert!(!rendered.contains("client:"), "rendered: {rendered}");
        assert!(!rendered.contains("Updates"), "rendered: {rendered}");
    }

    #[test]
    fn startup_header_footer_uses_separate_working_dir_line() {
        let width = 48;
        let working_dir = "G:\\Workspace\\Project2026\\JCode\\very-long-project-name\\nested\\path";
        let model_login = "Model Configured";
        let login_status = "Not Logged In";
        let version = "v0.12.0";

        let working_dir_line = startup_working_dir_line(width, working_dir);
        let footer_line = startup_status_line_text(width, model_login, login_status, version);

        assert_eq!(
            working_dir_line,
            truncate_to_width(working_dir, width),
            "working directory should occupy its own truncated line"
        );
        assert!(!footer_line.contains("Workspace"), "footer: {footer_line}");
        assert!(footer_line.contains("Model"), "footer: {footer_line}");
        assert!(footer_line.contains(login_status), "footer: {footer_line}");
        assert!(
            footer_line.trim_end().ends_with(version),
            "footer: {footer_line}"
        );
    }

    #[test]
    fn startup_status_line_distinguishes_model_login_from_api_key_configuration() {
        let auth = AuthStatus {
            jcode: AuthState::NotConfigured,
            openai: AuthState::Available,
            openai_has_api_key: true,
            ..AuthStatus::default()
        };

        let line = startup_status_line(
            80,
            startup_model_login_label(&auth),
            startup_model_login_state(&auth),
            startup_login_status_label(&auth),
            auth.jcode,
            "v0.12.0",
        );
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("Model Configured"), "footer: {rendered}");
        assert!(!rendered.contains("Model Logged In"), "footer: {rendered}");
        assert!(rendered.contains("Not Logged In"), "footer: {rendered}");
        assert!(rendered.contains("v0.12.0"), "footer: {rendered}");
        assert!(rendered.contains('●'), "footer: {rendered}");
    }

    #[test]
    fn startup_status_line_shows_model_logged_in_for_oauth_session() {
        let auth = AuthStatus {
            jcode: AuthState::NotConfigured,
            openai: AuthState::Available,
            openai_has_oauth: true,
            ..AuthStatus::default()
        };

        let line = startup_status_line(
            80,
            startup_model_login_label(&auth),
            startup_model_login_state(&auth),
            startup_login_status_label(&auth),
            auth.jcode,
            "v0.12.0",
        );
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("Model Configured"), "footer: {rendered}");
        assert!(!rendered.contains("Model Logged In"), "footer: {rendered}");
    }

    #[test]
    fn startup_logo_fallback_shifts_middle_rows_left_by_one_cell() {
        let lines = startup_logo_lines(80);
        assert_eq!(lines.len(), 5, "expected full startup logo");

        assert_eq!(lines[0].chars().next(), Some('█'));
        assert_eq!(lines[1].chars().next(), Some('█'));
        assert_eq!(lines[2].chars().next(), Some('█'));
        assert_eq!(lines[3].chars().next(), Some(' '));
        assert_eq!(lines[4].chars().next(), Some('█'));

        assert!(
            !lines[1].starts_with(" "),
            "second row should not have a leading pad: {:?}",
            lines[1]
        );
        assert!(
            !lines[2].starts_with(" "),
            "third row should not have a leading pad: {:?}",
            lines[2]
        );
        assert!(
            lines[3].starts_with("      "),
            "fourth row should keep the SAITEC logo's deeper left indent: {:?}",
            lines[3]
        );
        assert!(
            !lines[3].starts_with("       "),
            "fourth row should not drift further right than the intended SAITEC layout: {:?}",
            lines[3]
        );
        assert!(
            !lines[4].starts_with(" "),
            "fifth row should not have a leading pad: {:?}",
            lines[4]
        );
    }

    #[test]
    fn auth_status_line_hides_not_configured_providers() {
        let auth = AuthStatus {
            anthropic: ProviderAuth {
                state: AuthState::Expired,
                has_oauth: true,
                has_api_key: false,
            },
            openai: AuthState::Available,
            openai_has_oauth: false,
            openai_has_api_key: true,
            ..AuthStatus::default()
        };

        let line = build_auth_status_line(&auth, 120);
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(
            rendered.contains("anthropic(oauth)"),
            "rendered: {rendered}"
        );
        assert!(rendered.contains("openai(key)"), "rendered: {rendered}");
        assert!(!rendered.contains("openrouter"), "rendered: {rendered}");
        assert!(!rendered.contains("copilot"), "rendered: {rendered}");
        assert!(!rendered.contains("cursor"), "rendered: {rendered}");
    }

    #[test]
    fn auth_status_line_is_empty_when_nothing_was_attempted() {
        let line = build_auth_status_line(&AuthStatus::default(), 120);
        assert!(line.spans.is_empty(), "line should be empty: {line:?}");
    }

    #[test]
    fn startup_logo_animation_changes_visible_cells_over_time() {
        let early = animated_startup_logo_lines_for(
            80,
            0.0,
            LogoFrameOptions {
                animated: true,
                startup_surface: true,
            },
        );
        let later = animated_startup_logo_lines_for(
            80,
            1.4,
            LogoFrameOptions {
                animated: true,
                startup_surface: true,
            },
        );

        let early_rendered = early
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("\n");
        let later_rendered = later
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("\n");

        assert_ne!(early_rendered, later_rendered);
    }

    #[test]
    fn startup_logo_animation_preserves_layout_widths() {
        let early = animated_startup_logo_lines_for(
            80,
            0.0,
            LogoFrameOptions {
                animated: true,
                startup_surface: true,
            },
        );
        let later = animated_startup_logo_lines_for(
            80,
            1.4,
            LogoFrameOptions {
                animated: true,
                startup_surface: true,
            },
        );

        let early_widths = early.iter().map(|line| line.width()).collect::<Vec<_>>();
        let later_widths = later.iter().map(|line| line.width()).collect::<Vec<_>>();

        assert_eq!(early_widths, later_widths);
        assert_eq!(early.len(), later.len());
    }

    #[test]
    fn startup_logo_static_mode_keeps_glyphs_stable() {
        let early = animated_startup_logo_lines_for(80, 0.0, LogoFrameOptions::static_startup());
        let later = animated_startup_logo_lines_for(80, 2.0, LogoFrameOptions::static_startup());

        let early_rendered = early
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("\n");
        let later_rendered = later
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(early_rendered, later_rendered);
    }

    #[test]
    fn persistent_header_brand_animation_keeps_text_but_changes_style() {
        let early = animated_brand_header_line_for(0.0, true);
        let later = animated_brand_header_line_for(1.8, true);

        let early_text = early
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let later_text = later
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert_eq!(early_text, "🍇 SAITEC-TUI");
        assert_eq!(later_text, "🍇 SAITEC-TUI");
        assert_ne!(early.spans[0].style.fg, later.spans[0].style.fg);
    }
}
