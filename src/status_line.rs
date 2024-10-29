use std::fmt::Display;
use std::time::{Duration, Instant};

use itertools::Itertools;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Clone, Copy, PartialEq, Default)]
pub struct StatusId(u32);
impl StatusId {
    pub fn next(&mut self) {
        self.0 += 1;
    }
}

struct Message {
    id: StatusId,
    msg: String,
    created_at: Instant,
    duration: Option<Duration>,
    show_elapsed: bool,
}
impl Message {
    pub fn get_elapsed_secs(&self) -> f32 {
        self.created_at.elapsed().as_millis() as f32 / 1000f32
    }
}
impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.show_elapsed {
            write!(f, "{} ({}s)", self.msg, self.get_elapsed_secs())
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

#[derive(Default)]
pub struct StatusLine {
    ids: StatusId,
    lines: Vec<Message>,
}
impl StatusLine {
    pub fn show(
        &mut self,
        msg: String,
        duration: Option<Duration>,
        show_elapsed: bool,
    ) -> StatusId {
        self.ids.next();
        self.lines.push(Message {
            id: self.ids,
            msg,
            created_at: Instant::now(),
            duration,
            show_elapsed,
        });
        self.ids
    }
    pub fn update(&mut self) {
        self.lines.retain(|line| {
            line.duration
                .map_or(true, |ttl| line.created_at + ttl > Instant::now())
        });
    }
    pub fn remove(&mut self, id: StatusId) {
        self.lines.retain(|l| l.id != id);
    }
    pub fn get_layout(&self) -> Layout {
        let layout = Layout::default().direction(ratatui::layout::Direction::Vertical);
        if self.lines.is_empty() {
            layout.constraints([Constraint::Fill(1), Constraint::Length(0)])
        } else {
            layout.constraints([Constraint::Fill(1), Constraint::Length(1)])
        }
    }
    pub fn draw(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Paragraph::new(self.get_line()), area);
    }

    pub fn get_line(&self) -> String {
        self.lines
            .iter()
            .enumerate()
            .map(|(i, m)| format!("[{}] {}", i + 1, m))
            .rev()
            .join(" | ")
    }
}
