use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Kind, Screen};
use crate::input::LineEdit;
use crate::settings::{Row, SettingsState};
use crate::slash::SlashItem;

const FG: Color = Color::Rgb(220, 220, 216);
const DIM: Color = Color::Rgb(110, 110, 108);
const USER: Color = Color::Rgb(180, 180, 176);
const ERR: Color = Color::Rgb(196, 90, 80);
const ACCENT: Color = Color::Rgb(200, 196, 188);
const RULE: Color = Color::Rgb(50, 50, 48);
const LABEL_W: usize = 9;

pub fn draw(frame: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::Chat => draw_chat(frame, app),
        Screen::Settings => draw_settings(frame, app),
    }
}

fn draw_chat(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let slash = app.slash_open();
    let items = if slash { app.slash_items() } else { Vec::new() };
    let drop_h = if slash {
        items.len().clamp(1, 12) as u16 + 1
    } else {
        0
    };

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(drop_h),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, chunks[0], app, false);
    draw_messages(frame, chunks[1], app);
    if slash {
        let menu = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(chunks[2]);
        draw_rule(frame, menu[0]);
        draw_slash(frame, menu[1], app.slash_cursor, &items);
    }
    draw_rule(frame, chunks[3]);
    draw_input(frame, chunks[4], &app.input, app.busy);
    draw_hint(
        frame,
        chunks[5],
        if app.busy {
            "esperando respuesta"
        } else if app.slash_picking() {
            "↑↓ · enter modelo · esc atrás"
        } else if slash {
            "↑↓ · enter · esc"
        } else {
            "enter · /model · /providers · tab · esc"
        },
    );
}

fn draw_slash(frame: &mut Frame, area: Rect, cursor: usize, items: &[SlashItem]) {
    let area = gutter(area);
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("sin coincidencias", Style::new().fg(DIM))),
            area,
        );
        return;
    }

    let height = area.height.max(1) as usize;
    let width = area.width.max(1) as usize;
    let start = cursor
        .saturating_sub(height.saturating_sub(1))
        .min(items.len().saturating_sub(height));

    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(i, item)| slash_line(item, i == cursor, width))
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

fn slash_line(item: &SlashItem, selected: bool, width: usize) -> Line<'static> {
    if let SlashItem::Header(title) = item {
        return section_line(title, width);
    }
    let mark = if selected { "›" } else { " " };
    let title = format!("{:<18}", item.title());
    let hint = item.hint();
    if selected {
        return Line::from(Span::styled(
            occupy(format!("{mark} {title}  {hint}"), width),
            Style::new().fg(FG).add_modifier(Modifier::REVERSED),
        ));
    }
    Line::from(vec![
        Span::styled(format!("{mark} {title}  "), Style::new().fg(FG)),
        Span::styled(hint, Style::new().fg(DIM)),
    ])
}

fn draw_settings(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, chunks[0], app, true);
    draw_settings_list(frame, chunks[1], app);
    draw_rule(frame, chunks[2]);

    if let Some((target, line)) = app.settings.edit.as_ref() {
        let label = edit_label(target);
        draw_labeled_input(frame, chunks[3], label, line);
        draw_hint(frame, chunks[4], "enter guardar · esc cancelar");
    } else {
        frame.render_widget(Paragraph::new(""), chunks[3]);
        draw_hint(
            frame,
            chunks[4],
            "enter · e renombrar · n nuevo · d borrar · tab",
        );
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App, config: bool) {
    let area = gutter(area);
    let status = if app.busy { " · …" } else { "" };
    let mode = if config { " · config" } else { "" };
    let right = format!(
        "{} · {}{mode}{status}",
        app.provider_label(),
        app.model_label()
    );
    frame.render_widget(
        Paragraph::new(header_line("leo", &right, area.width as usize)),
        area,
    );
}

fn draw_settings_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let area = gutter(area);
    let rows = SettingsState::rows(&app.snapshot);
    app.settings.clamp(&rows);
    let cursor = app.settings.cursor;
    let width = area.width.max(1) as usize;

    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let selected = i == cursor && row.selectable();
            row_line(row, selected, width)
        })
        .collect();

    let content = lines.len() as u16;
    let max_scroll = content.saturating_sub(area.height);
    let scroll = (cursor as u16).saturating_sub(area.height.saturating_sub(1) / 2);
    let scroll = scroll.min(max_scroll);

    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

fn row_line(row: &Row, selected: bool, width: usize) -> Line<'static> {
    let mark = if selected { "›" } else { " " };
    let (text, style) = match row {
        Row::Header(title) => return section_line(title, width),
        Row::Provider {
            name, kind, active, ..
        } => {
            let star = if *active { "*" } else { " " };
            (
                format!("{star} {name}  {kind}"),
                if *active {
                    Style::new().fg(FG)
                } else {
                    Style::new().fg(USER)
                },
            )
        }
        Row::Model { name, active, .. } => {
            let star = if *active { "*" } else { " " };
            (format!("{star} {name}"), Style::new().fg(FG))
        }
        Row::NewProvider => ("+  nuevo proveedor".into(), Style::new().fg(DIM)),
        Row::NewModel { .. } => ("+  nuevo modelo".into(), Style::new().fg(DIM)),
        Row::Kind { kind, .. } => (field("tipo", kind), Style::new().fg(FG)),
        Row::ApiKey { status, .. } => (field("api key", status), Style::new().fg(FG)),
        Row::BaseUrl { url, .. } => (field("url", url), Style::new().fg(FG)),
        Row::System { preview } => (field("sistema", preview), Style::new().fg(FG)),
    };
    let style = if selected {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    Line::from(Span::styled(occupy(format!("{mark} {text}"), width), style))
}

fn field(label: &str, value: &str) -> String {
    format!("  {label:<LABEL_W$} {value}")
}

fn edit_label(target: &crate::settings::EditTarget) -> &'static str {
    use crate::settings::EditTarget::*;
    match target {
        System => "sistema",
        ApiKey { .. } => "api key",
        BaseUrl { .. } => "url",
        NewProvider | RenameProvider { .. } => "nombre",
        NewModel { .. } | RenameModel { .. } => "modelo",
    }
}

fn draw_messages(frame: &mut Frame, area: Rect, app: &mut App) {
    let area = gutter(area);
    let width = area.width.max(1) as usize;
    let snapshots: Vec<(Kind, String)> = app
        .bubbles
        .iter()
        .map(|b| (b.kind, b.text.clone()))
        .collect();
    let busy = app.busy;

    let mut lines: Vec<Line> = vec![Line::default()];
    if snapshots.is_empty() && !busy {
        lines.push(Line::from(Span::styled(
            "escribe una pregunta",
            Style::new().fg(DIM),
        )));
    }
    for (i, (kind, text)) in snapshots.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }
        lines.extend(bubble_lines(*kind, text, width));
    }
    if busy {
        if lines.len() > 1 {
            lines.push(Line::default());
        }
        lines.push(Line::from(Span::styled(
            "leo",
            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled("  …", Style::new().fg(DIM))));
    }

    let content = lines.len() as u16;
    let max_scroll = content.saturating_sub(area.height);
    app.clamp_scroll(max_scroll);
    let scroll = app.scroll;

    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

fn bubble_lines(kind: Kind, text: &str, width: usize) -> Vec<Line<'static>> {
    let (label, label_style, body_style) = match kind {
        Kind::User => ("tú", Style::new().fg(DIM), Style::new().fg(USER)),
        Kind::Leo => (
            "leo",
            Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            Style::new().fg(FG),
        ),
        Kind::Error => ("error", Style::new().fg(ERR), Style::new().fg(FG)),
    };
    let indent = "  ";
    let inner = width.saturating_sub(indent.chars().count()).max(8);
    let mut lines = vec![Line::from(Span::styled(label, label_style))];
    lines.extend(
        wrap(text, inner)
            .into_iter()
            .map(|chunk| Line::from(vec![Span::raw(indent), Span::styled(chunk, body_style)])),
    );
    lines
}

fn draw_rule(frame: &mut Frame, area: Rect) {
    let area = gutter(area);
    let rule = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(rule, Style::new().fg(RULE))),
        area,
    );
}

fn draw_input(frame: &mut Frame, area: Rect, input: &LineEdit, busy: bool) {
    let prompt = if busy { "  " } else { "› " };
    draw_labeled_input_raw(frame, gutter(area), prompt, input, true);
}

fn draw_labeled_input(frame: &mut Frame, area: Rect, label: &str, input: &LineEdit) {
    let prompt = format!("{label} › ");
    draw_labeled_input_raw(frame, gutter(area), &prompt, input, true);
}

fn draw_labeled_input_raw(
    frame: &mut Frame,
    area: Rect,
    prompt: &str,
    input: &LineEdit,
    show_cursor: bool,
) {
    let line = Line::from(vec![
        Span::styled(prompt.to_string(), Style::new().fg(DIM)),
        Span::styled(input.text.clone(), Style::new().fg(FG)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
    if show_cursor {
        let prefix_cols = prompt.chars().count() as u16;
        let cursor_cols = input.text[..input.cursor].chars().count() as u16;
        let x = area.x + prefix_cols + cursor_cols;
        if x < area.x + area.width {
            frame.set_cursor_position((x, area.y));
        }
    }
}

fn draw_hint(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(Span::styled(text, Style::new().fg(DIM))),
        gutter(area),
    );
}

fn gutter(area: Rect) -> Rect {
    if area.width < 4 {
        return area;
    }
    Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    }
}

fn occupy(text: String, width: usize) -> String {
    let count = text.chars().count();
    if count >= width {
        text.chars().take(width).collect()
    } else {
        let mut out = text;
        out.push_str(&" ".repeat(width - count));
        out
    }
}

fn section_line(title: &str, width: usize) -> Line<'static> {
    let head = format!(" {title} ");
    let fill = width.saturating_sub(head.chars().count());
    Line::from(vec![
        Span::styled(head, Style::new().fg(DIM).add_modifier(Modifier::BOLD)),
        Span::styled("─".repeat(fill), Style::new().fg(RULE)),
    ])
}

fn header_line(brand: &str, right: &str, width: usize) -> Line<'static> {
    let brand_w = brand.chars().count();
    let right_w = right.chars().count();
    let inner = width.saturating_sub(brand_w).saturating_sub(right_w);
    if inner >= 3 {
        Line::from(vec![
            Span::styled(
                brand.to_string(),
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("─".repeat(inner - 2), Style::new().fg(RULE)),
            Span::raw(" "),
            Span::styled(right.to_string(), Style::new().fg(DIM)),
        ])
    } else {
        let pad = inner.min(1);
        let fit: String = right
            .chars()
            .take(width.saturating_sub(brand_w + pad))
            .collect();
        Line::from(vec![
            Span::styled(
                brand.to_string(),
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(pad)),
            Span::styled(fit, Style::new().fg(DIM)),
        ])
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut w = 0usize;
        for word in paragraph.split(' ') {
            let word_w = word.chars().count().max(1);
            if !current.is_empty() && w + 1 + word_w > width {
                lines.push(std::mem::take(&mut current));
                w = 0;
            }
            if !current.is_empty() {
                current.push(' ');
                w += 1;
            }
            if word_w > width {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    w = 0;
                }
                for ch in word.chars() {
                    if w >= width {
                        lines.push(std::mem::take(&mut current));
                        w = 0;
                    }
                    current.push(ch);
                    w += 1;
                }
            } else {
                current.push_str(word);
                w += word_w;
            }
        }
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{header_line, occupy, wrap};

    fn line_text(line: &ratatui::text::Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn wraps_long_words_and_spaces() {
        let lines = wrap("uno dos tres", 7);
        assert_eq!(lines, vec!["uno dos", "tres"]);
        let hard = wrap("abcdefghij", 4);
        assert_eq!(hard, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn occupy_pads_and_truncates() {
        assert_eq!(occupy("hi".into(), 5), "hi   ");
        assert_eq!(occupy("hello world".into(), 5), "hello");
    }

    #[test]
    fn header_fills_between_brand_and_meta() {
        let text = line_text(&header_line("leo", "grok · grok-4.6", 32));
        assert!(text.starts_with("leo "));
        assert!(text.ends_with(" grok · grok-4.6"));
        assert!(text.contains('─'));
        assert_eq!(text.chars().count(), 32);
    }

    #[test]
    fn bubble_puts_speaker_above_body() {
        let lines = super::bubble_lines(crate::app::Kind::User, "hola mundo", 20);
        assert_eq!(line_text(&lines[0]), "tú");
        assert_eq!(line_text(&lines[1]), "  hola mundo");
    }

    fn screen_text(app: &mut crate::app::App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| super::draw(f, app)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn empty_chat_has_header_rule_and_prompt() {
        let mut app = crate::app::App::new(leo_store::stub_snapshot("grok", "grok-4.6", "x"));
        let text = screen_text(&mut app, 48, 12);
        assert!(text.contains("leo"));
        assert!(text.contains("grok · grok-4.6"));
        assert!(text.contains("escribe una pregunta"));
        assert!(text.contains('›'));
        assert!(text.contains("enter · /model"));
    }

    #[test]
    fn settings_uses_section_rules() {
        let mut app = crate::app::App::new(leo_store::stub_snapshot("grok", "grok-4.6", "x"));
        app.screen = crate::app::Screen::Settings;
        let text = screen_text(&mut app, 48, 16);
        assert!(text.contains("proveedores"));
        assert!(text.contains("config"));
        assert!(text.contains("tipo"));
    }

    #[test]
    fn chat_and_slash_layout() {
        let mut app = crate::app::App::new(leo_store::stub_snapshot("grok", "grok-4.6", "x"));
        app.bubbles.push(crate::app::Bubble {
            kind: crate::app::Kind::User,
            text: "hola".into(),
        });
        app.bubbles.push(crate::app::Bubble {
            kind: crate::app::Kind::Leo,
            text: "qué tal".into(),
        });
        let chat = screen_text(&mut app, 48, 14);
        assert!(chat.contains("tú"));
        assert!(chat.contains("  hola"));
        assert!(chat.contains("  qué tal"));

        app.input.paste("/model");
        let slash = screen_text(&mut app, 48, 16);
        assert!(slash.contains("modelos"));
        assert!(slash.contains("›"));
    }
}
