# Design system for Keysmith (and, later, Prism)

Date: 2026-11-10. Status: draft for George's approval.

## Why

George ships several desktop apps on Rune and rune-kit: Keysmith (this repo) and Prism
(`~/Development/skills-explorer`). He wants them to share one design language, so they read as
products by one author.

Today Keysmith has no system. Its inspector (the right pane) is hard to read: almost all of it is
set in 11pt, labels and values differ only by colour, and there are no groups. The composition
feels chaotic everywhere: panes have several left edges, pages are structured differently,
dialogs come in four widths, and ten different gaps are in use.

Prism has written rules (`DESIGN.md`, `docs/design/`) and a few helpers, but no token module and
no spacing scale. George agreed that the language itself may be revisited, and that Prism will
change to match later.

## Goals

- One design language, written down and enforced in code: type roles, colour roles, spacing,
  radii, control sizes, layout skeletons, motion.
- Keysmith rebuilt on it, on every screen and dialog.
- The system is self-contained so it can be copied into Prism as is, and later extracted into a
  shared library.
- The app is a working tool that feels pleasant to use: quick, responsive, quiet.

## Non-goals

- Changing Prism now. That is a later project that reuses this module and document.
- Extracting a shared crate or repository now. George decided: keep the system inside each app's
  repo for now, extract later.
- A new colour palette or typeface. The system uses rune-kit's neutral palette and the app's
  fonts; it assigns roles to them.
- New features in Keysmith. Behaviour stays as it is, except where a layout rule requires a small
  change (an overflow menu for inspector actions).

## Anti-goals

- A system that exists only as a document, with screens still built from raw values.
- Decorative motion: anything that makes a frequent action slower.
- Screens that pass the rules but lose information the current UI shows.

## Decisions George made

1. **Shared language across his apps**, Keysmith first, Prism next.
2. **The language may change** from what Prism has today.
3. **Grouped cards (direction C)** for an object's details: a small group label above a card;
   inside, rows with the label on the left and the value on the right, a hairline between rows.
4. **Motion**: a working app that feels pleasant. Short, responsive motion, not none.
5. **Layout**: audit all layouts and adopt one approach everywhere (rules below, accepted).
6. **Home**: inside this repo for now, same in Prism later, extracted afterwards.
7. **rune-kit components are wrapped, not rewritten**, with one exception: the dialog frame is
   the system's own (see "Relation to rune-kit").
8. **In forms the label sits above its input.** Fields are grouped into the same cards as in the
   inspector, paired fields (Address + Port, User + Jump through) stay on one line.

He also confirmed the items in "The language" below as a whole. The exact numbers are Claude's
proposals that he accepted without changing; they can be tuned during the build if a screen
shows they are wrong, and any change is made in the token module, never at a call site.

## How it is made

- A module `src/ds/` in Keysmith. It depends only on `rune` and `rune-kit`, never on `crate::app`
  or any Keysmith type, so the folder can be copied into Prism unchanged. (Claude's call.)
- `DESIGN.md` at the repo root states the rules for people and agents; `AGENTS.md`/`CLAUDE.md`
  point to it. The rules below are its content.
- Screens in `src/ui/` use only `ds` roles and blocks. After the migration, `src/ui/` contains
  no `TextStyle::new`, no raw `k.ty.*`, and no numeric gap, padding or radius outside the scale.
  A test or lint that greps for these keeps it true. (Claude's call.)
- A headless gallery scene (`--scene ds`) renders every block in both themes, so the system can
  be reviewed on one image. (Claude's call.)

## Relation to rune-kit

`ds` wraps rune-kit's controls (Button, IconButton, TextInput, Select, Checkbox, Sidebar,
Toolbar, Toast, menus) and uses the nearest value the kit offers. Where the kit cannot match a
rule, the difference is a recorded exception in `DESIGN.md`, not a reimplementation:

- Control height 32 does not exist in the kit (16/20/24/28/36). Toolbar and dialog footer
  buttons use the kit's nearest size; the builder picks 28 or 36 once and uses it everywhere.
- Sidebar rows, the toolbar title and toasts keep the kit's own typography.

The dialog frame is the one block `ds` builds itself on raw Rune, because the kit's dialog
contradicts the rules in ways that show on every dialog: `ds` needs radius 12, a fixed top
position, a 15 semibold title, and a footer. It must keep what the kit's dialog provides: focus
trap, Escape and outside-press to close, focus return, the close button, accessibility roles.

The lint covers `src/ui/` only: screens may not use raw values. `ds` itself is where raw values
and kit calls live.

## The language

### Type roles

One ladder. A screen picks a role, never a size.

| Role | Style | Colour | Used for |
| --- | --- | --- | --- |
| Title | 15, semibold, line 20 | base | The object's name in the inspector, dialog titles |
| Value | 13, regular, line 20 | base | Values, row names (medium weight for names in lists) |
| Code | 12 mono, line 18 | base | Paths, commands, option names and values, fingerprints |
| Label | 11, regular, line 16 | muted | Field labels, group labels, list meta lines |
| Hint | 11, regular, line 16 | faint | Secondary notes: "default", file locations, help under a field |

A label is always quieter than its value. Bold section headings and dividers are gone: group
labels and the cards carry that structure. rune-kit's 14pt and older 12/13 styles are not used.

### Colour roles

Text uses base, muted and faint only. Status colours (success, warning, danger, info) are for
dots and icons, never for text or fills, with one exception: a destructive button's label.
Status is always a 7pt dot plus a word; colour is never the only signal.

Surfaces: pane background; card (one step above the pane, hairline border); selected row;
hover. Each is one named role mapped to a rune-kit palette entry.

### Spacing, radii, control sizes

- Everything is a multiple of 4. Spacing scale: **4, 8, 12, 16, 24**.
- Radii are nested: **4** tag, **6** control and list row, **8** card, **12** dialog.
- Control heights by place: **24** in the inspector, **28** in lists and filters, **32** in the
  toolbar and dialog footers.

### Blocks

- **Card**: label above (optional), bordered surface, rows inside separated by hairlines.
  - *Pair row*: label left, value right, one line.
  - *Stacked row*: label on top, value across the full width, wraps. For long values: commands,
    paths, public keys, fingerprints.
  - *Item row*: a name with a hint under it, a trailing status or button. For keys of a host,
    hosts of a key, files.
  - *Field row* (in forms): the label above an input, both inside the card. Two fields may share
    a line. Compound fields (key path + key picker, passphrase + repeat + show, the option
    name/value editor, a checkbox with its description) are full-width rows.
- **Inspector head**: title, one meta line, optional status.
- **Action row**: one row, at most three buttons, the rest in a "…" menu, the destructive action
  on the right.
- **List row**: icon, name, one meta line, trailing tags. Heights 32 (one line) and 44 (two).
- **Tag** and **Status** (dot + word).
- **Status line**: the slot between filter and list. It holds the count (one Label line) and,
  under it, any number of notices stacked. A notice is a dot, a line of text that may wrap, and
  an optional trailing button. (Claude's call.)
- **Dialog frame**: title, body of cards, footer with Cancel and one primary button.
- **Empty state**: says what is missing and what to do.

### Layout rules

1. **One content edge per pane**, 16 from the pane's border. Filter, status line, row text and
   cards all sit on it. A row's hover and selection background extends past it; its text does not.
2. **One page skeleton**: toolbar → filter → status line → list. Counts and notices (the
   "ssh-agent isn't running" banner) live in the status line.
3. **One inspector skeleton**: head → action row → cards. Everything below the action row is
   inside a card.
4. **One list row anatomy** on every page (hosts and keys alike).
5. **Two dialog widths**: 440 for confirmations, 560 for forms. A dialog's top sits at a fixed
   height, so dialogs of different heights appear in the same place.
6. **One primary (contrast) button per view.**
7. **Long text**: names truncate at the end, paths in the middle, values in a stacked row wrap.
8. **Pane widths**: the sidebar is fixed, the inspector is 320–400, the list takes the rest. In a
   narrow window the inspector collapses first.

### Motion

- Press is felt: buttons and rows respond under the pointer.
- The list selection moves to the new row quickly instead of jumping.
- Menus and dialogs grow from where they were invoked in 150–200 ms and leave faster than they
  arrive. Ease-out on entry.
- Inspector content changes with a short transition when the selection changes, without a flash.
- Copy confirms on the button itself: the icon becomes a check for about a second.
- Keyboard-driven actions done many times a day (⌘N, arrows through the list) are instant.
- With the system's reduce-motion setting on, only opacity changes remain.
- Durations and curves are named tokens in `ds`, like every other value.

## What the rebuild changes in Keysmith

- **Hosts inspector**: head; action row (Edit, Duplicate, "…" with Move up, Move down; Remove on
  the right); a card with Copy / Open in Terminal / Test connection and the test's result; then
  cards:
  - "Connection": only what the section itself sets among Address, User, Port, Jump through.
    Omitted when it sets none.
  - "Keys": the section's IdentityFile entries, as item rows.
  - "Options": every other option the section sets.
  - "Inherited and defaults": what `ssh -G` resolves that the section does not set, exactly the
    subset shown today, with the "All options" toggle expanding the card in place to the full
    list. Its loading and error states stay as today; it is absent for patterns and Match
    blocks. No "default" hints elsewhere: `ssh -G` cannot tell a built-in default from a value
    inherited from `Host *`. (Claude's call; keeps today's data flow.)
- **Keys inspector**: head; action row with Add to agent / Remove from agent, Set or Change
  passphrase, Edit comment visible, and Write .pub, Show in Finder in "…"; Move to Trash on the
  right (Claude's call). The loose-permissions warning with its Fix button is a notice directly
  under the action row. Cards "Public key" (stacked, with Copy), "Details" (Fingerprint,
  Comment), "Files", "Used by".
- **Hosts and Keys pages**: same skeleton and row anatomy. On Keys the agent bar becomes a
  notice in the status line in both states (running and down), keeping its detail text and the
  "Remove all" button. Config and key read errors are notices too, one each.
- **Dialogs**: Add/Edit host, New key, Passphrase and confirmations use the dialog frame, the two
  widths, and cards with field rows.
- **Sidebar**: same type roles and spacing scale.

No information shown today is dropped.

## Left to the builder

- The module's internal API (function names, builders) and how roles map onto rune-kit's
  `Kit` fields.
- Inspector auto-collapse in a narrow window: rune-kit's SplitView does not do it, so it is
  app-side logic.
- How each motion rule is realised in Rune. Where Rune cannot do one (for example a sliding
  selection), say so and leave it out rather than fake it; report the gap.
- Exact values where the scale leaves a choice (which of 4/8/12 between two elements), kept
  consistent per block.
- The order of migration, as long as tests and clippy stay green at each step.
- Updating the headless UI tests whose element keys or structure change.

## Done when

- Every screen and dialog is built only from `ds`; the lint for raw values passes.
- `DESIGN.md` exists and matches the code.
- The gallery scene renders all blocks in light and dark.
- Existing tests pass (updated where structure changed), clippy is clean.
- Snapshots of every scene show: one content edge per pane, cards in the inspector, two dialog
  widths, one row anatomy.
