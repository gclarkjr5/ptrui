use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use serde::{Deserialize, Serialize};
use std::env;
use tui_textarea::{CursorMove, Input, Key, Scrolling, TextArea};

const TRANSLATION_DEBOUNCE: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy)]
struct Language {
    name: &'static str,
    code: &'static str,
}

const LANGUAGES: &[Language] = &[
    Language { name: "English", code: "EN" },
    Language { name: "Spanish", code: "ES" },
    Language { name: "French", code: "FR" },
    Language { name: "German", code: "DE" },
    Language { name: "Italian", code: "IT" },
    Language { name: "Portuguese", code: "PT" },
    Language { name: "Dutch", code: "NL" },
    Language { name: "Polish", code: "PL" },
    Language { name: "Russian", code: "RU" },
    Language { name: "Japanese", code: "JA" },
    Language { name: "Chinese", code: "ZH" },
    Language { name: "Korean", code: "KO" },
    Language { name: "Swedish", code: "SV" },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
    Visual,
    Operator(char),
}

impl Mode {
    fn cursor_style(self) -> Style {
        let color = match self {
            Self::Normal => Color::Reset,
            Self::Insert => Color::LightBlue,
            Self::Visual => Color::LightYellow,
            Self::Operator(_) => Color::LightGreen,
        };
        Style::default().fg(color).add_modifier(Modifier::REVERSED)
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Visual => write!(f, "VISUAL"),
            Self::Operator(c) => write!(f, "OPERATOR({})", c),
        }
    }
}

enum Transition {
    Nop,
    Mode(Mode),
    Pending(Input),
}

struct Vim {
    mode: Mode,
    pending: Input,
}

impl Vim {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            pending: Input::default(),
        }
    }

    fn transition(&self, input: Input, textarea: &mut TextArea<'_>) -> Transition {
        if input.key == Key::Null {
            return Transition::Nop;
        }

        match self.mode {
            Mode::Normal | Mode::Visual | Mode::Operator(_) => {
                match input {
                    Input { key: Key::Char('h'), .. } => textarea.move_cursor(CursorMove::Back),
                    Input { key: Key::Char('j'), .. } => textarea.move_cursor(CursorMove::Down),
                    Input { key: Key::Char('k'), .. } => textarea.move_cursor(CursorMove::Up),
                    Input { key: Key::Char('l'), .. } => textarea.move_cursor(CursorMove::Forward),
                    Input { key: Key::Char('w'), .. } => textarea.move_cursor(CursorMove::WordForward),
                    Input {
                        key: Key::Char('e'),
                        ctrl: false,
                        ..
                    } => {
                        textarea.move_cursor(CursorMove::WordEnd);
                        if matches!(self.mode, Mode::Operator(_)) {
                            textarea.move_cursor(CursorMove::Forward);
                        }
                    }
                    Input {
                        key: Key::Char('b'),
                        ctrl: false,
                        ..
                    } => textarea.move_cursor(CursorMove::WordBack),
                    Input { key: Key::Char('^'), .. } => textarea.move_cursor(CursorMove::Head),
                    Input { key: Key::Char('$'), .. } => textarea.move_cursor(CursorMove::End),
                    Input { key: Key::Char('D'), .. } => {
                        textarea.delete_line_by_end();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input { key: Key::Char('C'), .. } => {
                        textarea.delete_line_by_end();
                        textarea.cancel_selection();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input { key: Key::Char('p'), .. } => {
                        textarea.paste();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('u'),
                        ctrl: false,
                        ..
                    } => {
                        textarea.undo();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('r'),
                        ctrl: true,
                        ..
                    } => {
                        textarea.redo();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input { key: Key::Char('x'), .. } => {
                        textarea.delete_next_char();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input { key: Key::Char('i'), .. } => {
                        textarea.cancel_selection();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input { key: Key::Char('a'), .. } => {
                        textarea.cancel_selection();
                        textarea.move_cursor(CursorMove::Forward);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input { key: Key::Char('A'), .. } => {
                        textarea.cancel_selection();
                        textarea.move_cursor(CursorMove::End);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input { key: Key::Char('o'), .. } => {
                        textarea.move_cursor(CursorMove::End);
                        textarea.insert_newline();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input { key: Key::Char('O'), .. } => {
                        textarea.move_cursor(CursorMove::Head);
                        textarea.insert_newline();
                        textarea.move_cursor(CursorMove::Up);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input { key: Key::Char('I'), .. } => {
                        textarea.cancel_selection();
                        textarea.move_cursor(CursorMove::Head);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('e'),
                        ctrl: true,
                        ..
                    } => textarea.scroll((1, 0)),
                    Input {
                        key: Key::Char('y'),
                        ctrl: true,
                        ..
                    } => textarea.scroll((-1, 0)),
                    Input {
                        key: Key::Char('d'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::HalfPageDown),
                    Input {
                        key: Key::Char('u'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::HalfPageUp),
                    Input {
                        key: Key::Char('f'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::PageDown),
                    Input {
                        key: Key::Char('b'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::PageUp),
                    Input {
                        key: Key::Char('v'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Normal => {
                        textarea.start_selection();
                        return Transition::Mode(Mode::Visual);
                    }
                    Input {
                        key: Key::Char('V'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Normal => {
                        textarea.move_cursor(CursorMove::Head);
                        textarea.start_selection();
                        textarea.move_cursor(CursorMove::End);
                        return Transition::Mode(Mode::Visual);
                    }
                    Input { key: Key::Esc, .. }
                    | Input {
                        key: Key::Char('v'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.cancel_selection();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('g'),
                        ctrl: false,
                        ..
                    } if matches!(
                        self.pending,
                        Input {
                            key: Key::Char('g'),
                            ctrl: false,
                            ..
                        }
                    ) =>
                    {
                        textarea.move_cursor(CursorMove::Top)
                    }
                    Input {
                        key: Key::Char('G'),
                        ctrl: false,
                        ..
                    } => textarea.move_cursor(CursorMove::Bottom),
                    Input {
                        key: Key::Char(c),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Operator(c) => {
                        textarea.move_cursor(CursorMove::Head);
                        textarea.start_selection();
                        let cursor = textarea.cursor();
                        textarea.move_cursor(CursorMove::Down);
                        if cursor == textarea.cursor() {
                            textarea.move_cursor(CursorMove::End);
                        }
                    }
                    Input {
                        key: Key::Char(op @ ('y' | 'd' | 'c')),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Normal => {
                        textarea.start_selection();
                        return Transition::Mode(Mode::Operator(op));
                    }
                    Input {
                        key: Key::Char('y'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.move_cursor(CursorMove::Forward);
                        textarea.copy();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('d'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.move_cursor(CursorMove::Forward);
                        textarea.cut();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('c'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.move_cursor(CursorMove::Forward);
                        textarea.cut();
                        return Transition::Mode(Mode::Insert);
                    }
                    input => return Transition::Pending(input),
                }

                match self.mode {
                    Mode::Operator('y') => {
                        textarea.copy();
                        Transition::Mode(Mode::Normal)
                    }
                    Mode::Operator('d') => {
                        textarea.cut();
                        Transition::Mode(Mode::Normal)
                    }
                    Mode::Operator('c') => {
                        textarea.cut();
                        Transition::Mode(Mode::Insert)
                    }
                    _ => Transition::Nop,
                }
            }
            Mode::Insert => match input {
                Input { key: Key::Esc, .. }
                | Input {
                    key: Key::Char('c'),
                    ctrl: true,
                    ..
                } => Transition::Mode(Mode::Normal),
                input => {
                    textarea.input(input);
                    Transition::Mode(Mode::Insert)
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppAction {
    None,
    Quit,
    NativeizeBoth,
}

struct App {
    // Which side currently accepts keyboard input.
    active: ActiveSide,
    // Left-side text (English).
    input: TextArea<'static>,
    // Right-side text (Spanish).
    output: TextArea<'static>,
    left_vim: Vim,
    right_vim: Vim,
    left_language: usize,
    right_language: usize,
    pending_translation: bool,
    last_edit: Option<Instant>,
    error: Option<String>,
    picker: Option<LanguagePicker>,
}

impl App {
    fn new() -> Self {
        let left_language = find_language_index("EN").unwrap_or(0);
        let right_language = find_language_index("ES").unwrap_or(1);
        Self {
            active: ActiveSide::Left,
            input: TextArea::default(),
            output: TextArea::default(),
            left_vim: Vim::new(Mode::Normal),
            right_vim: Vim::new(Mode::Normal),
            left_language,
            right_language,
            pending_translation: false,
            last_edit: None,
            error: None,
            picker: None,
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> AppAction {
        if key.kind != KeyEventKind::Press {
            return AppAction::None;
        }
        if self.picker.is_some() {
            return self.handle_picker_key(key);
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                AppAction::Quit
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_picker(ActiveSide::Left);
                AppAction::None
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_picker(ActiveSide::Right);
                AppAction::None
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                AppAction::NativeizeBoth
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match self.active {
                    ActiveSide::Left => self.input = TextArea::default(),
                    ActiveSide::Right => self.output = TextArea::default(),
                }
                schedule_translation(self);
                AppAction::None
            }
            KeyCode::Tab => {
                // Switch which side gets input.
                self.active = match self.active {
                    ActiveSide::Left => ActiveSide::Right,
                    ActiveSide::Right => ActiveSide::Left,
                };
                AppAction::None
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_picker(ActiveSide::Left);
                AppAction::None
            }
            _ => {
                let input = textarea_input_from_key(key);
                let modified = match self.active {
                    ActiveSide::Left => {
                        let before = textarea_text(&self.input);
                        let transition = self.left_vim.transition(input, &mut self.input);
                        self.update_vim_state(ActiveSide::Left, transition);
                        before != textarea_text(&self.input)
                    }
                    ActiveSide::Right => {
                        let before = textarea_text(&self.output);
                        let transition = self.right_vim.transition(input, &mut self.output);
                        self.update_vim_state(ActiveSide::Right, transition);
                        before != textarea_text(&self.output)
                    }
                };
                if modified {
                    schedule_translation(self);
                }
                AppAction::None
            }
        }
    }

    fn open_picker(&mut self, side: ActiveSide) {
        self.picker = Some(LanguagePicker {
            side,
            query: String::new(),
            selected: 0,
        });
    }

    fn handle_picker_key(&mut self, key: crossterm::event::KeyEvent) -> AppAction {
        let Some(picker) = self.picker.as_mut() else {
            return AppAction::None;
        };
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return AppAction::Quit;
            }
            KeyCode::Esc => {
                self.picker = None;
            }
            KeyCode::Enter => {
                let indices = filtered_language_indices(&picker.query);
                if let Some(&language_index) = indices.get(picker.selected) {
                    match picker.side {
                        ActiveSide::Left => self.left_language = language_index,
                        ActiveSide::Right => self.right_language = language_index,
                    }
                    schedule_translation(self);
                }
                self.picker = None;
            }
            KeyCode::Up => {
                if picker.selected > 0 {
                    picker.selected -= 1;
                }
            }
            KeyCode::Down => {
                let indices = filtered_language_indices(&picker.query);
                if !indices.is_empty() && picker.selected + 1 < indices.len() {
                    picker.selected += 1;
                }
            }
            KeyCode::Backspace => {
                picker.query.pop();
                picker.selected = 0;
            }
            KeyCode::Char(c) => {
                if !c.is_control() && picker.query.len() < 32 {
                    picker.query.push(c);
                    picker.selected = 0;
                }
            }
            _ => {}
        }
        AppAction::None
    }

    fn update_vim_state(&mut self, side: ActiveSide, transition: Transition) {
        let vim = match side {
            ActiveSide::Left => &mut self.left_vim,
            ActiveSide::Right => &mut self.right_vim,
        };
        match transition {
            Transition::Nop => {}
            Transition::Pending(input) => vim.pending = input,
            Transition::Mode(mode) => {
                vim.mode = mode;
                vim.pending = Input::default();
            }
        }
    }

    fn active_mode(&self) -> Mode {
        match self.active {
            ActiveSide::Left => self.left_vim.mode,
            ActiveSide::Right => self.right_vim.mode,
        }
    }
}

fn main() -> io::Result<()> {
    // Raw mode lets us read keys directly without line buffering.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // Switch to an alternate screen so we can draw a TUI.
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    // Always restore the terminal to a clean state.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    let api = PtruiApi::from_env()
        .map_err(|message| io::Error::new(io::ErrorKind::Other, message))?;
    let poll_rate = Duration::from_millis(100);

    loop {
        // Redraw the UI every loop iteration.
        terminal.draw(|frame| draw_ui(frame, &app))?;

        // Poll for input; this keeps the UI responsive.
        if event::poll(poll_rate)? {
            if let Event::Key(key) = event::read()? {
                match app.handle_key(key) {
                    AppAction::Quit => return Ok(()),
                    AppAction::NativeizeBoth => nativeize_both(&mut app, &api),
                    AppAction::None => {}
                }
            }
        }
        maybe_translate(&mut app, &api);
    }
}

fn draw_ui(frame: &mut ratatui::Frame, app: &App) {
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

    draw_header(frame, chunks[0], app);
    draw_translator(frame, chunks[1], app);
    draw_help(frame, chunks[2], app);

    if app.picker.is_some() {
        draw_language_picker(frame, app);
    }
}

fn draw_header(frame: &mut ratatui::Frame, area: Rect, _app: &App) {
    // Header shows app name and a small hint.
    let title = Line::from(vec![
        Span::styled("ptrui", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  |  "),
        Span::styled(
            "tab to switch",
            Style::default().fg(Color::Green),
        ),
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

    let left_language = LANGUAGES
        .get(app.left_language)
        .unwrap_or(&LANGUAGES[0]);
    let right_language = LANGUAGES
        .get(app.right_language)
        .unwrap_or(&LANGUAGES[0]);
    let left_title = match app.active {
        ActiveSide::Left => format!("{} (active, {})", left_language.name, app.active_mode()),
        ActiveSide::Right => left_language.name.to_string(),
    };
    let right_title = match app.active {
        ActiveSide::Left => right_language.name.to_string(),
        ActiveSide::Right => format!("{} (active, {})", right_language.name, app.active_mode()),
    };
    let text_style = Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
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

#[derive(Debug, Serialize)]
struct TranslateRequest<'a> {
    text: Vec<&'a str>,
    source_lang: &'a str,
    target_lang: &'a str,
}

#[derive(Debug, Deserialize)]
struct TranslateResponse {
    translations: Vec<TranslationItem>,
}

#[derive(Debug, Deserialize)]
struct TranslationItem {
    text: String,
}

struct PtruiApi {
    client: reqwest::blocking::Client,
    url: String,
    auth_header: Option<String>,
    auth_value: Option<String>,
}

impl PtruiApi {
    fn from_env() -> Result<Self, String> {
        let url = env::var("TRANSLATION_API_URL")
            .map_err(|_| "Missing TRANSLATION_API_URL environment variable".to_string())?;
        let auth_key = env::var("TRANSLATION_API_KEY").ok();
        let auth_header = env::var("TRANSLATION_API_AUTH_HEADER").ok();

        let (header_name, header_value) = match auth_key {
            Some(key) => {
                let header = auth_header.unwrap_or_else(|| "Authorization".to_string());
                let value = if header.eq_ignore_ascii_case("Authorization") {
                    format!("DeepL-Auth-Key {}", key)
                } else {
                    key
                };
                (Some(header), Some(value))
            }
            None => (None, None),
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|err| format!("Failed to build HTTP client: {}", err))?;

        Ok(Self {
            client,
            url,
            auth_header: header_name,
            auth_value: header_value,
        })
    }
}

struct LanguagePicker {
    side: ActiveSide,
    query: String,
    selected: usize,
}

fn find_language_index(code: &str) -> Option<usize> {
    LANGUAGES
        .iter()
        .position(|language| language.code.eq_ignore_ascii_case(code))
}

fn filtered_language_indices(query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..LANGUAGES.len()).collect();
    }
    let mut matches: Vec<(usize, usize)> = Vec::new();
    for (index, language) in LANGUAGES.iter().enumerate() {
        let candidate = format!(
            "{} {}",
            language.name.to_ascii_lowercase(),
            language.code.to_ascii_lowercase()
        );
        if let Some(score) = fuzzy_score(query, &candidate) {
            matches.push((score, index));
        }
    }
    matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| LANGUAGES[a.1].name.cmp(LANGUAGES[b.1].name)));
    matches.into_iter().map(|(_, index)| index).collect()
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let mut score = 0usize;
    let mut last_index = 0usize;
    let query_lower = query.to_ascii_lowercase();
    for needle in query_lower.chars() {
        if let Some(found) = candidate[last_index..].find(needle) {
            score += found;
            last_index += found + 1;
        } else {
            return None;
        }
    }
    Some(score)
}

fn schedule_translation(app: &mut App) {
    app.pending_translation = true;
    app.last_edit = Some(Instant::now());
    app.error = None;
}

fn maybe_translate(app: &mut App, api: &PtruiApi) {
    if !app.pending_translation {
        return;
    }
    let Some(last_edit) = app.last_edit else {
        return;
    };
    if last_edit.elapsed() < TRANSLATION_DEBOUNCE {
        return;
    }

    let left_lang = LANGUAGES
        .get(app.left_language)
        .unwrap_or(&LANGUAGES[0]);
    let right_lang = LANGUAGES
        .get(app.right_language)
        .unwrap_or(&LANGUAGES[0]);
    let (source_text, source_lang, target_lang, target_slot) = match app.active {
        ActiveSide::Left => (
            textarea_text(&app.input),
            left_lang.code,
            right_lang.code,
            &mut app.output,
        ),
        ActiveSide::Right => (
            textarea_text(&app.output),
            right_lang.code,
            left_lang.code,
            &mut app.input,
        ),
    };

    if source_text.trim().is_empty() {
        set_textarea_text(target_slot, "");
        app.pending_translation = false;
        return;
    }

    match translate_via_api(api, &source_text, source_lang, target_lang) {
        Ok(translated) => {
            set_textarea_text(target_slot, &translated);
            app.error = None;
        }
        Err(message) => {
            app.error = Some(message);
        }
    }

    app.pending_translation = false;
}

fn translate_via_api(
    api: &PtruiApi,
    text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<String, String> {
    let payload = TranslateRequest {
        text: vec![text],
        source_lang,
        target_lang,
    };
    let mut request = api.client.post(&api.url).json(&payload);
    if let (Some(header), Some(value)) = (&api.auth_header, &api.auth_value) {
        request = request.header(header, value);
        // println!("Request: {:?}", request);
        
    }
    let response = request
        .send()
        .map_err(|err| format!("Failed to call translation API: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Translation API error ({}): {}", status, body));
    }

    let response: TranslateResponse = response
        .json()
        .map_err(|err| format!("Invalid API response: {}", err))?;
    response
        .translations
        .into_iter()
        .next()
        .map(|item| item.text)
        .ok_or_else(|| "API response missing translations".to_string())
}

fn nativeize_both(app: &mut App, api: &PtruiApi) {
    let left_lang = LANGUAGES
        .get(app.left_language)
        .unwrap_or(&LANGUAGES[0]);
    let right_lang = LANGUAGES
        .get(app.right_language)
        .unwrap_or(&LANGUAGES[0]);
    let left_source = textarea_text(&app.input);
    let right_source = textarea_text(&app.output);
    if left_source.trim().is_empty() && right_source.trim().is_empty() {
        return;
    }

    let mut new_left = left_source.clone();
    let mut new_right = right_source.clone();
    let mut error_message = None;

    if !left_source.trim().is_empty() {
        match translate_via_api(api, &left_source, left_lang.code, right_lang.code) {
            Ok(translated) => new_right = translated,
            Err(message) => error_message = Some(message),
        }
    }
    if !right_source.trim().is_empty() {
        match translate_via_api(api, &right_source, right_lang.code, left_lang.code) {
            Ok(translated) => new_left = translated,
            Err(message) => {
                if error_message.is_none() {
                    error_message = Some(message);
                }
            }
        }
    }

    set_textarea_text(&mut app.input, &new_left);
    set_textarea_text(&mut app.output, &new_right);
    app.error = error_message;
    app.pending_translation = false;
    app.last_edit = None;
}

fn textarea_text(textarea: &TextArea) -> String {
    textarea.lines().join("\n")
}

fn textarea_input_from_key(key: crossterm::event::KeyEvent) -> Input {
    let key_code = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        KeyCode::F(n) => Key::F(n),
        _ => Key::Null,
    };

    Input {
        key: key_code,
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
    }
}

fn set_textarea_text(textarea: &mut TextArea, text: &str) {
    *textarea = TextArea::from(text.lines());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn tab_switches_active_side() {
        let mut app = App::new();
        assert_eq!(app.active, ActiveSide::Left);
        app.handle_key(press(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.active, ActiveSide::Right);
    }

    #[test]
    fn ctrl_c_requests_quit() {
        let mut app = App::new();
        let action = app.handle_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(action, AppAction::Quit);
    }

    #[test]
    fn ctrl_r_clears_active_side() {
        let mut app = App::new();
        app.input = TextArea::from(["hello"]);
        app.handle_key(press(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(textarea_text(&app.input), "");
        assert!(app.pending_translation);
    }

    #[test]
    fn typing_schedules_translation_left_to_right() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Char('i'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('e'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('l'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('l'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('o'), KeyModifiers::NONE));
        assert_eq!(textarea_text(&app.input), "hello");
        assert!(app.pending_translation);
    }

    #[test]
    fn typing_schedules_translation_right_to_left() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Tab, KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('i'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('o'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('l'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(textarea_text(&app.output), "hola");
        assert!(app.pending_translation);
    }

    #[test]
    fn backspace_schedules_translation() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Char('i'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('e'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('l'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('l'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Char('o'), KeyModifiers::NONE));
        app.handle_key(press(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(textarea_text(&app.input), "hell");
        assert!(app.pending_translation);
    }
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
