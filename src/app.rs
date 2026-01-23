use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tui_textarea::{Input, TextArea};

use crate::api::{PtruiApi, translate_via_api};
use crate::languages::{LANGUAGES, filtered_language_indices, find_language_index};
use crate::textarea::{set_textarea_text, textarea_input_from_key, textarea_text};
use crate::ui::draw_ui;
use crate::vim::{Mode, Transition, Vim};

const TRANSLATION_DEBOUNCE: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    None,
    Quit,
    NativeizeBoth,
}

pub struct App {
    // Which side currently accepts keyboard input.
    pub active: ActiveSide,
    // Left-side text (English).
    pub input: TextArea<'static>,
    // Right-side text (Spanish).
    pub output: TextArea<'static>,
    left_vim: Vim,
    right_vim: Vim,
    pub left_language: usize,
    pub right_language: usize,
    pub pending_translation: bool,
    last_edit: Option<Instant>,
    pub error: Option<String>,
    pub picker: Option<LanguagePicker>,
}

impl App {
    pub fn new() -> Self {
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

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> AppAction {
        if key.kind != KeyEventKind::Press {
            return AppAction::None;
        }
        if self.picker.is_some() {
            return self.handle_picker_key(key);
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => AppAction::Quit,
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

    pub fn active_mode(&self) -> Mode {
        match self.active {
            ActiveSide::Left => self.left_vim.mode,
            ActiveSide::Right => self.right_vim.mode,
        }
    }
}

pub struct LanguagePicker {
    pub side: ActiveSide,
    pub query: String,
    pub selected: usize,
}

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    let api =
        PtruiApi::from_env().map_err(|message| io::Error::new(io::ErrorKind::Other, message))?;
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

    let left_lang = LANGUAGES.get(app.left_language).unwrap_or(&LANGUAGES[0]);
    let right_lang = LANGUAGES.get(app.right_language).unwrap_or(&LANGUAGES[0]);
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

fn nativeize_both(app: &mut App, api: &PtruiApi) {
    let left_lang = LANGUAGES.get(app.left_language).unwrap_or(&LANGUAGES[0]);
    let right_lang = LANGUAGES.get(app.right_language).unwrap_or(&LANGUAGES[0]);
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
