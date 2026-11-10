//! Cards and the rows that go in them. A card is a labelled surface; its rows share one left
//! edge, one padding and a hairline between them.

use rune::{El, col, el, row};
use rune_kit::Kit;

use super::{color, motion, radius, size, space, text};

/// A labelled group of rows. `build` returns `None` when there are no rows, so an empty group
/// leaves no empty frame behind.
pub struct Card {
    key: String,
    label: Option<String>,
    trailing: Option<El>,
    rows: Vec<El>,
}

impl Card {
    pub fn new(key: impl Into<String>) -> Self {
        Card { key: key.into(), label: None, trailing: None, rows: Vec::new() }
    }

    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }

    /// A control to the right of the label.
    pub fn trailing(mut self, e: El) -> Self {
        self.trailing = Some(e);
        self
    }

    pub fn row(mut self, e: El) -> Self {
        self.rows.push(e);
        self
    }

    pub fn rows(mut self, it: impl IntoIterator<Item = El>) -> Self {
        self.rows.extend(it);
        self
    }

    pub fn build(self, k: &Kit) -> Option<El> {
        if self.rows.is_empty() {
            return None;
        }
        let mut surface = col().w_full().radius(radius::CARD).fill(color::card(k)).border(1.0, color::hairline(k));
        for (i, r) in self.rows.into_iter().enumerate() {
            if i > 0 {
                surface = surface.child(el().w_full().h(1.0).fill(color::hairline(k)));
            }
            surface = surface.child(col().w_full().px(space::S3).py(space::S2).child(r));
        }
        let mut card = col().key(self.key).w_full().gap(space::S1);
        if self.label.is_some() || self.trailing.is_some() {
            let mut head = row().w_full().center_y().px(space::S1).gap(space::S2);
            head = head.child(match self.label {
                Some(l) => text::label(k, l).grow(1.0).min_w(0.0),
                None => row().grow(1.0),
            });
            if let Some(t) = self.trailing {
                head = head.child(t);
            }
            card = card.child(head);
        }
        Some(card.child(surface))
    }
}

/// Label on the left (fixed width), value right-aligned; one line, the value truncates.
pub fn pair_row(k: &Kit, label: impl Into<String>, value: El) -> El {
    pair_row_with(text::label(k, label), value)
}

/// `pair_row` with a ready-made label element (so a caller can key or style it).
pub fn pair_row_with(label: El, value: El) -> El {
    row()
        .w_full()
        .gap(space::S2)
        .center_y()
        .child(label.w(size::PAIR_LABEL))
        .child(row().grow(1.0).min_w(0.0).justify_end().child(value.shrink()))
}

/// Label above, value at full width (it may wrap).
pub fn stacked_row(k: &Kit, label: impl Into<String>, value: El) -> El {
    stacked_row_with(text::label(k, label), value)
}

/// `stacked_row` with a ready-made label element.
pub fn stacked_row_with(label: El, value: El) -> El {
    col().w_full().gap(space::S1).child(label).child(value)
}

/// A label and a value on one line; a value (`text`) too long for the right-hand column goes
/// under the label at full width, where it wraps.
pub fn value_row(label: El, value: El, text: &str) -> El {
    const LONG: usize = 28;
    if text.chars().count() > LONG { stacked_row_with(label, value) } else { pair_row_with(label, value) }
}

/// A name with an optional hint beneath it and an optional control on the right.
pub fn item_row(_k: &Kit, name: El, hint: Option<El>, trailing: Option<El>) -> El {
    let mut body = col().grow(1.0).min_w(0.0).child(name);
    if let Some(h) = hint {
        body = body.child(h);
    }
    let mut r = row().w_full().gap(space::S3).center_y().child(body);
    if let Some(t) = trailing {
        r = r.child(t);
    }
    r
}

/// Makes a card row (an `item_row`, say) a button: it highlights under the pointer and shrinks a
/// little while pressed, unless motion is `reduced`.
pub fn pressable<S: 'static>(k: &Kit, row: El, label: impl Into<String>, reduced: bool, on_press: impl Fn(&mut S) + 'static) -> El {
    row.role("button")
        .label(label)
        .focusable(true)
        .radius(radius::CONTROL)
        .hover(color::hover(k))
        .press_scale(motion::press_scale(reduced))
        .on_press(on_press)
}

/// A form field: label above the input.
pub fn field(k: &Kit, label: impl Into<String>, input: El) -> El {
    col().w_full().gap(space::S1).child(text::label(k, label)).child(input)
}

/// Two fields on one line; `b` has `b_width` when given, otherwise the two share the width.
pub fn field_pair(a: El, b: El, b_width: Option<f32>) -> El {
    let b = match b_width {
        Some(w) => b.w(w),
        None => b.grow(1.0).min_w(0.0),
    };
    row().w_full().gap(space::S3).items_start().child(a.grow(1.0).min_w(0.0)).child(b)
}

/// A full-width row for buttons or compound fields; the card supplies the padding.
pub fn plain_row(e: El) -> El {
    col().w_full().child(e)
}
