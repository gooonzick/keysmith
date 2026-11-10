# Design System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Keysmith one design language in code (`src/ds/`) and rebuild every screen and dialog on it.

**Architecture:** A self-contained module `src/ds/` holds tokens (type, colour, spacing, radius, size, motion) and blocks (card and rows, list row, page and inspector skeletons, status line, dialog frame). It depends only on `rune` and `rune-kit`, so the folder can be copied into Prism. `src/ui/` is rewritten to use only `ds`; a test forbids raw values there. `DESIGN.md` states the rules.

**Tech Stack:** Rust 2024, Rune + rune-kit at git rev `7ead7d63` (source: `~/.cargo/git/checkouts/rune-c9b805e9ed8a3730/7ead7d6/crates/`).

**Spec:** `docs/superpowers/specs/2026-11-10-design-system-design.md`. Read it first; this plan argues from it.

## Global Constraints

- `src/ds/` never imports `crate::app`, `crate::keys`, `crate::sshconfig`, `crate::paths` or `crate::ui`.
- Spacing scale: 4, 8, 12, 16, 24. Radii: 4 tag, 6 control and list row, 8 card, 12 dialog.
- Control heights by place: 24 inspector (`ControlSize::Regular`), 28 lists and filters (`ControlSize::Large`). The spec's 32 for toolbar and dialog footers does not exist in rune-kit; use **28 (`ControlSize::Large`)** there. This is a recorded exception.
- Type roles: Title 15 semibold lh 20; Value 13 regular lh 20 (medium for list names); Code 12 mono lh 18; Label 11 regular lh 16 muted; Hint 11 regular lh 16 faint.
- Text colours: base, muted, faint only. Status colours on dots and icons only, plus a destructive button's label.
- Dialog widths: 440 confirmations, 560 forms. Inspector width 320–400. Pane content edge: 16.
- One contrast button per view. Sentence case, no "please", "…" character (never `...`).
- No information shown today is dropped. No behaviour changes except the "…" overflow menus.
- After every task: `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.
- Commits: one per task, message `feat(ds): …` / `refactor(ui): …`. The repo has no commits yet; the first task's commit is preceded by an initial commit of the existing tree (`chore: initial import`), excluding `assets/icon-concepts/`. Do not push.
- Headless snapshot: `KEYSMITH_HOME=<dir> RUNE_HEADLESS_WAIT=1 RUNE_PERF_WARN=0 cargo run -q -- --snap out.png --scene <name>`; demo data: `scripts/demo-home.sh <dir>`.
- Test harness pattern: `src/ui_tests.rs` (`Runtime::new(state, view, (w,h), Env{system_dark, scale})`, `rt.find(key)` → centre point, `rt.presented(key)` → `([x, y, w, h], opacity)`, `pointer_down/up`, `rt.frame(dt)`).

## Review Focus

1. A very long host name, option value or path: it must truncate or wrap inside its card, never push the card or pane wider. (Tasks 2, 6.)
2. Many notices at once (agent down + several unreadable files): the status line stacks them and the list still gets the remaining height and scrolls. (Tasks 3, 6, 7.)
3. Minimum window 860×540 with the host form open: the dialog fits the window, its body scrolls, the footer stays visible. (Tasks 4, 8.)
4. A host or key with nothing to show in a card (no options, no keys, unused key): the card is omitted, no empty frame, and an empty inspector shows the empty state. (Tasks 6, 7.)
5. Light and dark themes: every `ds` surface and text role is legible in both; reduce-motion leaves only opacity changes. (Tasks 1, 5.)

## File Structure

```
src/ds/mod.rs        re-exports; module doc: "copyable unit, no app imports"
src/ds/tokens.rs     space, radius, size, width consts; type roles; colour roles; motion tokens
src/ds/text.rs       title/value/name/code/label/hint text helpers; truncation helpers
src/ds/status.rs     dot, status (dot + word), tag, notice
src/ds/card.rs       Card builder; pair_row, stacked_row, item_row, field, field_pair
src/ds/controls.rs   sized wrappers over rune-kit Button/IconButton/TextInput/Select; copy button; overflow menu
src/ds/page.rs       page skeleton (filter, status line, list), list_row, empty state
src/ds/inspector.rs  inspector skeleton: head, action row, body
src/ds/dialog.rs     dialog frame (own, on raw Rune)
src/ds/gallery.rs    the `ds` scene: every block, for snapshots
src/ds/tests.rs      headless tests of blocks (state type `()`)
src/ui/*.rs          rewritten on ds (hosts, keys, dialogs, mod)
src/ui_lint.rs       test: no raw values in src/ui/
DESIGN.md            the rules
```

---

### Task 1: Tokens

**Files:** Create `src/ds/mod.rs`, `src/ds/tokens.rs`, `src/ds/tests.rs`; modify `src/main.rs` (`mod ds;`).

**Interfaces — Produces:**

```rust
// tokens.rs
pub mod space { pub const S1: f32 = 4.0; pub const S2: f32 = 8.0; pub const S3: f32 = 12.0; pub const S4: f32 = 16.0; pub const S6: f32 = 24.0; }
pub mod radius { pub const TAG: f32 = 4.0; pub const CONTROL: f32 = 6.0; pub const CARD: f32 = 8.0; pub const DIALOG: f32 = 12.0; }
pub mod size {
    pub const ROW_1: f32 = 32.0; pub const ROW_2: f32 = 44.0; pub const DOT: f32 = 7.0; pub const ICON: f32 = 16.0;
    pub const DIALOG_CONFIRM: f32 = 440.0; pub const DIALOG_FORM: f32 = 560.0; pub const DIALOG_TOP: f32 = 96.0;
    pub const INSPECTOR_MIN: f32 = 320.0; pub const INSPECTOR_MAX: f32 = 400.0; pub const PANE_EDGE: f32 = 16.0;
    pub const PAIR_LABEL: f32 = 120.0;
}
pub enum Place { Inspector, List, Bar }            // -> ControlSize Regular / Large / Large
impl Place { pub fn control(self) -> rune_kit::ControlSize; pub fn height(self) -> f32; }
pub mod ty { // each: fn(&Kit) -> TextStyle
    pub fn title(k) ; pub fn value(k); pub fn name(k) /* value, weight 500 */; pub fn code(k); pub fn label(k); pub fn hint(k); }
pub mod color { // each: fn(&Kit) -> Color
    pub fn text(k); pub fn text_muted(k); pub fn text_faint(k);
    pub fn card(k) /* k.c.layer_01 */; pub fn hairline(k) /* k.c.border_muted */; pub fn selected(k) /* k.c.layer_02 */; pub fn hover(k);
    pub fn ok(k); pub fn warn(k); pub fn bad(k); pub fn info(k); /* the .icon of each StateColors */ }
pub mod motion {
    pub const COPY_CONFIRM_SECS: f64 = 1.2;
    pub fn press_scale(reduced: bool) -> f32 /* 0.97, or 1.0 when reduced */;
    pub fn enter() -> Spring /* ~180 ms, no bounce */; pub fn exit() -> Spring /* ~120 ms */; pub fn swap() -> Spring /* inspector content, ~150 ms */; }
```

`ty::*` derive from `k.ty.ui`, `k.ty.mono`, `k.ty.small` (so the app's font families carry over); only weight, size and line height are set here.

- [ ] **Step 1: Failing tests** in `src/ds/tests.rs`:
  - `scales`: `space` values equal `[4, 8, 12, 16, 24]`; `radius` values equal `[4, 6, 8, 12]`.
  - `reduced_motion_keeps_only_opacity`: `motion::press_scale(true) == 1.0`, `motion::press_scale(false) == 0.97`.
  - `type_ladder`: with `Kit::neutral(false)`: title `(15, 600, lh 20)`, value `(13, 400, 20)`, name weight 500, code size 12 and mono, label `(11, 400, 16)`, hint equal to label.
  - `roles_differ_in_both_themes`: for `dark in [false, true]`: `text != text_muted != text_faint`, `card != selected`.
  - `places`: `Place::Inspector.height() == 24.0`, `List == 28.0`, `Bar == 28.0`.
- [ ] **Step 2:** `cargo test ds::` → FAIL (module missing).
- [ ] **Step 3:** Implement `tokens.rs`, `mod.rs` (`pub use tokens::*;`).
- [ ] **Step 4:** `cargo test ds::` → PASS; clippy clean (add `#![allow(dead_code)]` to `ds/mod.rs` until Task 6 consumes everything; remove it in Task 9).
- [ ] **Step 5:** Commit (initial import first, see Global Constraints).

### Task 2: Text, status and card blocks

**Files:** Create `src/ds/text.rs`, `src/ds/status.rs`, `src/ds/card.rs`; extend `src/ds/tests.rs`.

**Interfaces — Consumes:** Task 1 tokens. **Produces:**

```rust
// text.rs — all return El
pub fn title(k: &Kit, s: impl Into<String>) -> El;      // wraps
pub fn value(k, s) -> El;  pub fn name(k, s) -> El;     // truncate at end
pub fn code(k, s) -> El;                                 // selectable, wrap_anywhere
pub fn path(k, s) -> El;                                 // code role, truncate_middle, one line
pub fn label(k, s) -> El;  pub fn hint(k, s) -> El;     // label: one line; hint: wraps
// status.rs
pub enum Tone { Neutral, Ok, Warn, Bad, Info }
pub fn dot(k, tone: Tone) -> El;                         // 7×7
pub fn status(k, tone: Tone, word: impl Into<String>) -> El;   // dot + label text
pub fn tag(k, text: impl Into<String>, tone: Option<Tone>, help: impl Into<String>) -> El; // h 20, radius TAG, hairline border
pub struct Notice; impl Notice { pub fn new(tone: Tone, text: impl Into<String>) -> Self; pub fn detail(self, s) -> Self; pub fn action(self, button: El) -> Self; pub fn build(self, k) -> El; }
// card.rs
pub struct Card; impl Card {
    pub fn new(key: impl Into<String>) -> Self; pub fn label(self, s) -> Self; pub fn trailing(self, e: El) -> Self; // control right of the label
    pub fn row(self, e: El) -> Self; pub fn rows(self, it: impl IntoIterator<Item = El>) -> Self;
    pub fn build(self, k) -> Option<El>;   // None when it has no rows
}
pub fn pair_row(k, label: impl Into<String>, value: El) -> El;      // label w PAIR_LABEL left, value right-aligned, one line, value truncates
pub fn stacked_row(k, label: impl Into<String>, value: El) -> El;   // label above, value full width
pub fn item_row(k, name: El, hint: Option<El>, trailing: Option<El>) -> El;
pub fn field(k, label: impl Into<String>, input: El) -> El;         // form: label above input
pub fn field_pair(a: El, b: El, b_width: Option<f32>) -> El;        // two fields on one line
pub fn plain_row(e: El) -> El;                                      // a full-width row with the card's padding (buttons, compound fields)
```

Card anatomy: label (`ty::label`, muted) 4 above the surface, inset `S1` from its left; surface `color::card`, 1px `color::hairline` border, radius `CARD`; each row has padding `S3` horizontal, `S2` vertical; a 1px hairline between rows.

- [ ] **Step 1: Failing tests** (state `()`, window 400×600, `scale: 1.0`):
  - `card_rows_share_one_edge`: a card with a `pair_row`, a `stacked_row` and an `item_row` (keys `r1-label`, `r2-label`, `r3-name` on their first text): all three `presented(..).0[0]` are equal.
  - `pair_row_is_one_line_and_truncates`: value of 300 chars; row height `< 40`; `presented("card").0[2] <= 400.0` (Review Focus 1).
  - `stacked_row_wraps_long_value`: 300-char code value; row height `> 40`; card width `<= 400`.
  - `empty_card_is_none`: `Card::new("c").label("X").build(&k).is_none()`.
  - `notice_wraps_and_keeps_its_button`: 200-char text + action button keyed `fix`; `find("fix")` is `Some` and inside the 400 width.
- [ ] **Step 2:** run → FAIL. **Step 3:** implement. **Step 4:** run → PASS. **Step 5:** commit.

### Task 3: Controls, page skeleton, list row, inspector skeleton

**Files:** Create `src/ds/controls.rs`, `src/ds/page.rs`, `src/ds/inspector.rs`; extend `src/ds/tests.rs`.

**Interfaces — Produces:**

```rust
// controls.rs — return rune-kit builders already sized/styled; caller adds on_press/help/key and .build(k)
pub fn button(label: impl Into<String>, place: Place) -> Button;          // Outline
pub fn primary(label, place: Place) -> Button;                            // Contrast
pub fn danger(label, place: Place) -> Button;                             // Danger
pub fn quiet(label, place: Place) -> Button;                              // Ghost
pub fn icon_button(icon: &'static Icon, label: impl Into<String>, place: Place) -> IconButton; // GhostMuted, label = a11y + help
pub fn filter<'a>(edit: &'a TextEdit, placeholder: &str) -> TextInput<'a>; // search icon, clear button only when non-empty, full width, Place::List
pub struct MenuAction<S> { pub label: String, pub disabled: bool, pub run: Box<dyn Fn(&mut S)> }
pub fn overflow<S: 'static>(k, key: &str, open: bool, toggle: impl Fn(&mut S) + 'static, close: impl Fn(&mut S) + 'static, actions: Vec<MenuAction<S>>, layer: &mut Vec<El>) -> El; // "…" icon button + rune_kit::Menu
// page.rs
pub struct Page; impl Page {
    pub fn new(key: impl Into<String>) -> Self; pub fn filter(self, e: El) -> Self;
    pub fn count(self, s: impl Into<String>) -> Self; pub fn notice(self, n: El) -> Self;   // status line: count, then notices stacked
    pub fn rows(self, it: impl IntoIterator<Item = El>) -> Self; pub fn empty(self, e: El) -> Self; // shown when there are no rows
    pub fn build(self, k) -> El;   // padding PANE_EDGE; list scrolls and takes the remaining height
}
pub struct ListRow; impl ListRow {
    pub fn new(key: impl Into<String>, name: impl Into<String>) -> Self; pub fn icon(self, i: &'static Icon) -> Self;
    pub fn meta(self, s: impl Into<String>) -> Self;      // present → height ROW_2, absent → ROW_1
    pub fn tags(self, it: impl IntoIterator<Item = El>) -> Self; pub fn selected(self, on: bool) -> Self; pub fn dim(self, on: bool) -> Self;
    pub fn build<S: 'static>(self, k, on_press: impl Fn(&mut S) + 'static) -> El;  // role button, focusable, radius CONTROL; takes `reduced: bool` via `.reduced(on)` for the press scale
}
pub fn empty(k, title: impl Into<String>, detail: impl Into<String>) -> El;
// inspector.rs
pub struct Inspector; impl Inspector {
    pub fn new() -> Self; pub fn title(self, s) -> Self; pub fn meta(self, s) -> Self; pub fn status(self, e: El) -> Self;
    pub fn description(self, s) -> Self;                   // wraps, under the meta line
    pub fn actions(self, left: Vec<El>, right: Option<El>) -> Self;   // one row, never wraps; ≤ 3 in `left` plus an overflow
    pub fn notice(self, n: El) -> Self;                    // directly under the action row
    pub fn card(self, c: Option<El>) -> Self;              // None is skipped
    pub fn build(self, k) -> El;                           // padding PANE_EDGE, gap S4 between blocks; key "inspector"
}
```

List row layout rule: the row's background extends `S2` past the content edge on both sides (negative horizontal margin on the row, same padding inside), so its icon/name sit exactly on the pane's content edge.

- [ ] **Step 1: Failing tests:**
  - `page_has_one_content_edge`: a `Page` with filter (key `filter`), count (`count`), one notice (`notice-text`) and a `ListRow` (name key `row-name`, no icon): the left x of filter, count text, notice dot-or-text container and `row-name` are all `16.0` (±0.5).
  - `list_row_heights`: with meta → `presented(row).0[3] == 44.0`; without → `32.0`.
  - `many_notices_leave_the_list_scrollable`: 6 notices + 40 rows in a 600-high window: the last notice is visible, `find` of row 0 is `Some`, page height `<= 600` (Review Focus 2).
  - `inspector_actions_stay_on_one_row`: three `button(..)` + overflow + right `danger`, pane width 320: all five share one centre y; right button's right edge is `320 - 16`.
  - `overflow_opens_and_runs`: click the "…" trigger → menu open flag set by `toggle`; click the item → its `run` is called.
- [ ] **Step 2–5:** fail → implement → pass → commit.

### Task 4: Dialog frame

**Files:** Create `src/ds/dialog.rs`; extend `src/ds/tests.rs`. Reference: rune-kit `dialog.rs:38-60` for the modal wiring (`.modal(true, close)`, `.modal_click_outside`, scrim, `role("dialog")`, `label`).

**Interfaces — Produces:**

```rust
pub enum DialogKind { Confirm, Form }       // width DIALOG_CONFIRM / DIALOG_FORM
pub struct DialogFrame; impl DialogFrame {
    pub fn new(key: impl Into<String>, title: impl Into<String>, kind: DialogKind) -> Self;
    pub fn body(self, e: El) -> Self;       // repeated; gap S4; this region scrolls when the dialog is taller than the window
    pub fn error(self, e: Option<String>) -> Self;   // danger dot + icon + text above the footer
    pub fn footer(self, cancel: El, primary: El) -> Self;
    pub fn menus(self, m: Vec<El>) -> Self; // dropdown overlays; pushed to the layer AFTER the dialog (they anchor to fields inside it)
    pub fn build<S: 'static>(self, k, on_close: impl Fn(&mut S) + Clone + 'static, layer: &mut Vec<El>);
}
```

Anatomy: scrim; panel top at `DIALOG_TOP` (or 16 when the window is shorter than `DIALOG_TOP + 400`), horizontally centred, `max_h = window − top − 16`; radius `DIALOG`; header (title in `ty::title`, close icon button "Close"), padding `S4`; body; footer right-aligned, gap `S2`, `Place::Bar` buttons. Enter motion `motion::enter()`, exit `motion::exit()`. Elements keyed `{key}-dialog`, `{key}-body`, `close`.

- [ ] **Step 1: Failing tests:**
  - `dialog_widths`: Confirm → panel width 440; Form → 560 (window 1200×760).
  - `dialogs_share_a_top`: a short and a tall dialog have the same panel y (`96`).
  - `tall_form_fits_a_small_window`: window 860×540, body 900 high: panel bottom `<= 540 − 16`, footer button `find` is `Some` and inside the window (Review Focus 3).
  - `escape_and_close_button_close`: pressing Escape and clicking `close` each call `on_close`.
  - `menu_layer_order`: with one `menus` element, it is the last element pushed to `layer`.
- [ ] **Step 2–5:** fail → implement → pass → commit.

### Task 5: Motion and the gallery scene

**Files:** Create `src/ds/gallery.rs`; modify `src/ds/controls.rs` (copy button), `src/ds/page.rs`/`card.rs` (press scale, layout animation), `src/main.rs` (scene), `src/app/mod.rs` (one field), `src/ui/mod.rs` (branch).

**Interfaces — Produces:**

```rust
// controls.rs
pub fn copy_button(label: impl Into<String>, place: Place, copied: bool) -> Button; // icon COPY, or CHECK when `copied`
// gallery.rs
pub fn gallery<S: 'static>(k: &Kit, layer: &mut Vec<El>) -> El;  // every block once: type ladder, statuses, tags, notices, a card of each row kind, list rows (both heights, selected), inspector, page
// app: `pub gallery: bool`; scene ("ds", |s| s.gallery = true); ui::view returns ds::gallery when set.
```

Motion realisation (spec "Motion"; where Rune cannot do one, leave it out and list it under "Gaps" in the task report):
- Press: `press_scale(motion::press_scale(reduced))` on list rows and item rows that are pressable.
- Selection: selected row background carries `animate_layout()` under one shared key so it moves between rows; if Rune cannot share a background across rows, keep the instant change.
- Dialog/menu: done in Task 4 via `token_motion`; set `origin` toward the trigger where the trigger rect is known.
- Inspector swap: the inspector body is keyed by the selected object's id and has `transition` (opacity only) with `motion::swap()`.
- Copy: app state holds `copied_at: Option<(String /*what*/, f64 /*time*/)>`; `poll` clears it after `COPY_CONFIRM_SECS`; the button shows the check while set. (`copied_at` lives in `App`, not `ds`.)
- Reduce motion: when `cx.is_reduced_motion()`, pass a flag so `press_scale` and layout animation are skipped; opacity transitions stay.

- [ ] **Step 1: Failing tests:**
  - `gallery_renders_in_both_themes`: `Runtime<()>` with `system_dark` false and true: frame renders; `find("ds-gallery")` is `Some`.
  - `copy_button_shows_a_check_then_reverts` (in `src/ui_tests.rs`): click `host-copy`; `state.copied_at` is `Some`; advance frames by 1.5 s; it is `None`.
- [ ] **Step 2:** fail. **Step 3:** implement. **Step 4:** pass.
- [ ] **Step 5:** Snapshot `--scene ds` in dark and light (light: check how the headless run picks the theme; if only via `Env`, render light through the test harness's snapshot facility or report it as a gap). Look at both images: every text role is legible, cards are visible against the pane (Review Focus 5). Fix token mappings if not.
- [ ] **Step 6:** Commit.

### Task 6: Hosts page and inspector on ds

**Files:** Rewrite `src/ui/hosts.rs`; modify `src/ui/mod.rs` (remove helpers replaced by `ds`: `heading`, `dot`, `empty`, `tag`, `field`, `mono`, `kv_row`, `inspector_body`, `notice_line`), `src/app/hosts.rs` (`host_menu: bool` on `App`), `src/ui_tests.rs`.

**Consumes:** Tasks 1–5. Element keys that tests click stay the same unless listed: `host-{d}-{s}`, `add-host`, `filter`, `host-edit`, `host-duplicate`, `host-remove`, `host-copy`, `host-terminal`, `host-test`, `identity-{i}-show`, `effective-toggle`. Changed: `host-up` / `host-down` are now menu items inside `host-more` (keys `host-more`, items keyed `host-up`, `host-down`).

Layout:
- **Page:** `Page` with filter, count (`"3 hosts in 2 files"`), one notice per `config_errors` entry (`Tone::Warn`), rows. Row: `ListRow` with icon `SERVER`, name = patterns, meta = `target(sec)` (omitted when empty), tags as today (match / pattern / key names with tone / file name).
- **Inspector:** title = patterns; meta = `"Host · ~/.ssh/config, line 3"`; description = the section's comment lines. Actions: `Edit…`, `Duplicate…`, overflow (`Move up`, `Move down`, disabled at the ends); right: `Remove…` (danger). Cards, in order, each omitted when empty:
  1. unlabelled card: `plain_row` with Copy / Open in Terminal / Test connection (`Place::Inspector`), and the test result as a second row (`status` + text) when present. Only for sections with an alias.
  2. `Connection`: `pair_row`s Address, User, Port, Jump through — only those the section sets.
  3. `Keys`: one `item_row` per IdentityFile: name = `path(value)`, hint = kind and state, trailing = status ("in the agent" / "missing") and a quiet `Show key` button.
  4. `Options`: `pair_row` per other option, label = option name in `code` role, value in `code` role.
  5. `Inherited and defaults` with trailing toggle `All options` / `Fewer`: rows as today's `effective()` (`pair_row`, canonical key names); `Asking ssh…` and error states as a single row; text `Nothing beyond what's set above.` when empty.
- No host selected → `ds::empty("No host selected", "Select a host to see its settings and keys.")`.

- [ ] **Step 1: Failing tests** in `src/ui_tests.rs`:
  - Update `remove_asks_first_and_move_reorders`: click `host-more`, then `host-down`.
  - `hosts_inspector_is_cards`: `find("card-connection")`, `find("card-keys")`, `find("card-inherited")` are `Some` for `work`; for `Host *` (no alias) `card-connection` is `None` and no empty card frame exists (Review Focus 4).
  - `long_values_stay_inside_the_inspector`: add a host whose `ProxyCommand` is 400 chars; `presented("inspector").0[2] <= 400.0` and equals its width before (Review Focus 1).
  - `unreadable_include_shows_as_a_notice`: config includes a directory path that is a file with mode 000; a notice with the file's name exists and `host-0-0` is still found (Review Focus 2).
- [ ] **Step 2:** fail. **Step 3:** rewrite. **Step 4:** all tests pass; clippy clean.
- [ ] **Step 5:** Snapshot `--scene hosts`; check: one left edge in the middle pane, cards in the inspector, action row on one line. **Step 6:** Commit.

### Task 7: Keys page and inspector on ds

**Files:** Rewrite `src/ui/keys.rs`; modify `src/app/keys.rs` (`key_menu: bool`), `src/ui_tests.rs`.

Keys kept: `nav-keys`, `new-key`, `key-{name}`, `key-passphrase`, `key-trash`, `key-host-{i}`. New: `key-more` with items `key-write-pub`, `key-reveal`.

Layout:
- **Page:** filter; count (`"3 keys"`); notices: the agent, always (`Tone::Ok` "ssh-agent holds 2 keys" with detail "1 from outside ~/.ssh" when so, action `Remove all`; or `Tone::Warn` "ssh-agent isn't running" with the reason as detail); one `Tone::Warn` per `key_errors`. Rows: `ListRow`, icon `KEY`, meta as today, tags as today.
- **Inspector:** title = name; meta = `"ED25519 · 256 bits · no passphrase"`. Actions: `Add to agent` / `Remove from agent`, `Set passphrase…` / `Change passphrase…`, `Edit comment…`; overflow: `Write .pub file` (only when missing), `Show in Finder`; right: `Move to Trash…`. Notice under the actions when the private key is readable by others: "Others can read this key" with action `Fix`. Cards: `Public key` (trailing `copy_button`, one `stacked_row` with the key in `code`); `Details` (`stacked_row` Fingerprint, `pair_row` Comment); `Files` (`pair_row` per file: label = `path`, value = mode); `Used by` (one `item_row` per host, pressable, opens the host; when no host uses the key, a single hint row "No host uses this key." so the fact stays visible).

- [ ] **Step 1: Failing tests:**
  - `keys_page_matches_hosts_page_skeleton`: on both pages the filter's rect is identical and the first row's name starts at the same x.
  - `agent_notice_keeps_remove_all`: with a running agent holding a key (harness from `keys_tests.rs`), `find("agent-clear")` is `Some`; clicking it empties the agent.
  - `key_more_menu_reveals_and_writes_pub`: delete `id_work.pub`; open `key-more`; `key-write-pub` exists; click → file exists again.
  - `unused_key_says_so`: a key no host references shows the hint row text (Review Focus 4).
- [ ] **Step 2–4:** fail → rewrite → pass. **Step 5:** snapshot `--scene keys`. **Step 6:** commit.

### Task 8: Dialogs on the ds frame

**Files:** Rewrite `src/ui/dialogs.rs`; modify `src/ui_tests.rs`.

All four dialogs use `DialogFrame`; `Select` menus go through `.menus(..)` (keeps the fix for dropdowns anchoring to fields inside the dialog; test `a_dialogs_dropdown_opens_under_its_field` must still pass). Keys kept: `host-save`, `host-cancel`, `host-name`, `host-extra-add`, `host-extra-{i}-key|value|remove`, `host-identity-pick-trigger`, `host-file-trigger`, `key-create`, `key-type-trigger`, `pass-save`, `confirm-ok`, `confirm-cancel`.

- **Add / Edit host** (`Form`): cards `Host` (Save in — only when adding and more than one file; Name; Description), `Connection` (`field_pair` Address + Port (96 wide); `field_pair` User + Jump through), `Key` (path input + key picker as one full-width row), `Other options` (the name/value rows, `Add option`, hint). The option rows' scroll area keeps room for the focus ring (current fix in `extra`).
- **New key** (`Form`): cards `Key` (Type + hint; File name; Comment), `Passphrase` (two inputs + show, hint), `Agent` (two checkboxes with descriptions).
- **Passphrase / comment** (`Confirm` width): one card with the fields.
- **Confirmations** (`Confirm`): body text in `value` role, footer Cancel + danger button.
- Errors: `DialogFrame::error`.

- [ ] **Step 1: Failing tests:**
  - `dialogs_come_in_two_widths`: open each of the four dialogs; panel widths ∈ {440, 560}; all four panels share the same top y.
  - `host_form_fits_the_minimum_window`: window 860×540, add 6 option rows: `host-save` is found inside the window; the body scrolls (Review Focus 3).
  - `fields_sit_above_their_inputs`: in the host form, the `Name` label's bottom is `<=` the `host-name` input's top, and both share a left edge.
- [ ] **Step 2–4:** fail → rewrite → pass (all existing dialog tests too). **Step 5:** snapshots `add-host`, `host-form`, `new-key`, `passphrase`, `remove`. **Step 6:** commit.

### Task 9: Shell, lint, DESIGN.md

**Files:** Modify `src/ui/mod.rs` (sidebar spacing from `space::*`, toolbar buttons via `ds::primary(.., Place::Bar)` / `ds::icon_button`, inspector width limits, auto-collapse); create `src/ui_lint.rs`, `DESIGN.md`; modify `README.md` (module table: add `ds`), `src/ds/mod.rs` (remove `allow(dead_code)`; delete anything unused), `src/main.rs` (`mod ui_lint;` under `cfg(test)`).

- Inspector width: clamp to `INSPECTOR_MIN..=INSPECTOR_MAX` (see how `SplitView`/pane state exposes width; if it only takes a fixed width, use 360 and record it). Auto-collapse: when the window is narrower than `sidebar + 480 + INSPECTOR_MIN`, the inspector is hidden (app-side, in `view`, using the window width from `Cx`), and comes back when the window widens.
- **Lint test** `no_raw_values_in_ui`: reads every `src/ui/*.rs` and fails, naming file and line, on any of: `TextStyle::new`, `k.ty.`, `k.c.`, `k.radius.`, and the regex `\.(gap|padding|px|py|pt|pb|pl|pr|mt|mb|ml|mr|mx|my|radius)\(\s*-?[0-9]`. Allowed: `space::`, `radius::`, `size::` constants and `ds` calls. (Sizes such as `.w(96.0)` for a field are allowed; move repeated ones into `size::`.)
- **DESIGN.md**: the spec's "The language" and "Relation to rune-kit" sections rewritten as rules with the final token names, the block list with one usage line each, the recorded exceptions (no 32 control height → 28; kit typography in sidebar, toolbar title, toasts; any motion gap found in Task 5), and "How to add a screen" (pick a skeleton, use blocks, never a raw value). Add a one-line pointer in `CLAUDE.md`/`AGENTS.md` (create `AGENTS.md` if neither exists): "Read DESIGN.md before any UI change."

- [ ] **Step 1:** Write `no_raw_values_in_ui` and `inspector_hides_in_a_narrow_window` (window 860×540 → `find("inspector")` is `None`; 1200×760 → `Some`). **Step 2:** fail (lint lists what is left).
- [ ] **Step 3:** Fix every reported line; implement widths and collapse; write `DESIGN.md`.
- [ ] **Step 4:** `cargo test`, clippy → clean.
- [ ] **Step 5:** Snapshots of all scenes (`hosts`, `host-form`, `add-host`, `keys`, `new-key`, `passphrase`, `remove`, `ds`); check against the spec's "Done when". **Step 6:** Commit.
