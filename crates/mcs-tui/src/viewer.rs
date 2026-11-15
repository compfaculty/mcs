use super::{display_text, App, Block, Color, KeyCode, KeyEvent, KeyModifiers, Paragraph, Style};
use mcs_viewer::{Action, Client, Event, Movement, Page, Viewport};
use ratatui::{
    layout::{Constraint, Layout},
    text::{Line, Span},
};
use std::path::PathBuf;

#[derive(Default)]
pub enum Screen {
    #[default]
    Browser,
    Loading,
    Text(Page),
    Media(Box<super::graphics::Frame>),
    Playback(Box<mcs_media::video::Playback>),
    Error(String),
}
pub struct ViewerUi {
    pub screen: Screen,
    pub path: PathBuf,
    pub viewport: Viewport,
    pub numbers: bool,
    pub search: Option<String>,
    pub query: String,
    pub client: Option<Client>,
    pub font: Option<ratatui_image::FontSize>,
    pub compression: bool,
    graphics: Option<super::graphics::Client>,
    kind: Option<mcs_media::Kind>,
    zoom: i8,
    pan_x: u32,
    pan_y: u32,
    columns: u16,
    rows: u16,
    delete: Vec<u32>,
    progress: String,
}
impl Default for ViewerUi {
    fn default() -> Self {
        Self {
            screen: Screen::Browser,
            path: PathBuf::new(),
            viewport: Viewport::default(),
            numbers: true,
            search: None,
            query: String::new(),
            client: None,
            font: None,
            compression: false,
            graphics: None,
            kind: None,
            zoom: 0,
            pan_x: 0,
            pan_y: 0,
            columns: 80,
            rows: 20,
            delete: Vec::new(),
            progress: String::new(),
        }
    }
}
impl ViewerUi {
    pub fn busy(&self) -> bool {
        self.client.as_ref().is_some_and(|client| client.busy)
            || self.graphics.as_ref().is_some_and(|client| client.busy)
    }
    pub fn open(&mut self, path: PathBuf) {
        self.clear_placement();
        self.path = path;
        self.kind = None;
        self.zoom = 0;
        self.pan_x = 0;
        self.pan_y = 0;
        self.progress.clear();
        if let Some(client) = &mut self.graphics {
            client.cancel();
        }
        self.search = None;
        self.query.clear();
        self.screen = Screen::Loading;
        if self.client.is_none() {
            match Client::new() {
                Ok(client) => self.client = Some(client),
                Err(error) => {
                    self.screen = Screen::Error(error.to_string());
                    return;
                }
            }
        }
        self.client
            .as_mut()
            .expect("created client")
            .submit(Action::Open(self.path.clone(), self.viewport));
    }
    pub fn poll(&mut self) -> bool {
        if let Some(event) = self.client.as_mut().and_then(Client::poll) {
            if !matches!(self.screen, Screen::Browser) {
                self.screen = match event {
                    Event::Page(page) => {
                        self.progress.clear();
                        Screen::Text(page)
                    }
                    Event::Progress { scanned, total } => {
                        self.progress = format!(
                            "Search {}%",
                            scanned
                                .saturating_mul(100)
                                .checked_div(total)
                                .unwrap_or(100)
                        );
                        std::mem::take(&mut self.screen)
                    }
                    Event::Error(error) => Screen::Error(error),
                    Event::Video(playback) => {
                        if self.font.is_some() {
                            Screen::Playback(Box::new(playback))
                        } else {
                            Screen::Error("Video requires a confirmed Kitty terminal".into())
                        }
                    }
                    Event::Media(kind) => {
                        self.kind = Some(kind);
                        if self.font.is_none() {
                            Screen::Error("Media requires a confirmed Kitty graphics terminal; text viewing remains available.".into())
                        } else {
                            self.request_media(0, None, true);
                            std::mem::take(&mut self.screen)
                        }
                    }
                };
                if let Screen::Text(page) = &self.screen {
                    if page.viewport != self.viewport {
                        self.navigate(Movement::Stay);
                    }
                }
                return true;
            }
        }
        if let Some(event) = self
            .graphics
            .as_mut()
            .and_then(super::graphics::Client::poll)
        {
            if !matches!(self.screen, Screen::Browser) {
                match event {
                    super::graphics::Event::Frame(frame) => {
                        self.clear_placement();
                        let resized = frame.columns != self.columns || frame.rows != self.rows;
                        self.screen = Screen::Media(Box::new(frame));
                        self.progress.clear();
                        if resized {
                            self.request_media(0, None, false);
                        }
                    }
                    super::graphics::Event::Progress(page) => {
                        self.progress = format!("Searching PDF page {page}")
                    }
                    super::graphics::Event::Error(error) => {
                        self.clear_placement();
                        self.screen = Screen::Error(error);
                    }
                }
                return true;
            }
        }
        false
    }
    fn clear_placement(&mut self) {
        if let Screen::Media(frame) = &self.screen {
            self.delete.push(frame.id);
        }
    }
    pub fn flush_graphics(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        for id in self.delete.drain(..) {
            write!(stdout, "\x1b_Ga=d,d=I,i={id},q=2;\x1b\\")?;
        }
        stdout.flush()
    }
    fn request_media(
        &mut self,
        page_delta: i32,
        search: Option<(String, bool, bool)>,
        reload: bool,
    ) {
        let Some(font) = self.font else {
            return;
        };
        let Some(kind) = self.kind else {
            return;
        };
        if self.graphics.is_none() {
            match super::graphics::Client::new() {
                Ok(client) => self.graphics = Some(client),
                Err(error) => {
                    self.screen = Screen::Error(error.to_string());
                    return;
                }
            }
        }
        let page = if let Screen::Media(frame) = &self.screen {
            frame.page
        } else {
            1
        };
        self.graphics
            .as_mut()
            .expect("graphics client")
            .submit(super::graphics::Job {
                compression: self.compression,
                path: self.path.clone(),
                kind,
                page,
                page_delta,
                view: mcs_media::ImageView {
                    width: u32::from(self.columns) * u32::from(font.width),
                    height: u32::from(self.rows) * u32::from(font.height),
                    zoom: self.zoom,
                    pan_x: self.pan_x,
                    pan_y: self.pan_y,
                },
                columns: self.columns,
                rows: self.rows,
                font,
                search,
                reload,
            });
    }
    pub fn close(&mut self) {
        if let Some(client) = &mut self.client {
            client.cancel();
        }
        if let Some(client) = &mut self.graphics {
            client.cancel();
        }
        self.clear_placement();
        self.screen = Screen::Browser;
        self.search = None;
    }
    pub fn navigate(&mut self, movement: Movement) {
        if matches!(self.screen, Screen::Media(_)) {
            let (delta, search) = match movement {
                Movement::Rows(delta) => (delta.clamp(-1, 1) as i32, None),
                Movement::Home => (-100_000, None),
                Movement::End => (100_000, None),
                Movement::Search { query, reverse } => (0, Some((query, reverse, false))),
                _ => (0, None),
            };
            self.request_media(delta, search, false);
            return;
        }
        if let Screen::Text(page) = &self.screen {
            if let Some(client) = &mut self.client {
                client.submit(Action::View {
                    top: page.top,
                    movement,
                    viewport: self.viewport,
                });
            }
        }
    }
    pub fn resize(&mut self, width: u16, height: u16) {
        self.columns = width.saturating_sub(2).max(1);
        self.rows = height.saturating_sub(5).max(1);
        self.viewport.width =
            usize::from(width.saturating_sub(if self.numbers { 11 } else { 2 })).max(1);
        self.viewport.height = usize::from(height.saturating_sub(5)).max(1);
        self.navigate(Movement::Stay);
    }
    pub fn key(&mut self, key: KeyEvent) {
        if let Some(input) = &mut self.search {
            match key.code {
                KeyCode::Esc => self.search = None,
                KeyCode::Enter => {
                    let query = self.search.take().expect("search input");
                    if !query.is_empty() {
                        self.query = query.clone();
                        self.viewport.horizontal = 0;
                        self.navigate(Movement::Search {
                            query,
                            reverse: false,
                        });
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch)
                    if !ch.is_control()
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && input.chars().count() < 256 =>
                {
                    input.push(ch);
                }
                _ => {}
            }
            return;
        }
        if matches!(self.screen, Screen::Media(_)) && self.media_key(key) {
            return;
        }
        match key.code {
            KeyCode::Esc if self.busy() => {
                if let Some(client) = &mut self.client {
                    client.cancel();
                }
                if let Some(client) = &mut self.graphics {
                    client.cancel();
                }
                self.progress = "Cancelled".into();
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(3) => self.close(),
            KeyCode::Char('r') => self.open(self.path.clone()),
            KeyCode::Char('/') => self.search = Some(String::new()),
            KeyCode::Char('n' | 'N') if !self.query.is_empty() => self.navigate(Movement::Search {
                query: self.query.clone(),
                reverse: key.code == KeyCode::Char('N'),
            }),
            KeyCode::Up | KeyCode::Char('k') => self.navigate(Movement::Rows(-1)),
            KeyCode::Down | KeyCode::Char('j') => self.navigate(Movement::Rows(1)),
            KeyCode::PageUp => self.navigate(Movement::Rows(-(self.viewport.height as isize))),
            KeyCode::PageDown => self.navigate(Movement::Rows(self.viewport.height as isize)),
            KeyCode::Home => self.navigate(Movement::Home),
            KeyCode::End => self.navigate(Movement::End),
            KeyCode::Left => {
                self.viewport.horizontal = self.viewport.horizontal.saturating_sub(8);
                self.navigate(Movement::Stay);
            }
            KeyCode::Right => {
                self.viewport.horizontal =
                    (self.viewport.horizontal + 8).min(mcs_viewer::text::SEGMENT);
                self.navigate(Movement::Stay);
            }
            KeyCode::Char('w') => {
                self.viewport.wrap = !self.viewport.wrap;
                self.navigate(Movement::Stay);
            }
            KeyCode::Char('l') => {
                self.numbers = !self.numbers;
                self.viewport.width = if self.numbers {
                    self.viewport.width.saturating_sub(9).max(1)
                } else {
                    self.viewport.width + 9
                };
                self.navigate(Movement::Stay);
            }
            _ => {}
        }
    }
    fn media_key(&mut self, key: KeyEvent) -> bool {
        let pdf = self.kind == Some(mcs_media::Kind::Pdf);
        let mut delta = 0;
        let mut search = None;
        match key.code {
            KeyCode::Char('+') | KeyCode::Char('=') => self.zoom = (self.zoom + 1).min(6),
            KeyCode::Char('-') => self.zoom = (self.zoom - 1).max(-4),
            KeyCode::Char('0') => {
                self.zoom = 0;
                self.pan_x = 0;
                self.pan_y = 0;
            }
            KeyCode::Left => self.pan_x = self.pan_x.saturating_sub(64),
            KeyCode::Right => self.pan_x = (self.pan_x + 64).min(16_384),
            KeyCode::Up => self.pan_y = self.pan_y.saturating_sub(64),
            KeyCode::Down => self.pan_y = (self.pan_y + 64).min(16_384),
            KeyCode::PageDown | KeyCode::Char(']') if pdf => {
                delta = 1;
                self.pan_y = 0;
                self.pan_x = 0;
            }
            KeyCode::PageUp | KeyCode::Char('[') if pdf => {
                delta = -1;
                self.pan_y = 0;
                self.pan_x = 0;
            }
            KeyCode::Home if pdf => delta = -100_000,
            KeyCode::End if pdf => delta = 100_000,
            KeyCode::Char('n' | 'N') if pdf && !self.query.is_empty() => {
                search = Some((self.query.clone(), key.code == KeyCode::Char('N'), true))
            }
            KeyCode::Char('/') if !pdf => return true,
            KeyCode::Char('w' | 'l') => return true,
            _ => return false,
        }
        self.request_media(delta, search, false);
        true
    }
    pub fn draw(&self, frame: &mut ratatui::Frame) {
        let areas = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(frame.area());
        frame.render_widget(
            Paragraph::new(format!(
                "F3 viewer | {}",
                display_text(&self.path.to_string_lossy())
            )),
            areas[0],
        );
        let status = match &self.screen {
            Screen::Text(page) => {
                let lines: Vec<_> = page
                    .rows
                    .iter()
                    .map(|row| {
                        let mut spans = Vec::new();
                        if self.numbers {
                            spans.push(Span::styled(
                                format!(
                                    "{:>7}{} ",
                                    row.line
                                        .map(|line| line.to_string())
                                        .unwrap_or_else(|| "?".into()),
                                    if row.continued { '+' } else { ' ' }
                                ),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                        let query = display_text(&page.query);
                        if query.is_empty() {
                            spans.push(Span::raw(row.text.clone()));
                        } else {
                            let mut rest = row.text.as_str();
                            while let Some(index) = rest.find(&query) {
                                spans.push(Span::raw(rest[..index].to_owned()));
                                spans.push(Span::styled(
                                    query.clone(),
                                    Style::default().fg(Color::Black).bg(Color::Yellow),
                                ));
                                rest = &rest[index + query.len()..];
                            }
                            spans.push(Span::raw(rest.to_owned()));
                        }
                        Line::from(spans)
                    })
                    .collect();
                frame.render_widget(Paragraph::new(lines).block(Block::bordered()), areas[1]);
                format!(
                    "byte {}/{} | index {}% | wrap:{} | {} {}",
                    page.top,
                    page.len,
                    page.indexed
                        .saturating_mul(100)
                        .checked_div(page.len)
                        .unwrap_or(100),
                    self.viewport.wrap,
                    display_text(&page.message),
                    display_text(&self.progress)
                )
            }
            Screen::Media(image) => {
                let inner = Block::bordered().inner(areas[1]);
                frame.render_widget(Block::bordered(), areas[1]);
                frame.render_widget(image.as_ref(), inner);
                format!(
                    "page {}/{} | zoom:{} | {} {} | +/- zoom 0 fit; arrows pan; PgUp/PgDn PDF",
                    image.page,
                    image.pages,
                    self.zoom,
                    display_text(&image.message),
                    display_text(&self.progress)
                )
            }
            Screen::Error(error) => {
                frame.render_widget(
                    Paragraph::new(display_text(error))
                        .block(Block::bordered().title("Viewer error")),
                    areas[1],
                );
                "r reload | Esc/q close".into()
            }
            _ => {
                frame.render_widget(Paragraph::new("Loading… Esc cancels; q closes"), areas[1]);
                "Loading".into()
            }
        };
        let status = if let Some(input) = &self.search {
            format!("Search: {} | Enter find; Esc cancel", display_text(input))
        } else if self.busy() {
            format!("Working… Esc cancel | {status}")
        } else {
            status
        };
        let help = if matches!(self.screen, Screen::Media(_)) {
            "Esc/q close | F10/Ctrl-C exit | +/- zoom 0 fit | arrows pan | PgUp/PgDn PDF"
        } else {
            "Esc/q close | F10/Ctrl-C exit | / search n/N next/prev | w wrap l numbers r reload"
        };
        frame.render_widget(Paragraph::new(format!("{status}\n{help}")), areas[2]);
    }
}

impl Drop for ViewerUi {
    fn drop(&mut self) {
        self.close();
        let _ = self.flush_graphics();
    }
}

#[derive(Default)]
pub struct Ui {
    pub browser: super::BrowserUi,
    pub viewer: ViewerUi,
}
impl Ui {
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewer.resize(width, height);
    }
    pub fn poll(&mut self) -> bool {
        self.viewer.poll()
    }
    pub fn busy(&self) -> bool {
        self.viewer.busy()
    }
}
pub fn handle_key(app: &mut App, ui: &mut Ui, key: KeyEvent) -> bool {
    if key.code == KeyCode::F(10)
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
    {
        return true;
    }
    if !matches!(ui.viewer.screen, Screen::Browser) {
        ui.viewer.key(key);
        return false;
    }
    if ui.browser.filter_before.is_none()
        && !ui.browser.bookmarks
        && matches!(key.code, KeyCode::F(3) | KeyCode::Char('v'))
    {
        if let Some(entry) = app.panes[app.active].current() {
            ui.viewer.open(entry.path.clone());
        }
        return false;
    }
    super::handle_browser_key(app, &mut ui.browser, key)
}
pub fn draw(frame: &mut ratatui::Frame, app: &App, ui: &Ui) {
    if matches!(ui.viewer.screen, Screen::Browser) {
        super::draw_browser(frame, app, &ui.browser);
    } else {
        ui.viewer.draw(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    #[test]
    fn viewer_search_and_close_preserve_browser_state() {
        let mut app = App::new(PathBuf::from("/mcs-viewer-absent"));
        app.active_mut().set_filter("retained".into());
        let mut ui = Ui::default();
        ui.viewer.screen = Screen::Text(Page::default());
        ui.viewer.key(key(KeyCode::Char('/')));
        assert!(!handle_key(&mut app, &mut ui, key(KeyCode::Char('q'))));
        assert_eq!(ui.viewer.search.as_deref(), Some("q"));
        handle_key(&mut app, &mut ui, key(KeyCode::Tab));
        assert_eq!(app.active, 0);
        handle_key(&mut app, &mut ui, key(KeyCode::Esc));
        handle_key(&mut app, &mut ui, key(KeyCode::Char('q')));
        assert!(matches!(ui.viewer.screen, Screen::Browser));
        assert_eq!(app.panes[0].filter, "retained");
        assert!(handle_key(&mut app, &mut ui, key(KeyCode::F(10))));
    }
    #[test]
    fn paging_dimensions_match_the_bordered_content_area() {
        let mut viewer = ViewerUi::default();
        viewer.resize(80, 24);
        assert_eq!(viewer.viewport.height, 19);
        assert_eq!(viewer.viewport.width, 69);
        viewer.key(key(KeyCode::Char('l')));
        assert_eq!(viewer.viewport.width, 78);
        viewer.resize(100, 30);
        assert_eq!(viewer.viewport.height, 25);
        assert_eq!(viewer.viewport.width, 98);
    }
    #[test]
    fn viewer_renders_tiny_terminals_and_sanitizes_errors() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut viewer = ViewerUi::default();
        for (width, height) in [(1, 1), (20, 5), (80, 24), (100, 30)] {
            viewer.screen = Screen::Error("bad\x1b[31m\npath".into());
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| viewer.draw(frame)).unwrap();
        }
        viewer.screen = Screen::Text(Page {
            rows: vec![mcs_viewer::text::Row {
                text: "hello query".into(),
                ..Default::default()
            }],
            query: "query".into(),
            ..Default::default()
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| viewer.draw(frame)).unwrap();
        assert!(terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.bg == Color::Yellow));
    }
}
