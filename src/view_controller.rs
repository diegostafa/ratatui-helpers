use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::widgets::Clear;
use ratatui::Frame;

use crate::dock::{Dock, DockPosition};
use crate::status_line::{StatusId, StatusLine};
use crate::view::View;

pub struct ViewController<M, S, K>
where
    S: Default,
    K: PartialEq,
{
    views: Vec<Box<dyn View<Model = M, Signal = S, Kind = K>>>,
    status: Arc<Mutex<StatusLine>>,
    status_ttl: Duration,
    dock: Option<Dock<M, S, K>>,
}
impl<M, S, K: PartialEq> ViewController<M, S, K>
where
    S: Default,
    K: PartialEq,
{
    pub fn new(status_ttl: Duration) -> Self {
        Self {
            views: vec![],
            status: Default::default(),
            status_ttl,
            dock: Default::default(),
        }
    }
    pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        let status = self.status.lock().unwrap();
        let layout = status.get_layout().split(area);
        status.draw(f, layout[1]);
        drop(status);

        if let Some(dock) = &mut self.dock {
            let layout = dock.get_layout().split(layout[0]);
            match dock.position {
                DockPosition::Left | DockPosition::Top => {
                    dock.draw(f, layout[0]);
                    self.draw_visible_views(f, layout[1], self.views.len() - 1);
                }
                DockPosition::Right | DockPosition::Bottom => {
                    dock.draw(f, layout[1]);
                    self.draw_visible_views(f, layout[0], self.views.len() - 1);
                }
            }
        } else {
            self.draw_visible_views(f, layout[0], self.views.len() - 1);
        }
    }
    pub fn is_running(&self) -> bool {
        !self.views.is_empty()
    }

    // --- views
    pub fn push(&mut self, view: Box<dyn View<Model = M, Signal = S, Kind = K>>) {
        if self.is_running() && self.curr_mut().kind() == view.kind() {
            self.pop();
            self.views.push(view);
        } else {
            self.views.push(view);
        }
        self.curr().set_title();
    }
    pub fn pop(&mut self) {
        self.views.pop();
        if self.is_running() {
            self.curr().set_title();
        }
    }
    pub fn curr(&self) -> &dyn View<Model = M, Signal = S, Kind = K> {
        self.views.last().unwrap().as_ref()
    }
    pub fn curr_mut(&mut self) -> &mut Box<dyn View<Model = M, Signal = S, Kind = K>> {
        self.views.last_mut().unwrap()
    }
    pub fn refresh(&mut self, model: &M) {
        self.refresh_visible_views(model, self.views.len() - 1);
    }
    fn refresh_visible_views(&mut self, model: &M, idx: usize) {
        if self.views[idx].is_floating() {
            self.refresh_visible_views(model, idx - 1);
            self.views[idx].refresh(model);
        } else {
            self.views[idx].refresh(model);
        }
    }
    fn draw_visible_views(&mut self, f: &mut Frame<'_>, area: Rect, idx: usize) {
        if self.views[idx].is_floating() {
            self.draw_visible_views(f, area, idx - 1);
            let view = &mut self.views[idx];
            let area = view.compute_area(area);
            f.render_widget(Clear, area);
            view.draw(f, area);
        } else {
            self.views[idx].draw(f, area);
        }
    }

    // --- status line
    pub fn status(&self) -> &Arc<Mutex<StatusLine>> {
        &self.status
    }
    pub fn show_status(&self, msg: String) {
        let _ = self
            .status
            .lock()
            .unwrap()
            .show(msg, Some(self.status_ttl), false);
    }
    pub fn show_status_for(&self, msg: String, duration: Duration) {
        let _ = self.status.lock().unwrap().show(msg, Some(duration), false);
    }
    pub fn show_status_always(&self, msg: String) -> StatusId {
        self.status.lock().unwrap().show(msg, None, true)
    }
    pub fn update_status_line(&self) {
        self.status.lock().unwrap().update();
    }

    // --- dock
    pub fn set_dock(&mut self, dock: Dock<M, S, K>) {
        self.dock = Some(dock);
    }
    pub fn remove_dock(&mut self) {
        self.dock = None;
    }
    pub fn update_dock(&mut self, ev: &Event) -> S {
        self.dock
            .as_mut()
            .map_or(S::default(), |dock| dock.view.update(ev))
    }
}
