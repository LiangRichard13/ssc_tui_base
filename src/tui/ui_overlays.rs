use super::{
    accent_color, ai_color, ai_text, asap_color, clear_area, dim_color, get_grouped_changelog,
    header_icon_color, header_name_color, header_session_color, pending_color, queued_color, rgb,
    tool_color, user_bg, user_color, user_text,
};
use crate::tui::TuiState;
use crate::tui::info_widget::WidgetPlacement;
use crate::tui::markdown;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

fn saitec_form_field_value(
    form: &crate::tui::app::SaitecPendingForm,
    field: crate::tui::app::SaitecLoginField,
) -> String {
    match field {
        crate::tui::app::SaitecLoginField::Email => form.form.email.clone(),
        crate::tui::app::SaitecLoginField::Phone => form.form.phone.clone(),
        crate::tui::app::SaitecLoginField::Password => {
            "*".repeat(form.form.password.chars().count())
        }
        crate::tui::app::SaitecLoginField::Submit => {
            if form.submitting {
                "[ Submitting... ]".to_string()
            } else {
                "[ Submit ]".to_string()
            }
        }
        crate::tui::app::SaitecLoginField::Cancel => "[ Cancel ]".to_string(),
    }
}

fn saitec_focus_style(
    form: &crate::tui::app::SaitecPendingForm,
    field: crate::tui::app::SaitecLoginField,
) -> Style {
    if form.focus == field {
        Style::default()
            .fg(accent_color())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(rgb(210, 210, 225))
    }
}

pub(super) fn draw_saitec_login_overlay(
    frame: &mut Frame,
    area: Rect,
    form: &crate::tui::app::SaitecPendingForm,
    live_input: &str,
    live_cursor_pos: usize,
) {
    let width = area.width.min(72).max(48);
    let inner_width = width.saturating_sub(2) as usize;
    let error_lines = form
        .error
        .as_deref()
        .map(|error| {
            markdown::wrap_line(
                Line::from(Span::styled(
                    format!(" {}", error),
                    Style::default().fg(Color::Red),
                )),
                inner_width.max(1),
            )
        })
        .unwrap_or_default();
    let hint_lines = if form.error.is_some() {
        Vec::new()
    } else if form.submitting {
        vec![Line::from(Span::styled(
            " Validating API key session and saving local auth...",
            Style::default().fg(pending_color()),
        ))]
    } else {
        vec![Line::from(Span::styled(
            " Email and phone cannot both be empty.",
            Style::default().fg(dim_color()),
        ))]
    };
    let footer_lines = if error_lines.is_empty() {
        hint_lines.clone()
    } else {
        error_lines.clone()
    };
    let desired_height = (10usize + footer_lines.len()).min(area.height.saturating_sub(2) as usize);
    let height = desired_height.max(10) as u16;
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    clear_area(frame, popup);

    let block = Block::default()
        .title(Span::styled(
            " Saitec Login ",
            Style::default()
                .fg(accent_color())
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " Tab/Shift+Tab move · Enter submit · /cancel abort ",
            Style::default().fg(dim_color()),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(rgb(90, 120, 110)));

    let email_value = if form.focus == crate::tui::app::SaitecLoginField::Email {
        live_input.to_string()
    } else {
        form.form.email.clone()
    };
    let phone_value = if form.focus == crate::tui::app::SaitecLoginField::Phone {
        live_input.to_string()
    } else {
        form.form.phone.clone()
    };
    let password_value = if form.focus == crate::tui::app::SaitecLoginField::Password {
        "*".repeat(live_input.chars().count())
    } else {
        saitec_form_field_value(form, crate::tui::app::SaitecLoginField::Password)
    };

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            " Enter email or phone plus password to continue.",
            Style::default().fg(rgb(180, 185, 195)),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " Email    ",
                saitec_focus_style(form, crate::tui::app::SaitecLoginField::Email),
            ),
            Span::styled(
                if email_value.is_empty() {
                    " ".to_string()
                } else {
                    email_value.clone()
                },
                Style::default().fg(rgb(235, 235, 245)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " Phone    ",
                saitec_focus_style(form, crate::tui::app::SaitecLoginField::Phone),
            ),
            Span::styled(
                if phone_value.is_empty() {
                    " ".to_string()
                } else {
                    phone_value.clone()
                },
                Style::default().fg(rgb(235, 235, 245)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " Password ",
                saitec_focus_style(form, crate::tui::app::SaitecLoginField::Password),
            ),
            Span::styled(
                password_value.clone(),
                Style::default().fg(rgb(235, 235, 245)),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                saitec_form_field_value(form, crate::tui::app::SaitecLoginField::Submit),
                saitec_focus_style(form, crate::tui::app::SaitecLoginField::Submit),
            ),
            Span::raw("  "),
            Span::styled(
                saitec_form_field_value(form, crate::tui::app::SaitecLoginField::Cancel),
                saitec_focus_style(form, crate::tui::app::SaitecLoginField::Cancel),
            ),
        ]),
        Line::from(""),
    ];

    lines.extend(footer_lines);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);

    let field_row = match form.focus {
        crate::tui::app::SaitecLoginField::Email => {
            Some((2u16, " Email    ", email_value.as_str()))
        }
        crate::tui::app::SaitecLoginField::Phone => {
            Some((3u16, " Phone    ", phone_value.as_str()))
        }
        crate::tui::app::SaitecLoginField::Password => Some((4u16, " Password ", live_input)),
        crate::tui::app::SaitecLoginField::Submit | crate::tui::app::SaitecLoginField::Cancel => {
            None
        }
    };

    if let Some((content_row, label, field_value)) = field_row {
        let cursor_char_pos =
            crate::tui::core::byte_offset_to_char_index(field_value, live_cursor_pos);
        let cursor_prefix = field_value
            .chars()
            .take(cursor_char_pos)
            .collect::<String>();
        let cursor_x = popup.x
            + 1
            + UnicodeWidthStr::width(label) as u16
            + UnicodeWidthStr::width(cursor_prefix.as_str()) as u16;
        let cursor_y = popup.y + 1 + content_row;
        frame.set_cursor_position(Position::new(
            cursor_x.min(popup.x + popup.width.saturating_sub(2)),
            cursor_y.min(popup.y + popup.height.saturating_sub(2)),
        ));
    }
}

pub(super) fn draw_changelog_overlay(frame: &mut Frame, area: Rect, scroll: usize) {
    clear_area(frame, area);

    let groups = get_grouped_changelog();
    let mut lines: Vec<Line<'static>> = Vec::new();

    if groups.is_empty() {
        lines.push(Line::from(Span::styled(
            "No changelog entries available.",
            Style::default().fg(dim_color()),
        )));
    } else {
        for group in &groups {
            let heading = match &group.released_at {
                Some(released_at) => format!("  {} · {}", group.version, released_at),
                None => format!("  {}", group.version),
            };
            lines.push(Line::from(Span::styled(
                heading,
                Style::default()
                    .fg(rgb(200, 200, 220))
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            for entry in &group.entries {
                lines.push(Line::from(vec![
                    Span::styled("    • ", Style::default().fg(dim_color())),
                    Span::styled(entry.clone(), Style::default().fg(rgb(170, 170, 185))),
                ]));
            }
            lines.push(Line::from(""));
        }
    }

    let total_lines = lines.len();
    let visible_height = area.height.saturating_sub(2) as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = scroll.min(max_scroll);

    let scroll_info = if total_lines > visible_height {
        let pct = if max_scroll > 0 {
            (scroll * 100) / max_scroll
        } else {
            100
        };
        format!(" {}% ", pct)
    } else {
        String::new()
    };

    let title = format!(" Changelog {} ", scroll_info);
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(rgb(200, 200, 220))
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " Esc to close · mouse wheel/j/k scroll · Space/PageUp page ",
            Style::default().fg(dim_color()),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim_color()));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

pub(super) fn draw_help_overlay(frame: &mut Frame, area: Rect, scroll: usize, app: &dyn TuiState) {
    clear_area(frame, area);

    let section_style = Style::default()
        .fg(accent_color())
        .add_modifier(Modifier::BOLD);
    let cmd_style = Style::default().fg(rgb(230, 230, 240));
    let desc_style = Style::default().fg(rgb(150, 150, 165));
    let key_style = Style::default().fg(rgb(200, 180, 120));
    let sep_style = Style::default().fg(rgb(50, 50, 55));

    let mut lines: Vec<Line<'static>> = Vec::new();

    let separator = || -> Line<'static> {
        Line::from(Span::styled(
            "  ─────────────────────────────────────────────────",
            sep_style,
        ))
    };

    let help_entry = |cmd: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(cmd.to_string(), cmd_style),
            Span::styled("  ", Style::default()),
            Span::styled(desc.to_string(), desc_style),
        ])
    };

    let key_entry = |key: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(format!("{:<22}", key), key_style),
            Span::styled(desc.to_string(), desc_style),
        ])
    };

    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled("  Commands", section_style)));
    lines.push(Line::from(""));
    lines.push(help_entry("/help", "Show this help overlay"));
    lines.push(help_entry(
        "/help <command>",
        "Show details for one command",
    ));
    lines.push(help_entry("/login", "Start the Saitec login flow"));
    lines.push(help_entry("/logout", "Clear local Saitec authentication"));
    lines.push(help_entry("/auth", "Show authentication status"));
    lines.push(help_entry("/model", "List or switch models"));
    lines.push(help_entry("/clear", "Clear conversation and start fresh"));
    lines.push(help_entry("/resume", "Browse and resume previous sessions"));
    lines.push(help_entry("/usage", "Show connected provider usage limits"));
    lines.push(help_entry("/version", "Show version and build details"));
    lines.push(help_entry("/quit", "Exit SAITEC-TUI"));

    lines.push(Line::from(""));
    lines.push(separator());
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled("  MCP Status", section_style)));
    lines.push(Line::from(""));
    let mcps = app.mcp_servers();
    if crate::saitec::product_profile::emphasize_mcp_status() {
        if mcps.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled("Header mcp:", cmd_style),
                Span::styled("  No MCP servers connected", desc_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled("Header mcp:", cmd_style),
                Span::styled("  Connected MCP servers and tool counts", desc_style),
            ]));
            for (name, count) in mcps {
                let label = if count > 0 {
                    format!("{name} ({count} tools)")
                } else {
                    format!("{name} (...)")
                };
                lines.push(help_entry(&label, "Connected MCP server"));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(separator());
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled("  Navigation", section_style)));
    lines.push(Line::from(""));
    lines.push(key_entry(
        "PageUp / PageDown",
        "Scroll the help or chat history",
    ));
    lines.push(key_entry(
        "Up / Down",
        "Scroll history when the input is empty",
    ));
    lines.push(key_entry(
        "Shift+Enter",
        "Insert a newline in the input box",
    ));
    lines.push(key_entry(
        "Ctrl+C / Ctrl+D",
        "Quit (press twice to confirm)",
    ));
    if let Some(label) = app.dictation_key_label() {
        lines.push(key_entry(&label, "Run configured dictation"));
    }

    lines.push(Line::from(""));

    let total_lines = lines.len();
    let visible_height = area.height.saturating_sub(2) as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = scroll.min(max_scroll);

    let scroll_info = if total_lines > visible_height {
        let pct = if max_scroll > 0 {
            (scroll * 100) / max_scroll
        } else {
            100
        };
        format!(" {}% ", pct)
    } else {
        String::new()
    };

    let title = format!(" Help {} ", scroll_info);
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(rgb(200, 200, 220))
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " Esc to close · mouse wheel/j/k scroll · Space/PageUp page · /help <cmd> for details ",
            Style::default().fg(dim_color()),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim_color()));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

pub(super) fn draw_debug_overlay(
    frame: &mut Frame,
    placements: &[WidgetPlacement],
    chunks: &[Rect],
) {
    if chunks.len() < 5 {
        return;
    }
    render_overlay_box(frame, chunks[0], "messages", Color::Red);
    render_overlay_box(frame, chunks[1], "queued", Color::Yellow);
    render_overlay_box(frame, chunks[2], "status", Color::Cyan);
    render_overlay_box(frame, chunks[3], "picker", Color::Magenta);
    render_overlay_box(frame, chunks[4], "input", Color::Green);
    if chunks.len() > 5 && chunks[5].height > 0 {
        render_overlay_box(frame, chunks[5], "donut", Color::Blue);
    }

    for placement in placements {
        let title = format!("widget:{}", placement.kind.as_str());
        render_overlay_box(frame, placement.rect, &title, Color::Magenta);
    }
}

fn render_overlay_box(frame: &mut Frame, area: Rect, title: &str, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(Span::styled(title.to_string(), Style::default().fg(color)));
    frame.render_widget(block, area);
}

pub(super) fn debug_palette_json() -> Option<serde_json::Value> {
    Some(serde_json::json!({
        "user_color": color_to_rgb(user_color()),
        "ai_color": color_to_rgb(ai_color()),
        "tool_color": color_to_rgb(tool_color()),
        "dim_color": color_to_rgb(dim_color()),
        "accent_color": color_to_rgb(accent_color()),
        "queued_color": color_to_rgb(queued_color()),
        "asap_color": color_to_rgb(asap_color()),
        "pending_color": color_to_rgb(pending_color()),
        "user_text": color_to_rgb(user_text()),
        "user_bg": color_to_rgb(user_bg()),
        "ai_text": color_to_rgb(ai_text()),
        "header_icon_color": color_to_rgb(header_icon_color()),
        "header_name_color": color_to_rgb(header_name_color()),
        "header_session_color": color_to_rgb(header_session_color()),
    }))
}

fn color_to_rgb(color: Color) -> Option<[u8; 3]> {
    match color {
        Color::Rgb(r, g, b) => Some([r, g, b]),
        Color::Indexed(n) if n >= 16 => {
            let (r, g, b) = crate::tui::color_support::indexed_to_rgb(n);
            Some([r, g, b])
        }
        _ => None,
    }
}
