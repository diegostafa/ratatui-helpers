#![feature(let_chains)]

use std::cmp::Ordering;
use std::time::Duration;

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{self, terminal};
use ratatui::layout::{Alignment, Constraint};
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Paragraph, TableState};
use ratatui::Terminal;
use ratatui_helpers::stateful_table::{IndexedRow, Padding, StatefulTable, TableStyle, Tabular};
use ratatui_helpers::view::View;
use ratatui_helpers::view_controller::ViewController;

#[derive(Default)]
enum Commands {
    #[default]
    None,
    QuitView,
    OpenMainView,
    ShowNotification(String),
}

#[derive(PartialEq)]
enum ViewKind {
    Main,
    Normal,
}

#[derive(Clone)]
struct Data(&'static str);
impl Tabular for Data {
    type Value = &'static str;
    fn cmp_by_col(&self, other: &Self, _col: usize) -> Ordering {
        self.0.cmp(other.0)
    }
    fn value(&self) -> Self::Value {
        self.0
    }
    fn content(&self) -> Vec<String> {
        vec![self.0.to_string()]
    }
    fn column_constraints() -> Vec<fn(u16) -> Constraint> {
        vec![Constraint::Fill]
    }
    fn column_names() -> Option<Vec<String>> {
        Some(vec!["Column Name".into()])
    }
    fn column_alignments() -> Option<Vec<Alignment>> {
        Some(vec![Alignment::Left])
    }
}

struct MainView<'a> {
    table: StatefulTable<'a, IndexedRow<Data>>,
}
impl MainView<'_> {
    fn new() -> Self {
        Self {
            table: StatefulTable::new_indexed(
                Self::gen_data(),
                TableState::new(),
                Self::style(),
                None,
            ),
        }
    }
    fn gen_data() -> Vec<Data> {
        (0..100).map(|i| Data(format!("ROW {i}").leak())).collect()
    }
    fn style() -> TableStyle<'static> {
        TableStyle {
            table: Style::new(),
            header: Style::new(),
            block: (Block::new(), Padding::default()),
            highlight: Style::new().fg(Color::Red).bg(Color::DarkGray),
            col_highlight: Style::new(),
            normal: Style::new(),
            column_spacing: 5,
        }
    }
}
impl View for MainView<'_> {
    type Model = ();
    type Signal = Commands;
    type Kind = ViewKind;

    fn kind(&self) -> Self::Kind {
        ViewKind::Main
    }
    fn draw(&mut self, f: &mut ratatui::Frame<'_>, area: ratatui::prelude::Rect) {
        self.table.draw(f, area);
    }
    fn update(&mut self, ev: &event::Event) -> Self::Signal {
        self.table.update(ev);
        if let Event::Key(ev) = ev {
            if let KeyCode::Char('q') = ev.code {
                return Commands::QuitView;
            }
        }
        Commands::None
    }
}

struct NormalView;
impl View for NormalView {
    type Model = ();
    type Signal = Commands;
    type Kind = ViewKind;

    fn kind(&self) -> Self::Kind {
        ViewKind::Normal
    }
    fn update(&mut self, ev: &event::Event) -> Self::Signal {
        if let Event::Key(ev) = ev {
            match ev.code {
                KeyCode::Char('q') => return Commands::QuitView,
                KeyCode::Char(c) => return Commands::ShowNotification(c.to_string()),
                KeyCode::Enter => return Commands::OpenMainView,
                _ => {}
            }
        }
        Commands::None
    }
    fn draw(&mut self, f: &mut ratatui::Frame<'_>, area: ratatui::prelude::Rect) {
        f.render_widget(Paragraph::new("normal view"), area);
    }
}

fn main() {
    let mut term = grab_term();
    let mut ctrl = ViewController::new(Duration::from_millis(1000));
    ctrl.push(Box::new(NormalView));

    while ctrl.is_running() {
        let _ = term.draw(|f| ctrl.draw(f, f.area()));
        if let Ok(true) = event::poll(Duration::from_millis(200)) {
            let ev = &event::read().unwrap();
            match ctrl.curr_mut().update(ev) {
                Commands::None => {}
                Commands::QuitView => ctrl.pop(),
                Commands::OpenMainView => ctrl.push(Box::new(MainView::new())),
                Commands::ShowNotification(s) => ctrl.show_status(s),
            }
        }
        ctrl.update_status_line();
    }
    drop_term(term);
}

fn grab_term() -> Terminal<CrosstermBackend<std::io::Stdout>> {
    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode().unwrap();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    Terminal::new(CrosstermBackend::new(stdout)).unwrap()
}
fn drop_term(mut term: Terminal<CrosstermBackend<std::io::Stdout>>) {
    terminal::disable_raw_mode().unwrap();
    crossterm::execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
    term.show_cursor().unwrap();
}
