use std::fmt::Display;

use itertools::Itertools;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Constraint};

use crate::stateful_table::Tabular;

pub trait KeyMap {
    type Command: Display;

    fn default() -> Self;
    fn get_shortcuts(&self) -> &[ShortCut<Self::Command>];
    fn get_command(&self, ev: &KeyEvent) -> Option<&Self::Command> {
        self.get_shortcuts()
            .iter()
            .find(|shortcut| shortcut.1.contains(ev))
            .map(|shortcut| &shortcut.0)
    }
}

pub struct ShortCut<T: Display>(pub T, pub Vec<KeyEvent>);
impl<T: Display> Tabular for ShortCut<T> {
    type Value = ();
    fn value(&self) -> Self::Value {}

    fn content(&self) -> Vec<String> {
        let keyevent_to_string = |ev: &KeyEvent| {
            let mut mods = ev.modifiers.iter().map(|m| m.to_string()).collect_vec();
            mods.push(ev.code.to_string());
            mods.join("+")
        };

        vec![
            format!("{}", self.0),
            self.1.iter().map(keyevent_to_string).join(", "),
        ]
    }
    fn column_constraints() -> Vec<fn(u16) -> Constraint> {
        vec![Constraint::Length, Constraint::Fill]
    }
    fn column_names() -> Option<Vec<String>> {
        Some(vec!["Command".to_string(), "Key".to_string()])
    }
    fn column_alignments() -> Option<Vec<Alignment>> {
        Some(vec![Alignment::Left, Alignment::Right])
    }
}
