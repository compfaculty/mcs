//! Responsive shortcut tiles. Glyphs are decorative; key bindings remain visible as text.
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

pub const FUNCTION_KEYS: [(&str, &str); 12] = [
    ("F1", "Help"),
    ("F2", "User"),
    ("F3", "View"),
    ("F4", "Edit"),
    ("F5", "Copy"),
    ("F6", "Move"),
    ("F7", "Mkdir"),
    ("F8", "Delete"),
    ("F9", "Menu"),
    ("F10", "Quit"),
    ("F11", "Unused"),
    ("F12", "Unused"),
];

fn disabled(key: &str) -> bool {
    matches!(key, "F11" | "F12")
        || (!cfg!(any(target_os = "macos", target_os = "linux"))
            && matches!(key, "F4" | "F5" | "F6" | "F7" | "F8"))
}

pub fn draw(frame: &mut Frame, area: Rect, actions: &[(&str, &str)], icons: bool) {
    // Keep conventional positions in wide windows; prioritize working actions when narrow.
    let compact: Vec<_> = actions
        .iter()
        .copied()
        .filter(|(key, label)| {
            if area.width < 24 {
                return *label == "Quit";
            }
            if actions.len() != 12 {
                return true;
            }
            if area.width < 80 || area.height < 3 {
                return matches!(*key, "F1" | "F3" | "F10");
            }
            if area.width < 120 {
                return !disabled(key);
            }
            area.width >= 144 || !matches!(*key, "F11" | "F12")
        })
        .collect();
    let actions = if compact.is_empty() {
        actions
    } else {
        &compact
    };
    let cells = Layout::horizontal(vec![Constraint::Fill(1); actions.len()])
        .spacing(u16::from(area.width >= 24))
        .split(area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(Color::Black)),
        area,
    );
    for ((key, label), &cell) in actions.iter().zip(cells.iter()) {
        let accent = match *label {
            _ if disabled(key) => Color::DarkGray,
            "Quit" => Color::Red,
            "View" | "Apply" | "Visit" => Color::Cyan,
            _ => Color::Gray,
        };
        // Short terminals keep a single-line, fully textual key/action fallback.
        if area.height < 3 {
            frame.render_widget(
                Paragraph::new(format!("{key} {label}"))
                    .style(Style::default().fg(accent).bg(Color::Black)),
                Rect {
                    height: 1.min(cell.height),
                    y: cell.bottom().saturating_sub(1),
                    ..cell
                },
            );
            continue;
        }
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if accent == Color::Gray {
                Color::DarkGray
            } else {
                accent
            }))
            .title(Line::from(Span::styled(
                format!(" {key} "),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )))
            .title_alignment(Alignment::Right)
            .style(Style::default().bg(Color::Black));
        let inner = block.inner(cell);
        let full = if icons && !disabled(key) {
            format!("{} {label}", glyph(label))
        } else {
            (*label).into()
        };
        let caption = if Line::raw(&full).width() <= usize::from(inner.width) {
            full
        } else {
            (*label).into()
        };
        frame.render_widget(block, cell);
        frame.render_widget(
            Paragraph::new(caption)
                .alignment(Alignment::Center)
                .style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            inner,
        );
    }
}

fn glyph(label: &str) -> &'static str {
    match label {
        "Help" => "?",
        "Actions" | "User" => "⚡",
        "Menu" => "☰",
        "View" => "📖",
        "Apply" => "✓",
        "Quit" => "⏻",
        "Visit" => "↵",
        "Cancel" | "Close" => "×",
        _ => "·",
    }
}
