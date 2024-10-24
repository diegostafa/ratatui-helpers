use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use crate::view::View;

#[derive(PartialEq)]
pub enum DockPosition {
    Left,
    Right,
    Top,
    Bottom,
}

pub struct Dock<M, S, K>
where
    S: Default,
    K: PartialEq,
{
    pub position: DockPosition,
    pub size: u16,
    pub view: Box<dyn View<Model = M, Signal = S, Kind = K>>,
}
impl<M, S, K> Dock<M, S, K>
where
    S: Default,
    K: PartialEq,
{
    pub fn get_layout(&self) -> Layout {
        match self.position {
            DockPosition::Left => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(self.size), Constraint::Fill(1)]),
            DockPosition::Right => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Fill(1), Constraint::Length(self.size)]),
            DockPosition::Top => Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(self.size), Constraint::Fill(1)]),
            DockPosition::Bottom => Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Fill(1), Constraint::Length(self.size)]),
        }
    }
    pub fn draw(&mut self, f: &mut Frame, area: Rect) {
        self.view.draw(f, area);
    }
}
