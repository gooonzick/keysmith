//! The inspector: a title block, one row of actions, a notice and a card, stacked on one edge.

use rune::{El, SpringPair, Transition, col, row};
use rune_kit::Kit;

use super::{motion, size, space, text};

#[derive(Default)]
pub struct Inspector {
    subject: Option<String>,
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

    /// What the inspector shows (a host's or key's id). When it changes, the old content fades
    /// out and the new fades in; with no subject the content just changes. The content is keyed
    /// `inspector-<id>`.
    pub fn subject(mut self, id: impl Into<String>) -> Self {
        self.subject = Some(id.into());
        self
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

    /// One row: up to three buttons and an overflow on the left, one more (usually the
    /// destructive one) pinned right. When the left buttons don't fit the pane they wrap onto a
    /// second line instead of pushing the pane wider.
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
        let mut pane = col().w_full().gap(space::S4);
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
            // The left group wraps rather than push the pane wider when its buttons don't fit
            // (a key's labels are long); the right one stays pinned to the first line.
            let left = row().grow(1.0).min_w(0.0).gap(space::S2).center_y().flex_wrap().children(self.left);
            let mut actions = row().w_full().gap(space::S2).items_start().justify_between().child(left);
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
        // Opacity only, so it stays under reduced motion.
        if let Some(id) = self.subject {
            pane = pane.key(format!("inspector-{id}")).transition(Transition::opacity().springs(SpringPair::new(motion::swap(), motion::swap())));
        }
        col().key("inspector").w_full().padding(size::PANE_EDGE).child(pane)
    }
}
