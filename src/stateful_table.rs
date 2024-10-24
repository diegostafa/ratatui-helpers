use std::fmt::Display;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::{Alignment, Constraint, Position, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, Row, StatefulWidget, Table, TableState};
use ratatui::Frame;

use crate::keymap::{KeyMap, ShortCut};

#[derive(Default)]
pub struct Padding {
    pub t: u16,
    pub r: u16,
    pub b: u16,
    pub l: u16,
}
impl Padding {
    pub fn add_padding(&mut self, padding: Padding) {
        self.t += padding.t;
        self.r += padding.r;
        self.b += padding.b;
        self.l += padding.l;
    }
    pub fn add_value(&mut self, val: u16) {
        self.t += val;
        self.r += val;
        self.b += val;
        self.l += val;
    }
}

pub trait Tabular {
    type Value;
    fn value(&self) -> Self::Value;
    fn content(&self) -> Vec<String>;
    fn style(&self) -> Style {
        Style::default()
    }
    fn column_constraints() -> Vec<fn(u16) -> Constraint>;
    fn column_names() -> Option<Vec<String>> {
        None
    }
    fn column_alignments() -> Option<Vec<Alignment>> {
        None
    }
    fn row_height() -> u16 {
        1
    }
    fn header_height() -> u16 {
        1
    }
}
pub trait InteractiveTable {
    fn select_next(&mut self);
    fn select_prev(&mut self);
    fn select_next_page(&mut self);
    fn select_prev_page(&mut self);
    fn select_absolute(&mut self, idx: usize);
    fn select_visible(&mut self, idx: usize);
    fn select_relative(&mut self, offset: isize);
    fn selected_index(&self) -> Option<usize>;
    fn screen_coords_to_row_index(&self, pos: (u16, u16)) -> Option<usize>;
}

pub struct TableStyle<'a> {
    pub table: Style,
    pub header: Style,
    pub block: (Block<'a>, Padding),
    pub highlight: Style,
    pub column_spacing: u16,
}

pub struct StatefulTable<'a, T: Tabular> {
    table: Table<'a>,
    state: TableState,
    values: Vec<T::Value>,
    area: Rect,
    padding: Padding,
    inner_width: u16,
    keymap: TableKeyMap,
}
impl<'a, T: Tabular> StatefulTable<'a, T> {
    pub fn new(
        data: Vec<T>,
        mut state: TableState,
        mut style: TableStyle<'a>,
        title: Option<String>,
    ) -> Self {
        let alignments = T::column_alignments();
        let names = T::column_names();

        let rows = data.iter().map(|model| {
            if cfg!(debug_assertions) {
                Self::check_tabular(model);
            }

            match &alignments {
                Some(alignments) => Row::new(
                    model
                        .content()
                        .into_iter()
                        .zip(alignments)
                        .map(|(c, a)| Text::raw(c).alignment(*a)),
                ),
                None => Row::new(model.content()),
            }
            .style(model.style())
            .height(T::row_height())
        });

        let col_widths = Self::columns_max_widths(&data);
        let constraints: Vec<_> = col_widths
            .iter()
            .zip(T::column_constraints().iter())
            .map(|(s, c)| c(*s))
            .collect();

        let mut padding = Padding::default();
        let mut table = Table::new(rows, constraints)
            .column_spacing(style.column_spacing)
            .row_highlight_style(style.highlight);

        if let Some(header) = names {
            padding.t += 1;
            let row = match &alignments {
                Some(alignments) => Row::new(
                    header
                        .into_iter()
                        .zip(alignments)
                        .map(|(c, a)| Text::raw(c).alignment(*a)),
                ),
                None => Row::new(header),
            };
            table = table.header(row.style(style.header));
        }

        padding.add_padding(style.block.1);
        if let Some(title) = title {
            style.block.0 = style.block.0.title(title);
        }
        table = table.block(style.block.0);

        if let Some(idx) = state.selected() {
            state.select(Some(idx.clamp(0, data.len().saturating_sub(1))));
        }

        Self {
            table,
            state,
            padding,
            inner_width: col_widths.iter().sum::<u16>()
                + (style.column_spacing * (col_widths.len() - 1) as u16),
            values: data.iter().map(T::value).collect(),
            area: Rect::default(),
            keymap: KeyMap::default(),
        }
    }
    pub fn selected_value(&self) -> Option<&T::Value> {
        self.state.selected().and_then(|i| self.values.get(i))
    }
    pub fn rows_count(&self) -> usize {
        self.values.len()
    }
    pub fn update(&mut self, ev: &Event) {
        match ev {
            Event::Key(ev) => {
                if let Some(cmd) = self.keymap.get_command(ev) {
                    match cmd {
                        TableCommand::GoDown => self.select_next(),
                        TableCommand::GoUp => self.select_prev(),
                        TableCommand::GoDownCycle => {
                            if let Some(idx) = self.selected_index() {
                                if idx == self.rows_count() - 1 {
                                    self.select_absolute(0);
                                } else {
                                    self.select_next();
                                }
                            }
                        }
                        TableCommand::GoUpCycle => {
                            if let Some(idx) = self.selected_index() {
                                if idx == 0 {
                                    self.select_absolute(self.rows_count() - 1);
                                } else {
                                    self.select_prev();
                                }
                            }
                        }
                        TableCommand::GoPageDown => self.select_next_page(),
                        TableCommand::GoPageUp => self.select_prev_page(),
                    }
                }
            }
            Event::Mouse(ev)
                if self.inner_area().contains(Position {
                    x: ev.column,
                    y: ev.row,
                }) =>
            {
                match ev.kind {
                    MouseEventKind::ScrollDown => match ev.modifiers {
                        KeyModifiers::ALT => self.select_relative(2),
                        _ => self.select_next(),
                    },
                    MouseEventKind::ScrollUp => match ev.modifiers {
                        KeyModifiers::ALT => self.select_relative(-2),
                        _ => self.select_prev(),
                    },
                    MouseEventKind::Down(MouseButton::Left | MouseButton::Right) => {
                        if let Some(row) = self.screen_coords_to_row_index((ev.row, ev.column)) {
                            self.select_absolute(row);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        self.area = area;
        f.render_stateful_widget(&self.table, area, &mut self.state);
    }
    pub fn state(&self) -> &TableState {
        &self.state
    }
    pub fn size(&self) -> (u16, u16) {
        (
            self.inner_width + self.padding.l + self.padding.r,
            self.rows_count() as u16 + self.padding.t + self.padding.b,
        )
    }
    pub fn inner_area(&self) -> Rect {
        Rect {
            x: self.area.x + self.padding.l,
            y: self.area.y + self.padding.t,
            width: self.area.width - self.padding.l - self.padding.r,
            height: self.area.height - self.padding.t - self.padding.b,
        }
    }
    fn columns_max_widths(data: &[T]) -> Vec<u16> {
        let mut data: Vec<_> = data.iter().map(T::content).collect();

        if let Some(headers) = T::column_names() {
            data.push(headers);
        }
        if data.is_empty() {
            return vec![];
        }
        let widths = |a: Vec<String>| a.iter().map(|e| e.len() as u16).collect::<Vec<_>>();
        let max_widths = |a: Vec<u16>, b: Vec<u16>| (0..a.len()).map(|i| a[i].max(b[i])).collect();
        data.into_iter().map(widths).reduce(max_widths).unwrap()
    }
    fn check_tabular(t: &T) {
        let content = t.content().len();
        let constraints = T::column_constraints().len();
        let names = T::column_names().map_or(content, |n| n.len());
        let alignements = T::column_alignments().map_or(content, |a| a.len());
        assert!(content == constraints && constraints == names && names == alignements);
    }
}
impl<T: Tabular> InteractiveTable for StatefulTable<'_, T> {
    fn select_next(&mut self) {
        self.select_relative(1);
    }
    fn select_prev(&mut self) {
        self.select_relative(-1);
    }
    fn select_next_page(&mut self) {
        self.select_relative(self.inner_area().height as isize)
    }
    fn select_prev_page(&mut self) {
        self.select_relative(-(self.inner_area().height as isize))
    }
    fn select_absolute(&mut self, idx: usize) {
        let idx = idx.clamp(0, self.rows_count().saturating_sub(1));
        self.state.select(Some(idx));
    }
    fn select_visible(&mut self, idx: usize) {
        self.select_absolute(self.state.offset().saturating_add(idx));
    }
    fn select_relative(&mut self, offset: isize) {
        let new = self.selected_index().map_or(0, |curr| {
            if offset < 0 {
                curr.saturating_sub(offset.unsigned_abs())
            } else {
                curr.saturating_add(offset.unsigned_abs())
            }
        });
        self.select_absolute(new);
    }
    fn selected_index(&self) -> Option<usize> {
        self.state.selected()
    }
    fn screen_coords_to_row_index(&self, (row, col): (u16, u16)) -> Option<usize> {
        let area = self.inner_area();
        let row = row.div_ceil(T::row_height());
        if row >= area.y
            && col >= area.x
            && row < area.y.saturating_add(area.height)
            && col < area.x.saturating_add(area.width)
        {
            let relative = row.saturating_sub(area.y);
            let absolute = relative.saturating_add(self.state.offset() as u16);
            return Some(absolute as usize);
        }
        None
    }
}
impl<'a, T: Tabular> StatefulWidget for StatefulTable<'a, T> {
    type State = TableState;
    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.area = area;
        self.table.render(area, buf, state);
    }
}

pub struct IndexedRow<T: Tabular> {
    pub idx: usize,
    pub data: T,
}
impl<T: Tabular> IndexedRow<T> {
    pub fn from(data: Vec<T>) -> Vec<IndexedRow<T>> {
        data.into_iter()
            .enumerate()
            .map(|(i, d)| IndexedRow { idx: i, data: d })
            .collect()
    }
}
impl<T: Tabular> Tabular for IndexedRow<T> {
    type Value = T::Value;
    fn value(&self) -> Self::Value {
        self.data.value()
    }
    fn content(&self) -> Vec<String> {
        let mut content = self.data.content();
        content.insert(0, format!("{}", self.idx));
        content
    }
    fn column_names() -> Option<Vec<String>> {
        T::column_names().map(|mut names| {
            names.insert(0, "#".into());
            names
        })
    }
    fn column_constraints() -> Vec<fn(u16) -> Constraint> {
        let mut constraints = T::column_constraints();
        constraints.insert(0, Constraint::Length);
        constraints
    }
    fn style(&self) -> Style {
        T::style(&self.data)
    }
    fn column_alignments() -> Option<Vec<Alignment>> {
        T::column_alignments().map(|mut alignemnts| {
            alignemnts.insert(0, Alignment::Center);
            alignemnts
        })
    }
}

pub enum TableCommand {
    GoDown,
    GoUp,
    GoDownCycle,
    GoUpCycle,
    GoPageDown,
    GoPageUp,
}
impl Display for TableCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableCommand::GoDown => write!(f, "go down"),
            TableCommand::GoUp => write!(f, "go up"),
            TableCommand::GoDownCycle => write!(f, "go down cycle"),
            TableCommand::GoUpCycle => write!(f, "go up cycle"),
            TableCommand::GoPageDown => write!(f, "go page down"),
            TableCommand::GoPageUp => write!(f, "go page up"),
        }
    }
}

pub struct TableKeyMap(pub Vec<ShortCut<TableCommand>>);
impl KeyMap for TableKeyMap {
    type Command = TableCommand;

    fn get_shortcuts(&self) -> &[ShortCut<Self::Command>] {
        &self.0
    }
    fn default() -> Self {
        Self(vec![
            ShortCut(
                TableCommand::GoDown,
                vec![
                    KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                    KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                ],
            ),
            ShortCut(
                TableCommand::GoUp,
                vec![
                    KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                    KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                ],
            ),
            ShortCut(
                TableCommand::GoDownCycle,
                vec![KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)],
            ),
            ShortCut(
                TableCommand::GoUpCycle,
                vec![KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)],
            ),
            ShortCut(
                TableCommand::GoPageDown,
                vec![KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)],
            ),
            ShortCut(
                TableCommand::GoPageUp,
                vec![KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)],
            ),
        ])
    }
}
