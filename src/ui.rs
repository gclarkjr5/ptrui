use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{ActiveSide, App};
use crate::languages::{filtered_language_indices, LANGUAGES};

pub fn draw_ui(frame: &mut ratatui::Frame, app: &App) {
    // The screen is vertically split into a header, app, and controls.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(5),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0]);
    draw_translator(frame, chunks[1], app);
    draw_help(frame, chunks[2], app);

    if app.picker.is_some() {
        draw_language_picker(frame, app);
    }
}

fn draw_header(frame: &mut ratatui::Frame, area: Rect) {
    // Header shows app name and a small hint.
    let title = Line::from(vec![
        Span::styled("ptrui", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  |  "),
        Span::styled("tab to switch", Style::default().fg(Color::Green)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::White));

    frame.render_widget(block, area);
}

fn draw_translator(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    // Two equal columns: English (left) and Spanish (right).
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_language = LANGUAGES.get(app.left_language).unwrap_or(&LANGUAGES[0]);
    let right_language = LANGUAGES.get(app.right_language).unwrap_or(&LANGUAGES[0]);
    let left_title = match app.active {
        ActiveSide::Left => format!("{} (active, {})", left_language.name, app.active_mode()),
        ActiveSide::Right => left_language.name.to_string(),
    };
    let right_title = match app.active {
        ActiveSide::Left => right_language.name.to_string(),
        ActiveSide::Right => format!("{} (active, {})", right_language.name, app.active_mode()),
    };
    let text_style = Style::default()
        .fg(Color::LightBlue)
        .add_modifier(Modifier::BOLD);
    let left_block = Block::default()
        .borders(Borders::ALL)
        .title(left_title)
        .border_style(match app.active {
            ActiveSide::Left => Style::default().fg(Color::Cyan),
            ActiveSide::Right => Style::default(),
        });
    let mut left = app.input.clone();
    left.set_block(left_block);
    left.set_style(text_style);
    if app.active == ActiveSide::Left {
        left.set_cursor_style(app.active_mode().cursor_style());
        left.set_cursor_line_style(Style::default().fg(Color::Cyan));
    } else {
        left.set_cursor_style(text_style);
        left.set_cursor_line_style(Style::default());
    }
    frame.render_widget(&left, columns[0]);

    let right_block = Block::default()
        .borders(Borders::ALL)
        .title(right_title)
        .border_style(match app.active {
            ActiveSide::Right => Style::default().fg(Color::Cyan),
            ActiveSide::Left => Style::default(),
        });
    let mut right = app.output.clone();
    right.set_block(right_block);
    right.set_style(text_style);
    if app.active == ActiveSide::Right {
        right.set_cursor_style(app.active_mode().cursor_style());
        right.set_cursor_line_style(Style::default().fg(Color::Cyan));
    } else {
        right.set_cursor_style(text_style);
        right.set_cursor_line_style(Style::default());
    }
    frame.render_widget(&right, columns[1]);
}

fn draw_help(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            Span::styled("Ctrl+c", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  quit"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+h", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  change left language"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+l", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  change right language"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+n", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  native-ize both"),
        ]),
        Line::from(vec![
            Span::styled("Ctrl+r", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  clear active"),
        ]),
        Line::from(vec![
            Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  switch side"),
        ]),
        Line::from(vec![
            Span::styled("Vim", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  i/a/o insert, Esc normal, hjkl move"),
        ]),
        Line::from(vec![
            Span::styled("Status", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            match &app.error {
                Some(message) => Span::styled(message.as_str(), Style::default().fg(Color::Red)),
                None if app.pending_translation => {
                    Span::styled("translating...", Style::default().fg(Color::Yellow))
                }
                None => Span::styled("ready", Style::default().fg(Color::Green)),
            },
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn draw_language_picker(frame: &mut ratatui::Frame, app: &App) {
    let Some(picker) = &app.picker else {
        return;
    };
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let title = match picker.side {
        ActiveSide::Left => "Select source language",
        ActiveSide::Right => "Select target language",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let query = Paragraph::new(Line::from(vec![
        Span::styled("Search: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(picker.query.as_str()),
    ]))
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true });
    frame.render_widget(query, rows[0]);

    let indices = filtered_language_indices(&picker.query);
    let items: Vec<ListItem> = indices
        .iter()
        .map(|&index| {
            let language = LANGUAGES.get(index).unwrap_or(&LANGUAGES[0]);
            ListItem::new(format!("{} ({})", language.name, language.code))
        })
        .collect();

    let mut state = ListState::default();
    if !indices.is_empty() {
        let selected = picker.selected.min(indices.len().saturating_sub(1));
        state.select(Some(selected));
    }

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, rows[1], &mut state);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" select  "),
        Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" cancel  "),
        Span::styled("Up/Down", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" navigate"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, rows[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);

    horizontal[1]
}
