use crossterm::event::Event;
use ratatui::crossterm::terminal;
use ratatui::layout::Rect;
use ratatui::{crossterm, Frame};

pub trait View {
    type Model;
    type Signal: Default;
    type Kind: PartialEq;

    fn kind(&self) -> Self::Kind;

    fn refresh(&mut self, _model: &Self::Model) {}

    fn is_floating(&self) -> bool {
        false
    }
    fn title(&self) -> String {
        String::new()
    }
    fn set_title(&self) {
        crossterm::execute!(std::io::stdout(), terminal::SetTitle(self.title())).unwrap()
    }
    fn compute_area(&self, area: Rect) -> Rect {
        area
    }
    fn draw(&mut self, _f: &mut Frame<'_>, _area: Rect) {}
    fn update(&mut self, _ev: &Event) -> Self::Signal {
        Self::Signal::default()
    }
    fn on_prompt_submit(&mut self, _value: String) -> Self::Signal {
        Self::Signal::default()
    }
    fn on_prompt_change(&mut self, _value: String) -> Self::Signal {
        Self::Signal::default()
    }
}
