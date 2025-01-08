use std::cmp::Ordering;
use std::fmt::Display;
use std::ops::Div;
use std::vec;

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

#[derive(Default, Clone, Copy)]
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

pub trait Tabular: Clone {
    type Value;
    fn data(&self) -> impl Tabular {
        self.clone()
    }
    fn cmp_by_col(&self, _other: &Self, _col: usize) -> Ordering {
        Ordering::Equal
    }
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
    data: Vec<T>,
    table: Table<'a>,
    state: TableState,
    style: TableStyle<'a>,
    selected_col_ord: Ordering,
    area: Rect,
    values: Vec<T::Value>,
    keymap: TableKeyMap,
    padding: Padding,
    inner_width: u16,
    col_constraints: Vec<Constraint>,
    indexed: bool,
}
impl<'a, T: Tabular> StatefulTable<'a, T> {
    fn build_header(
        alignments: &[Alignment],
        selected_col: Option<usize>,
        selected_col_ord: Ordering,
        header_style: Style,
    ) -> Option<Row<'a>> {
        T::column_names().map(|header| {
            Row::new(
                header
                    .into_iter()
                    .zip(alignments)
                    .enumerate()
                    .map(|(i, (c, a))| {
                        Text::raw(match selected_col {
                            Some(col) if col == i => Self::format_column_name(c, selected_col_ord),
                            _ => c,
                        })
                        .alignment(*a)
                    }),
            )
            .style(header_style)
        })
    }
    fn format_column_name(s: String, ord: Ordering) -> String {
        match ord {
            Ordering::Less => format!("{s}▲"),
            Ordering::Equal => format!("{s}-"),
            Ordering::Greater => format!("{s}▼"),
        }
    }

    fn sort_rows(&mut self) {
        if let Some(col) = self.selected_col() {
            if self.indexed && col == 0 {
                return;
            }
            let alignments = Self::alignemnts();
            let mut data = self
                .data
                .clone()
                .into_iter()
                .zip(std::mem::take(&mut self.values))
                .collect_vec();

            match self.selected_col_ord {
                Ordering::Less => data.sort_by(|a, b| a.0.cmp_by_col(&b.0, col)),
                Ordering::Greater => data.sort_by(|a, b| b.0.cmp_by_col(&a.0, col)),
                Ordering::Equal => {}
            }

            let (data, values): (Vec<_>, Vec<_>) = data.into_iter().unzip();
            let rows = if self.indexed {
                // rebuild indexes
                let dedup = data.iter().map(T::data).collect();
                Self::build_rows(&IndexedRow::from(dedup), &alignments)
            } else {
                Self::build_rows(&data, &alignments)
            };
            let mut table = std::mem::take(&mut self.table);
            table = table.rows(rows);
            if let Some(header) = Self::build_header(
                &alignments,
                self.selected_col(),
                self.selected_col_ord,
                self.style.header,
            ) {
                table = table.header(header);
            }
            self.table = table;
            self.values = values;
        }
    }

    pub fn new(
        data: Vec<T>,
        state: TableState,
        style: TableStyle<'a>,
        title: Option<String>,
    ) -> Self {
        Self::build_table(data, state, style, title, false)
    }
    pub fn new_indexed(
        data: Vec<T>,
        state: TableState,
        style: TableStyle<'a>,
        title: Option<String>,
    ) -> StatefulTable<'a, IndexedRow<T>> {
        StatefulTable::build_table(IndexedRow::from(data), state, style, title, true)
    }

    fn build_table(
        data: Vec<T>,
        mut state: TableState,
        mut style: TableStyle<'a>,
        title: Option<String>,
        indexed: bool,
    ) -> Self {
        let values = data.iter().map(T::value).collect();
        if let Some(idx) = state.selected() {
            state.select(Some(idx.clamp(0, data.len().saturating_sub(1))));
        }

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
            .collect();

        let alignments = Self::alignemnts();
        let mut table = Table::new(Self::build_rows(&data, &alignments), constraints)
            .style(style.normal)
            .column_spacing(style.column_spacing)
            .row_highlight_style(style.highlight)
            .column_highlight_style(style.col_highlight);

        let mut padding = Padding::default();
        if let Some(header) = Self::build_header(&alignments, None, Ordering::Equal, style.header) {
            padding.t += 1;
            table = table.header(header);
        }

        padding.add_padding(style.block.1);
        if let Some(title) = &title {
            style.block.0 = style.block.0.title(title.clone());
        }
        table = table.block(style.block.0.clone());
        let inner_width =
            col_widths.iter().sum::<u16>() + (style.column_spacing * (col_widths.len() - 1) as u16);

        Self {
            table,
            state,
            style,
            padding,
            inner_width,
            values,
            data,
            col_constraints,
            area: Rect::default(),
            keymap: KeyMap::default(),
            selected_col_ord: Ordering::Equal,
            indexed,
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
                            self.sort_rows();
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
        T::column_names()?;
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
                .iter()
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
    fn build_rows(data: &[impl Tabular], alignments: &[Alignment]) -> Vec<Row<'a>> {
        data.iter()
            .map(|row| {
                Row::new(
                    row.content()
                        .into_iter()
                        .zip(alignments)
                        .map(|(c, a)| Text::raw(c).alignment(*a)),
                )
                .style(row.style())
                .height(T::row_height())
            })
            .collect()
    }
    fn alignemnts() -> Vec<Alignment> {
        T::column_alignments().unwrap_or(vec![Alignment::default(); T::column_constraints().len()])
    }

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
        match self.selected_col() {
            Some(old) if old != idx => self.selected_col_ord = Ordering::Equal,
            Some(_) => {
                self.selected_col_ord = match self.selected_col_ord {
                    Ordering::Less => Ordering::Equal,
                    Ordering::Equal => Ordering::Greater,
                    Ordering::Greater => Ordering::Less,
                }
            }
            None => self.selected_col_ord = Ordering::Equal,
        }
        self.state.select_column(Some(idx));
    }
}
impl<T: Tabular> StatefulWidget for StatefulTable<'_, T> {
    type State = TableState;
    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.area = area;
        self.table.render(area, buf, state);
    }
}

#[derive(Clone)]
pub struct IndexedRow<T: Tabular> {
    idx: usize,
    data: T,
}
impl<T: Tabular> IndexedRow<T> {
    pub fn sort_by<F>(rows: &mut [IndexedRow<T>], mut cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        rows.sort_by(|a, b| cmp(&a.data, &b.data));
    }
}
impl<T: Tabular> IndexedRow<T> {
    fn from(data: Vec<T>) -> Vec<IndexedRow<T>> {
        data.into_iter()
            .enumerate()
            .map(|(idx, data)| IndexedRow { idx, data })
            .collect()
    }
}
impl<T: Tabular> Tabular for IndexedRow<T> {
    type Value = T::Value;
    fn data(&self) -> impl Tabular {
        self.data.clone()
    }
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
    fn row_height() -> u16 {
        T::row_height()
    }
    fn cmp_by_col(&self, other: &Self, col: usize) -> Ordering {
        if col == 0 {
            Ordering::Equal
        } else {
            self.data.cmp_by_col(&other.data, col - 1)
        }
    }
    fn header_height() -> u16 {
        1
    }
}

#[derive(Clone)]
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

#[derive(Clone)]
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
