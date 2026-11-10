# Keysmith design rules

Keysmith's screens are built from one design system, `src/ds/`. This file states its rules for
people and agents. The code is the source of truth for every number: when this file and
`src/ds/tokens.rs` disagree, the code wins and this file is wrong.

`src/ds/` depends only on `rune` and `rune-kit`, never on `crate::app`, `crate::keys`,
`crate::sshconfig`, `crate::paths` or `crate::ui`, so the folder can be copied into another
Rune app (Prism is next) unchanged.

## The one rule

**A screen never picks a raw value.** Code in `src/ui/` uses `ds` blocks and the `space::`,
`radius::` and `size::` constants. It contains no `TextStyle::new`, no `k.ty.*`, `k.c.*` or
`k.radius.*`, and no number passed to `gap`, `padding`, `px`/`py`/`pt`/`pb`/`pl`/`pr`,
`mt`/`mb`/`ml`/`mr`/`mx`/`my` or `radius`. The test `ui_lint::no_raw_values_in_ui` fails on any
of these and names the file and line.

Widths of individual fields (`.width(220.0)` on a select) are allowed. A width used in more than
one place goes into `size::`.

If a screen needs a value `ds` does not have, add it to `src/ds/tokens.rs` (or a block to `ds`),
never at the call site. A number may be tuned when a screen shows it is wrong; it is tuned in
the token module, and this file is updated with it.

## Type roles

One ladder. A screen picks a role, never a size. Functions in `ds::text` (styles in `ds::ty`).

| Role | `ds::text::` | Style | Colour | Used for |
| --- | --- | --- | --- | --- |
| Title | `title` | 15 semibold, line 20, wraps (inside a word too long for a line) | base | The object's name in the inspector, dialog titles |
| Value | `value` | 13 regular, line 20, truncates at the end | base | Values |
| Name | `name` | Value in medium (500), truncates | base | Names in lists and item rows |
| Body | `body` | Value, wraps | base | Sentences: descriptions, confirmation text, notices |
| Code | `code` | 12 mono, line 18, breaks anywhere, selectable | base | Commands, option names and values, fingerprints, public keys |
| Path | `path` | Code, one line, cut in the middle, selectable | base | File paths |
| Label | `label` | 11 regular, line 16, truncates | muted | Field labels, card labels, list meta lines, counts |
| Hint | `hint` | 11 regular, line 16, wraps | faint | Help under a field, "Asking ssh…", empty-state detail |

A label is always quieter than its value. There are no bold section headings and no dividers:
card labels and cards carry the structure. rune-kit's 14pt and its older 12/13 styles are not
used (see the exceptions for where kit typography remains).

## Colour roles

`ds::color`. Text is `text` (base), `text_muted` or `text_faint`, nothing else.

Status colours `ok`, `warn`, `bad`, `info` go on dots and icons only, never on text or fills.
The one exception is a destructive button's label (`ds::danger`). Status is always a dot (or an
icon) plus a word; colour is never the only signal.

Surfaces: `pane` (a page or inspector), `card` (one step above the pane, with a `hairline`
border), `selected` (the selected row), `hover`. Each maps to one rune-kit palette entry, so both
themes come from the kit.

## Spacing, radii, sizes

- Everything is a multiple of 4. `space::S1` 4, `S2` 8, `S3` 12, `S4` 16, `S6` 24. The one
  exception is `space::ROW_GAP` (2) between list rows and sidebar rows.
- Radii nest: `radius::TAG` 4, `CONTROL` 6 (controls and list rows), `CARD` 8, `DIALOG` 12.
- Control heights by place, `ds::Place`: `Inspector` 24 (kit `ControlSize::Regular`); `List` 28
  (lists, filters, status-line buttons) and `Bar` 28 (toolbar, dialog footers), both kit
  `ControlSize::Large`. Every button takes a `Place`; none picks a `ControlSize`.
- `size::`: `ROW_1` 32 and `ROW_2` 44 (list rows with one and two lines; sidebar rows too),
  `DOT` 7, `ICON` 16, `PANE_EDGE` 16, `PAIR_LABEL` 120 (a pair row's label column, wider for a
  longer label), `ROW_NAME_MIN` 120 (a list row's name, which its tags give way to),
  `FIELD_SHORT` 96 (a port beside an address), `FOCUS_RING` 3 (room left around fields for their
  focus ring), `DIALOG_CONFIRM` 440, `DIALOG_FORM` 560, `DIALOG_TOP` 96, `INSPECTOR_MIN` 352,
  `INSPECTOR_MAX` 400, `LIST_MIN` 276.

## Layout

1. **One content edge per pane**, `size::PANE_EDGE` (16) from the pane's border. Filter, status
   line, row text and cards sit on it. A row's hover and selection background reaches `S2` past
   it; its text does not.
2. **One page skeleton** (`ds::Page`): toolbar → filter → status line → list. The count and
   every notice (ssh-agent state, unreadable files) live in the status line.
3. **One inspector skeleton** (`ds::Inspector`): head → action row → notice → cards. Everything
   below the action row is in a card.
4. **One list row anatomy** (`ds::ListRow`) on every page.
5. **Two dialog widths**: 440 for confirmations (`DialogKind::Confirm`), 560 for forms
   (`DialogKind::Form`). Every dialog's top sits at `DIALOG_TOP`, so dialogs of different
   heights open in the same place.
6. **One primary (contrast) button per view** (`ds::primary`): the toolbar's add button on a
   page, the main action in a dialog.
7. **Long text**: names truncate at the end, paths in the middle, values in a stacked row wrap,
   titles wrap and break inside a word that does not fit. A pair row's label always shows in
   full: its column is `PAIR_LABEL` or as wide as the label, and the value takes the rest and
   truncates (its tooltip is the whole value). `value_row` keeps a row on one line only when it
   fits the narrowest inspector: label (counted as at least 19 characters, the column) plus
   value at most 44 characters; otherwise the value moves under the label (a stacked row). In a
   list row the name keeps `ROW_NAME_MIN`; tags past it are cut.
8. **Pane widths**: the sidebar is the kit's resizable pane (232 by default); the inspector is
   `INSPECTOR_MIN`..=`INSPECTOR_MAX` (352-400); the list takes the rest and keeps at least
   `LIST_MIN` (276). When space is tight the inspector is drawn narrower than the width the user
   chose, down to `INSPECTOR_MIN`; the stored width is never changed, so it returns to it when the
   window widens. Below sidebar + `LIST_MIN` + `INSPECTOR_MIN` (276 + 352) the inspector hides,
   and it comes back when there is room again (`ui::inspector_pane`). The 860 minimum window with
   the default sidebar shows all three panes: sidebar 232, list 276, inspector 352. A wider sidebar
   hides the inspector first. In a 276 list the toolbar title is at the leading edge and truncates
   (`ds::toolbar_title`), and the filter placeholder is short enough for one line.
9. **Words**: sentence case, no "please", an ellipsis is the "…" character (never `...`).
10. **No empty frames**: a card with no rows is not drawn (`Card::build` returns `None`); an
    inspector with nothing selected shows an empty state (`Inspector::empty`): "Select a … to
    see …".

## Motion

`ds::motion`. Durations and curves are named tokens like every other value.

- `enter()` ~180 ms and `exit()` ~120 ms, no bounce: dialogs and menus arrive slower than they
  leave.
- `swap()` ~150 ms: the inspector's content cross-fades when the selection changes (opacity
  only, `Inspector::subject`).
- `press_scale(reduced)`: list rows and pressable card rows shrink to 0.97 under the pointer.
- `panes()`: the sidebar and inspector open and close on one spring, 150–300 ms by width.
- `COPY_CONFIRM_SECS` (1.2 s): a copy button shows a check and "Copied", then reverts.
- `DIM` (0.75): the opacity of a de-emphasised row (a pattern host).
- Keyboard actions done many times a day are instant: ⌘N and Enter open their dialog at once,
  arrow keys move the selection and change the inspector at once (`tx.instant()`).
- Under reduce motion only opacity changes remain: pass `cx.is_reduced_motion()` into
  `ListRow::reduced`, `pressable` and `SplitView::reduced_motion`.

## Blocks

| Block | Use it for |
| --- | --- |
| `Page` | A list page: filter, count, notices, then rows or an empty state; the rows scroll. |
| `ListRow` | A row in a `Page` list: icon, name, meta line, tags; 32 or 44 high. |
| `empty` | What is missing and what to do, in place of a list or an inspector. |
| `Inspector` | The right pane: title, meta, status, description, action row, notice, cards; `Inspector::empty` when nothing is selected. |
| `Card` | A labelled group of rows on a bordered surface, hairlines between rows, an optional trailing control by the label. |
| `pair_row` / `pair_row_with` | Label left (in full), short value right, one line. |
| `stacked_row` / `stacked_row_with` | Label above a long value that wraps: public keys, fingerprints, commands. |
| `value_row` | A pair row that becomes a stacked row when label and value don't fit one line. |
| `item_row` | A name with a hint under it and a trailing status or button: keys of a host, hosts of a key, files. |
| `pressable` | Makes a card row a button (hover, press scale). |
| `field` | A form field: label above the input. |
| `field_pair` | Two fields on one line (Address + Port, User + Jump through). |
| `plain_row` | A full-width card row for buttons or compound fields. |
| `button` | The ordinary (outlined) button. |
| `primary` | The one contrast button of a view. |
| `danger` | A destructive action; the only coloured label. Pinned right in an action row. |
| `quiet` | A secondary action with no outline ("All options", "Show key", "Add option"). |
| `copy_button` | A button that copies and confirms with a check. |
| `icon_button` | An icon-only button; its label is the accessible name and tooltip. |
| `filter` | The search field at the top of a page, with a clear button once there is text. |
| `overflow` + `MenuAction` | The "…" button and its menu for actions beyond the visible three. |
| `dot`, `status`, `tag` | A 7pt status dot; dot plus a word; a small outlined label with an optional dot. |
| `Notice` + `Tone` | A dot, a wrapping message, an optional detail and button; in the status line, under an action row or in a card. |
| `DialogFrame` + `DialogKind` | Every dialog: title and close button, a body of cards that scrolls, an error line, a footer with Cancel and one main action. |
| `gallery` | Every block on one screen: `cargo run -- --snap out.png --scene ds`. |

An inspector's action row holds at most three buttons plus the overflow on the left and one
action (usually the destructive one) on the right. It never wraps; the rest goes in "…".

## Relation to rune-kit

`ds` wraps rune-kit's controls (Button, IconButton, TextInput, Select, Checkbox, Menu, Sidebar,
Toolbar, Toast, SplitView) and uses the nearest value the kit offers. Where the kit cannot match
a rule, the difference is a recorded exception below, not a reimplementation.

The dialog frame is the one block `ds` builds on raw Rune, because the kit's dialog is centred,
has radius 6, a smaller title and no footer. It keeps what the kit's dialog provides: focus trap,
Escape and outside press to close, focus return, the close button, the dialog role.

Raw values and kit calls live in `src/ds/`. The lint covers `src/ui/` only.

## Recorded exceptions

- **No 32-point controls.** The kit's heights are 16/20/24/28/36. Toolbar and dialog footer
  buttons use 28 (`Place::Bar`, `ControlSize::Large`), not the 32 first planned.
- **Kit typography stays** in sidebar rows, the toolbar title and toasts. Sidebar rows do take
  `ROW_1`/`ROW_2` heights and the spacing scale.
- **Toolbar edge 12.** `SplitView` pads the toolbar 12 from the main pane's edge and offers no
  setter, so the toolbar's buttons sit 4 outside the page's 16 content edge.
- **Menu items are keyed by index.** rune-kit's `Menu` has no item key setter, so overflow items
  are `<key>-popup-item<i>`. Menu lists keep a stable order and disable an item that does not
  apply rather than hide it, so each index keeps its meaning.
- **Dialog top in a short window.** The rule "top 16 when the window is too short for
  `DIALOG_TOP`" is not implemented: Rune gives `build` no window size, and the window's minimum
  height is 540. The dialog shrinks instead and its body scrolls; the footer stays visible.
- **Inspector minimum 352.** The spec's 320 was tuned after a screen showed it was too narrow:
  the Keys action row with "Remove from agent" overflows by 22 points at 320.
- **Copy has no toast.** A host's Copy confirms on the button itself; the old "Copied" toast is
  gone.

### Motion gaps

Rune cannot do these well, so they are left out rather than faked:

- **No selection slide.** The selection jumps to the new row: Rune's matched-element motion
  paints above the window and would cover the destination row.
- **Dialogs grow about their centre**, not from the control that opened them (scale 0.97 to 1).
- **Buttons have no press scale.** Only list rows and pressable card rows shrink under the
  pointer; kit buttons show a press by darkening their fill.

## How to add a screen

1. **Pick a skeleton.** A list of things is a `Page` of `ListRow`s with a `Toolbar` whose one
   `primary` adds a thing. Details of one thing are an `Inspector`. A question or a form is a
   `DialogFrame` of the right `DialogKind`.
2. **Fill it with blocks.** Group details in `Card`s of `pair_row`, `stacked_row`, `item_row`;
   group form fields in `Card`s of `field` and `field_pair`. Put state in `Notice`s and
   `status`; put actions in `button`, `quiet`, `danger`, with the rest in `overflow`.
3. **Pick roles, not values.** Text through `ds::text`, spacing from `space::`, sizes from
   `size::`, buttons with a `Place`. Never a raw number, style or colour.
4. **Check it.** `cargo test` (the lint included) and `cargo clippy --all-targets -- -D warnings`;
   render the scene headless and look at it in both themes. If a block is missing, add it to
   `ds` and to the gallery first.
