//! The inspector: a title block, one row of actions, a notice and a card, stacked on one edge.

use rune::{El, col, row};
use rune_kit::Kit;

use super::{size, space, text};

#[derive(Default)]
pub struct Inspector {
    title: Option<String>,
    meta: Option<String>,
    status: Option<El>,
    description: Option<String>,
    left: Vec<El>,
    right: Option<El>,
    notice: Option<El>,
    cards: Vec<Option<El>>,
}

impl Inspector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, s: impl Into<String>) -> Self {
        self.title = Some(s.into());
        self
    }

    pub fn meta(mut self, s: impl Into<String>) -> Self {
        self.meta = Some(s.into());
        self
    }

    pub fn status(mut self, e: El) -> Self {
        self.status = Some(e);
        self
    }

    /// Under the meta line; wraps.
    pub fn description(mut self, s: impl Into<String>) -> Self {
        self.description = Some(s.into());
        self
    }

    /// One row that never wraps: up to three buttons and an overflow on the left, one more
    /// (usually the destructive one) pinned right.
    pub fn actions(mut self, left: Vec<El>, right: Option<El>) -> Self {
        self.left = left;
        self.right = right;
        self
    }

    /// Directly under the action row.
    pub fn notice(mut self, n: El) -> Self {
        self.notice = Some(n);
        self
    }

    /// A card; `None` (an empty group) is skipped.
    pub fn card(mut self, c: Option<El>) -> Self {
        self.cards.push(c);
        self
    }

    pub fn build(self, k: &Kit) -> El {
        let mut pane = col().key("inspector").w_full().padding(size::PANE_EDGE).gap(space::S4);
        let mut head = col().w_full().gap(space::S1);
        let mut any = false;
        if let Some(t) = self.title {
            head = head.child(text::title(k, t));
            any = true;
        }
        if let Some(m) = self.meta {
            head = head.child(text::label(k, m));
            any = true;
        }
        if let Some(s) = self.status {
            head = head.child(s);
            any = true;
        }
        if let Some(d) = self.description {
            head = head.child(text::body(k, d));
            any = true;
        }
        if any {
            pane = pane.child(head);
        }
        if !self.left.is_empty() || self.right.is_some() {
            let mut actions = row().w_full().gap(space::S2).center_y().justify_between().child(row().gap(space::S2).center_y().children(self.left));
            if let Some(r) = self.right {
                actions = actions.child(r);
            }
            pane = pane.child(actions);
        }
        if let Some(n) = self.notice {
            pane = pane.child(n);
        }
        for c in self.cards.into_iter().flatten() {
            pane = pane.child(c);
        }
        pane
    }
}
