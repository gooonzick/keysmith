//! Controls: buttons, the list filter and the overflow menu. Each returns a rune-kit builder
//! already sized for its place; the caller adds `on_press`, `help` and a key, then builds it.

use std::rc::Rc;

use rune::{El, Icon, TextEdit};
use rune_kit::{Button, ButtonVariant, IconButton, IconButtonVariant, Kit, Menu, MenuItem, TextInput, glyphs};

use super::Place;

/// Three dots: rune-kit's glyph set has no "more".
static MORE: Icon = Icon {
    name: "ds-more",
    view: 16.0,
    body: r#"<circle cx="3.5" cy="8" r="1.1" fill="currentColor"/><circle cx="8" cy="8" r="1.1" fill="currentColor"/><circle cx="12.5" cy="8" r="1.1" fill="currentColor"/>"#,
};

fn with(label: impl Into<String>, place: Place, variant: ButtonVariant) -> Button {
    Button::new(label).variant(variant).control_size(place.control())
}

/// The ordinary button: outlined.
pub fn button(label: impl Into<String>, place: Place) -> Button {
    with(label, place, ButtonVariant::Outline)
}

/// The one main action of a view.
pub fn primary(label: impl Into<String>, place: Place) -> Button {
    with(label, place, ButtonVariant::Contrast)
}

/// A destructive action.
pub fn danger(label: impl Into<String>, place: Place) -> Button {
    with(label, place, ButtonVariant::Danger)
}

/// A button with no outline, for secondary actions.
pub fn quiet(label: impl Into<String>, place: Place) -> Button {
    with(label, place, ButtonVariant::Ghost)
}

/// An icon-only button; `label` is its accessible name and tooltip.
pub fn icon_button(icon: &'static Icon, label: impl Into<String>, place: Place) -> IconButton {
    let label = label.into();
    IconButton::new(icon).variant(IconButtonVariant::GhostMuted).control_size(place.control()).label(label.clone()).help(label)
}

/// The list filter: a search field at full width with a clear button once there is text. The
/// caller adds `label`, `on_edit` and a key.
pub fn filter(edit: &TextEdit, placeholder: &str) -> TextInput {
    let mut input = TextInput::new(edit).placeholder(placeholder).leading_icon(&glyphs::SEARCH).width(None);
    if !edit.text.is_empty() {
        input = input.clear_button();
    }
    input
}

/// One entry of an overflow menu.
pub struct MenuAction<S> {
    pub label: String,
    pub disabled: bool,
    pub run: Box<dyn Fn(&mut S)>,
}

/// A "more" button that opens a menu of `actions`. The menu goes into `layer` and anchors to the
/// button (keyed `<key>-button`); choosing an item closes the menu, then runs its action.
pub fn overflow<S: 'static>(
    k: &Kit,
    key: &str,
    open: bool,
    toggle: impl Fn(&mut S) + 'static,
    close: impl Fn(&mut S) + 'static,
    actions: Vec<MenuAction<S>>,
    layer: &mut Vec<El>,
) -> El {
    let trigger = icon_button(&MORE, "More actions", Place::Inspector).expanded(open).on_press(toggle).build(k).key(format!("{key}-button"));
    let close = Rc::new(close);
    let mut menu = Menu::new(key).trigger(trigger).open(open).on_close({
        let close = close.clone();
        move |s: &mut S| close(s)
    });
    for a in actions {
        let close = close.clone();
        let run = a.run;
        menu = menu.item(MenuItem::new(a.label).disabled(a.disabled).on_select(move |s: &mut S| {
            close(s);
            run(s);
        }));
    }
    menu.build(k, layer)
}
