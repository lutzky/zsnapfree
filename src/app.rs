// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// zsnapfree is a TUI for showing how much space can be reclaimed by freeing
/// zfs snapshots. It is a TUI wrapper over the standard `zfs` tool.

use std::time::Duration;

use crate::tui;
use crate::zfs;
use crate::zfs::ReclaimResult;

use color_eyre::Result;
use human_bytes::human_bytes;
use ratatui::crossterm::event::poll;
use ratatui::widgets::block::Title;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, List, ListItem, ListState, StatefulWidget, Widget},
    Frame,
};

pub struct App {
    dataset: String,
    items: Vec<SnapshotListItem>,
    pub result: zfs::ReclaimResult,
    snapshot_list_state: ListState,
    dirty: bool,

    exit: bool,
}

#[derive(Debug)]
struct SnapshotListItem {
    name: String,
    marked: bool,
}

fn snap_ranges(items: &[SnapshotListItem]) -> Vec<zfs::SnapRange> {
    items
        .chunk_by(|a, b| a.marked == b.marked)
        .filter(|chunk| chunk.first().is_some_and(|f| f.marked))
        .map(|chunk| {
            if chunk.len() == 1 {
                zfs::SnapRange::Single(&chunk[0].name)
            } else {
                zfs::SnapRange::Range(&chunk[0].name, &chunk.last().unwrap().name)
            }
        })
        .collect()
}

impl App {
    pub fn new(dataset: &str) -> Self {
        Self {
            dataset: dataset.to_owned(),
            snapshot_list_state: ListState::default(),
            result: ReclaimResult::default(),
            items: zfs::get_snapshots(dataset)
                .unwrap()
                .iter()
                .map(|snapshot| SnapshotListItem {
                    name: snapshot.to_owned(),
                    marked: false,
                })
                .collect(),
            dirty: false,

            exit: false,
        }
    }

    pub fn equivalent_command_line(&self) -> String {
        format!(
            "zfs destroy -nv {}@{}",
            self.dataset,
            &zfs::snap_range_commandline(&snap_ranges(&self.items))
        )
    }

    pub fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render_frame(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn exit(&mut self) {
        self.exit = true
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Home => self.select_first(),
            KeyCode::End => self.select_last(),
            KeyCode::Char('q') | KeyCode::Esc => self.exit(),
            KeyCode::Char('k') | KeyCode::Up => self.select_prev(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char(' ') | KeyCode::Enter => self.mark_current(),
            _ => {}
        }
    }

    fn mark_current(&mut self) {
        let Some(selected) = self.snapshot_list_state.selected() else {
            return;
        };
        self.items[selected].marked ^= true;
        self.dirty = true;
        self.select_next();
    }

    fn select_prev(&mut self) {
        self.snapshot_list_state.select_previous();
    }

    fn select_next(&mut self) {
        self.snapshot_list_state.select_next();
    }

    fn select_first(&mut self) {
        self.snapshot_list_state.select_first();
    }

    fn select_last(&mut self) {
        self.snapshot_list_state.select_last();
    }

    pub fn recalculate_result(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let ranges = snap_ranges(&self.items);

        if ranges.is_empty() {
            self.result = ReclaimResult::default();
            return;
        }

        self.result = zfs::get_reclaim(&self.dataset, &ranges).unwrap();
    }

    fn handle_events(&mut self) -> Result<()> {
        if poll(Duration::from_millis(500))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event);
                }
                _ => {}
            };
        } else {
            self.recalculate_result();
        }
        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Title::from(format!(" Snapshots for {} ", self.dataset).bold());

        let title_bottom_parts = [
            vec![
                " Destroying ".blue().bold(),
                format!("{}", self.result.destroys.len()).into(),
                " snapshots would reclaim ".blue().bold(),
                human_bytes(self.result.bytes as f64).into(),
                " ".into(),
            ],
            if self.dirty {
                vec!["<recalculating...> ".yellow()]
            } else {
                vec![]
            },
        ]
        .concat();

        let title_bottom = Line::from(title_bottom_parts);

        let block = Block::bordered()
            .title(title)
            .title_bottom(title_bottom)
            .border_set(border::THICK);

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|my_item| {
                let prefix = if my_item.marked { "+" } else { " " };
                ListItem::new(Line::styled(
                    format!("{prefix} {}", my_item.name),
                    if my_item.marked {
                        Style::new().yellow()
                    } else {
                        Style::new()
                    },
                ))
            })
            .collect();

        let list = List::new(items)
            .block(Block::bordered().title("List"))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .fg(ratatui::style::Color::Blue),
            )
            .highlight_symbol("> ")
            .block(block);

        StatefulWidget::render(list, area, buf, &mut self.snapshot_list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::SnapshotListItem;
    use super::*;

    #[test]
    fn consecutive_snap_ranges() {
        use zfs::SnapRange::*;
        let items: Vec<SnapshotListItem> = vec![
            ("a", false),
            ("b", true),
            ("c", true),
            ("d", true),
            ("e", false),
            ("f", true),
            ("g", true),
            ("h", false),
            ("i", true),
        ]
        .iter()
        .map(|(name, marked)| SnapshotListItem {
            name: name.to_string(),
            marked: *marked,
        })
        .collect();

        let want = vec![Range("b", "d"), Range("f", "g"), Single("i")];

        assert_eq!(snap_ranges(&items), want);
    }
}
