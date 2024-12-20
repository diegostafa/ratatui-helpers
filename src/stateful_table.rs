use std::fmt::Display;
use std::ops::Div;

use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
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
    type ColumnValue: Clone;

    fn value(&self) -> Self::Value;
    fn column_values() -> Vec<Self::ColumnValue>;
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

    fn selected_row(&self) -> Option<usize>;
    fn selected_col(&self) -> Option<usize>;

    fn screen_coords_to_row_index(&self, pos: (u16, u16)) -> Option<usize>;
    fn screen_coords_to_col_index(&self, pos: (u16, u16)) -> Option<usize>;
}

#[derive(Default)]
pub struct TableStyle<'a> {
    pub table: Style,
    pub header: Style,
    pub block: (Block<'a>, Padding),
    pub highlight: Style,
    pub col_highlight: Style,
    pub normal: Style,
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
    col_constraints: Vec<Constraint>,
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
        let constraints = col_widths
            .iter()
            .zip(T::column_constraints().iter())
            .map(|(s, c)| c(*s))
            .collect_vec();

        let col_constraints = constraints
            .clone()
            .into_iter()
            .interleave(vec![
                Constraint::Length(style.column_spacing);
                constraints.len() - 1
            ])
            .collect_vec();

        let mut padding = Padding::default();
        let mut table = Table::new(rows, constraints)
            .style(style.normal)
            .column_spacing(style.column_spacing)
            .row_highlight_style(style.highlight)
            .column_highlight_style(style.col_highlight);

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
            col_constraints,
        }
    }
    pub fn selected_value(&self) -> Option<&T::Value> {
        self.state.selected().and_then(|i| self.values.get(i))
    }
    pub fn selected_row(&self) -> Option<usize> {
        self.state.selected()
    }
    pub fn selected_col(&self) -> Option<usize> {
        self.state.selected_column()
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
                            if let Some(idx) = self.selected_row() {
                                if idx == self.rows_count() - 1 {
                                    self.select_absolute(0);
                                } else {
                                    self.select_next();
                                }
                            }
                        }
                        TableCommand::GoUpCycle => {
                            if let Some(idx) = self.selected_row() {
                                if idx == 0 {
                                    self.select_absolute(self.rows_count() - 1);
                                } else {
                                    self.select_prev();
                                }
                            }
                        }
                        TableCommand::GoPageDown => self.select_next_page(),
                        TableCommand::GoPageUp => self.select_prev_page(),
                        TableCommand::GoHalfPageDown => {
                            let offset = self.rows_area().height as isize / 2;
                            self.select_relative(offset);
                        } // TableCommand::GoHalfPageUp => {
                          //     let offset = self.rows_area().height as isize / 2;
                          //     self.select_relative(-offset);
                          // }
                    }
                }
            }
            Event::Mouse(ev) => {
                let pos = Position {
                    x: ev.column,
                    y: ev.row,
                };
                if !self.area.contains(pos) {
                    return;
                }
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
                        if let Some(row) = self.screen_coords_to_row_index(pos) {
                            self.select_absolute(row);
                        } else if let Some(col) = self.screen_coords_to_col_index(pos) {
                            self.select_absolute_col(col);
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
    pub fn min_area(&self) -> (u16, u16) {
        let w = self.inner_width + self.padding.l + self.padding.r;
        let h = (self.rows_count() as u16 * T::row_height()) + self.padding.t + self.padding.b;
        (w, h)
    }
    pub fn header_area(&self) -> Option<Rect> {
        if T::column_names().is_none() {
            return None;
        }
        let area = self.rows_area();
        Some(Rect {
            x: area.x,
            y: area.y - 1,
            width: area.width,
            height: 1,
        })
    }
    pub fn rows_area(&self) -> Rect {
        Rect {
            x: self.area.x + self.padding.l,
            y: self.area.y + self.padding.t,
            width: self.area.width - self.padding.l - self.padding.r,
            height: self.area.height - self.padding.t - self.padding.b,
        }
    }
    pub fn screen_coords_to_row_index(&self, pos: Position) -> Option<usize> {
        let area = self.rows_area();
        if pos.y >= area.y
            && pos.x >= area.x
            && pos.y < area.y.saturating_add(area.height)
            && pos.x < area.x.saturating_add(area.width)
        {
            let relative = pos.y.saturating_sub(area.y).div(T::row_height());
            let absolute = relative.saturating_add(self.state.offset() as u16);
            return Some(absolute as usize);
        }
        None
    }
    pub fn screen_coords_to_col_index(&self, pos: Position) -> Option<usize> {
        self.header_area().and_then(|area| {
            Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints(self.col_constraints.clone())
                .split(area)
                .into_iter()
                .enumerate()
                .find(|(i, rect)| i % 2 == 0 && rect.contains(pos))
                .map(|(i, _)| i / 2)
        })
    }
    fn columns_max_widths(data: &[T]) -> Vec<u16> {
        let mut data = data.iter().map(T::content).collect_vec();
        if let Some(headers) = T::column_names() {
            data.push(headers);
        }
        if data.is_empty() {
            return vec![];
        }
        let widths = |a: Vec<String>| a.iter().map(|e| e.len() as u16).collect();
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

impl<'a, T: Tabular> StatefulTable<'a, T> {
    pub fn select_next(&mut self) {
        self.select_relative(1);
    }
    pub fn select_prev(&mut self) {
        self.select_relative(-1);
    }
    pub fn select_next_page(&mut self) {
        self.select_relative(self.rows_area().height as isize)
    }
    pub fn select_prev_page(&mut self) {
        self.select_relative(-(self.rows_area().height as isize))
    }
    pub fn select_absolute(&mut self, idx: usize) {
        let idx = idx.clamp(0, self.rows_count().saturating_sub(1));
        self.state.select(Some(idx));
    }
    pub fn select_visible(&mut self, idx: usize) {
        self.select_absolute(self.state.offset().saturating_add(idx));
    }
    pub fn select_relative(&mut self, offset: isize) {
        let new = self.selected_row().map_or(0, |curr| {
            if offset < 0 {
                curr.saturating_sub(offset.unsigned_abs())
            } else {
                curr.saturating_add(offset.unsigned_abs())
            }
        });
        self.select_absolute(new);
    }
    pub fn select_next_col(&mut self) {
        self.select_relative_col(1);
    }
    pub fn select_prev_col(&mut self) {
        self.select_relative_col(-1);
    }
    pub fn select_relative_col(&mut self, offset: isize) {
        let new = self.selected_col().map_or(0, |curr| {
            if offset < 0 {
                curr.saturating_sub(offset.unsigned_abs())
            } else {
                curr.saturating_add(offset.unsigned_abs())
            }
        });
        self.select_absolute_col(new);
    }
    pub fn select_absolute_col(&mut self, idx: usize) {
        let idx = idx.clamp(0, self.col_constraints.len().saturating_sub(1));
        self.state.select_column(Some(idx));
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
    type ColumnValue = T::ColumnValue;
    fn value(&self) -> Self::Value {
        self.data.value()
    }
    fn column_values() -> Vec<Self::ColumnValue> {
        let mut values = T::column_values();
        if let Some(v) = values.first().cloned() {
            values.insert(0, v);
        }
        values
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
    fn row_height() -> u16 {
        T::row_height()
    }
}

pub enum TableCommand {
    GoDown,
    GoUp,
    GoDownCycle,
    GoUpCycle,
    GoPageDown,
    GoPageUp,
    GoHalfPageDown,
    // GoHalfPageUp,
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
            TableCommand::GoHalfPageDown => write!(f, "go half page down"),
            // TableCommand::GoHalfPageUp => write!(f, "go half page up"),
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
            ShortCut(
                TableCommand::GoHalfPageDown,
                vec![KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)],
            ),
            // FIXME: crossterm is not detecting the shift ???
            // ShortCut(
            //     TableCommand::GoHalfPageUp,
            //     vec![KeyEvent::new(KeyCode::Char(' '), KeyModifiers::SHIFT)],
            // ),
        ])
    }
}
