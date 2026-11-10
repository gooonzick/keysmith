//! Text blocks: one function per type role, so screens never pick fonts or colours themselves.

use rune::{El, text};
use rune_kit::Kit;

use super::{color, ty};

fn styled(s: impl Into<String>, style: rune::TextStyle, c: rune::Color) -> El {
    text(s.into()).font(style).color(c)
}

/// A heading; wraps.
pub fn title(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::title(k), color::text(k)).wrap()
}

/// A value; one line, cut with an ellipsis at the end.
pub fn value(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::value(k), color::text(k)).truncate()
}

/// A list name (medium weight); one line, cut at the end.
pub fn name(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::name(k), color::text(k)).truncate()
}

/// Code or a long token: selectable, breaks anywhere.
pub fn code(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::code(k), color::text(k)).wrap_anywhere().selectable(true).selection_color(k.selection())
}

/// A path: code face, one line, the middle cut when it does not fit.
pub fn path(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::code(k), color::text(k)).truncate_middle()
}

/// A small muted caption; one line.
pub fn label(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::label(k), color::text_muted(k)).truncate()
}

/// Faint helper text; wraps.
pub fn hint(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::hint(k), color::text_faint(k)).wrap()
}

/// A value that wraps instead of truncating: messages and sentences.
pub fn body(k: &Kit, s: impl Into<String>) -> El {
    styled(s, ty::value(k), color::text(k)).wrap()
}
