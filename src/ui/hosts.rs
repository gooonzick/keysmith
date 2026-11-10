//! The hosts page: the list of `Host` and `Match` sections, and the inspector with one host's
//! settings, its keys, what ssh makes of it, and what can be done to it.

use std::hash::{DefaultHasher, Hash, Hasher};

use rune::{El, Tx, col, row};
use rune_kit::shell::Toolbar;
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit};

use super::{PLUS, PULSE, REFRESH, SERVER, TERMINAL, host_key, plural};
use crate::app::{App, HostRef};
use crate::ds::{self, Card, Inspector, ListRow, MenuAction, Notice, Place, Tone, item_row, pair_row_with, plain_row, stacked_row_with, text};
use crate::paths::Paths;
use crate::sshconfig::{Kind, Section, canonical};

pub fn page(s: &App, k: &Kit, layer: &mut Vec<El>, reduced: bool) -> (El, Toolbar, El) {
    (main(s, k, reduced), toolbar(s, k), inspector(s, k, layer))
}

fn toolbar(s: &App, k: &Kit) -> Toolbar {
    let title = match &s.file_filter {
        Some(p) => s.paths.tilde(p),
        None => "Hosts".to_string(),
    };
    Toolbar::new().title(title).trailing(
        row()
            .gap(6.0)
            .center_y()
            .child(
                IconButton::new(&REFRESH)
                    .variant(IconButtonVariant::GhostMuted)
                    .label("Reload")
                    .help("Reload the config, keys and agent")
                    .shortcut("cmd+r")
                    .on_press(|s: &mut App| {
                        s.rescan();
                        s.refresh_effective();
                    })
                    .build(k)
                    .key("rescan"),
            )
            .child(
                Button::new("Add host")
                    .icon(&PLUS)
                    .variant(ButtonVariant::Contrast)
                    .control_size(ControlSize::Regular)
                    .help("Add a Host section to your SSH config")
                    .shortcut("cmd+n")
                    .on_press(|s: &mut App| s.open_add_host())
                    .build(k)
                    .key("add-host"),
            ),
    )
}

fn main(s: &App, k: &Kit, reduced: bool) -> El {
    let filter = ds::filter(&s.filter, "Filter hosts by name, address, user or key").label("Filter hosts").on_edit(|s: &mut App| &mut s.filter).build(k).key("filter");
    let visible = s.visible_hosts();
    let files = if s.file_filter.is_some() { "this file".to_string() } else { plural(s.docs.len().max(1), "file") };
    let mut page = ds::Page::new("hosts-page").filter(filter).count(format!("{} in {files}", plural(visible.len(), "host")));
    for (i, (file, e)) in s.config_errors.iter().enumerate() {
        page = page.notice(Notice::new(Tone::Warn, format!("Couldn't read {}", s.paths.tilde(file))).detail(e.clone()).key(format!("config-error-{i}")).build(k));
    }
    if s.host_count() == 0 {
        page = page.empty(ds::empty(k, "No hosts yet", format!("Add one, and ssh {} will know where to go.", "<name>")));
    } else if visible.is_empty() {
        page = page.empty(ds::empty(k, "Nothing to show", "Try another word."));
    }
    page.rows(visible.iter().map(|&id| host_row(s, id, k, reduced))).build(k)
}

/// `user@hostname:port`, as much of it as the section sets.
fn target(sec: &Section) -> String {
    let host = sec.get("HostName").unwrap_or_default();
    let mut t = String::new();
    if let Some(u) = sec.get("User") {
        t.push_str(&u);
        t.push('@');
    }
    t.push_str(if host.is_empty() && !t.is_empty() { "…" } else { &host });
    if let Some(p) = sec.get("Port") {
        t.push(':');
        t.push_str(&p);
    }
    if t.is_empty() {
        // No address: name the options instead, but not the keys, which the tags already show.
        t = sec.options.iter().filter(|o| !o.key.eq_ignore_ascii_case("IdentityFile")).take(3).map(|o| format!("{} {}", o.key, o.value)).collect::<Vec<_>>().join(" · ");
    }
    t
}

fn host_row(s: &App, id: (usize, usize), k: &Kit, reduced: bool) -> El {
    let sec = s.section(id);
    let mut tags = Vec::new();
    match sec.kind {
        Kind::Match => tags.push(ds::tag(k, "match", None, "A Match block: applies when its criteria hold")),
        Kind::Host if sec.is_pattern() => tags.push(ds::tag(k, "pattern", None, "Sets defaults for every host it matches")),
        Kind::Host => {}
    }
    for v in sec.all("IdentityFile") {
        let (label, tone, help) = match s.key_at(&v) {
            Some(key) if s.in_agent(key) => (key.name.clone(), Some(Tone::Ok), format!("{v} (in the agent)")),
            Some(key) => (key.name.clone(), None, v.clone()),
            None if s.paths.expand(&v).exists() => (short_path(&s.paths, &v), None, v.clone()),
            None => (short_path(&s.paths, &v), Some(Tone::Bad), format!("{v} isn't there")),
        };
        tags.push(ds::tag(k, label, tone, help));
    }
    if id.0 > 0 {
        let name = s.docs[id.0].path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        tags.push(ds::tag(k, name, None, format!("In {}", s.paths.tilde(&s.docs[id.0].path))));
    }
    // The description stays in the inspector: here it would crowd the name.
    let mut r = ListRow::new(host_key(id), sec.patterns.clone()).icon(&SERVER).tags(tags).selected(s.selected_host_id() == Some(id)).dim(sec.is_pattern()).reduced(reduced);
    let target = target(sec);
    if !target.is_empty() {
        r = r.meta(target);
    }
    r.build(k, move |s: &mut App| s.select_host(id))
}

/// A key file's name in a tag: just the name in ~/.ssh, the folder too anywhere else
/// (`_config/user` says more than `user`).
fn short_path(paths: &Paths, v: &str) -> String {
    let path = paths.expand(v);
    let name = path.file_name().map_or_else(|| v.to_string(), |n| n.to_string_lossy().into_owned());
    match path.parent() {
        Some(dir) if dir != paths.ssh => match dir.file_name() {
            Some(d) => format!("{}/{name}", d.to_string_lossy()),
            None => name,
        },
        _ => name,
    }
}

/// The inspector's subject: stays the same while the host moves within its file, so a move
/// doesn't fade the content.
fn subject(r: &HostRef) -> String {
    let mut h = DefaultHasher::new();
    (&r.path, &r.patterns, r.nth).hash(&mut h);
    format!("host-{:016x}", h.finish())
}

fn inspector(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let (Some(id), Some(r)) = (s.selected_host_id(), s.selected_host.as_ref()) else {
        return col().key("inspector").w_full().padding(ds::size::PANE_EDGE).child(ds::empty(k, "No host selected", "Select a host to see its settings and keys."));
    };
    let sec = s.section(id);
    let doc = &s.docs[id.0];
    let kind = match sec.kind {
        Kind::Match => "Match",
        Kind::Host if sec.is_pattern() => "Host pattern",
        Kind::Host => "Host",
    };
    let mut ins = Inspector::new().subject(subject(r)).title(sec.patterns.clone()).meta(format!("{kind} · {}, line {}", s.paths.tilde(&doc.path), sec.header + 1));
    if !sec.doc.is_empty() {
        ins = ins.description(sec.doc.join("\n"));
    }
    let (left, right) = actions(s, id, k, layer);
    ins.actions(left, Some(right))
        .card(sec.alias().and_then(|alias| connect(s, alias, k)))
        .card(connection(sec, k))
        .card(identities(s, sec, k))
        .card(options(sec, k))
        .card(effective(s, sec, k))
        .build(k)
}

/// Edit, Duplicate and a "…" menu with Move up and Move down; Remove on the right.
fn actions(s: &App, id: (usize, usize), k: &Kit, layer: &mut Vec<El>) -> (Vec<El>, El) {
    let count = s.docs[id.0].sections.len();
    let more = ds::overflow::<App>(
        k,
        "host-more",
        Place::Inspector,
        s.host_menu,
        |s: &mut App| s.host_menu = !s.host_menu,
        |s: &mut App| s.host_menu = false,
        vec![
            MenuAction::new("Move up", move |s: &mut App| s.move_host(id, -1)).description("ssh takes the first value it finds").disabled(id.1 == 0),
            MenuAction::new("Move down", move |s: &mut App| s.move_host(id, 1)).disabled(id.1 + 1 >= count),
        ],
        layer,
    );
    let left = vec![
        ds::button("Edit…", Place::Inspector).help("Change its settings (Enter)").on_press(move |s: &mut App| s.open_edit_host(id)).build(k).key("host-edit"),
        ds::button("Duplicate…", Place::Inspector).help("A new host with the same settings").on_press(move |s: &mut App| s.open_duplicate_host(id)).build(k).key("host-duplicate"),
        more,
    ];
    let remove = ds::danger("Remove…", Place::Inspector).on_press(move |s: &mut App| s.confirm_remove_host(id)).build(k).key("host-remove");
    (left, remove)
}

/// Copy the command, open it in Terminal, test the connection; then what the test said.
fn connect(s: &App, alias: &str, k: &Kit) -> Option<El> {
    let command = format!("ssh {alias}");
    let (a1, a2) = (alias.to_string(), alias.to_string());
    let testing = s.testing.as_deref() == Some(alias);
    let mut buttons = row().gap(ds::space::S2).center_y().flex_wrap().child(
        ds::copy_button("Copy", Place::Inspector, s.just_copied(&command))
            .help(format!("Copy “{command}”"))
            .on_press_tx(move |s: &mut App, tx: &mut Tx| {
                tx.copy(command.clone());
                s.copied(command.clone());
            })
            .build(k)
            .key("host-copy"),
    );
    if cfg!(target_os = "macos") {
        buttons = buttons.child(ds::button("Open in Terminal", Place::Inspector).icon(&TERMINAL).on_press(move |s: &mut App| s.open_in_terminal(&a1)).build(k).key("host-terminal"));
    }
    buttons = buttons.child(
        ds::button(if testing { "Testing…" } else { "Test connection" }, Place::Inspector)
            .icon(&PULSE)
            .help("Connects without prompts (BatchMode) and logs straight out")
            .disabled(s.testing.is_some())
            .on_press(move |s: &mut App| s.test_connection(a2.clone()))
            .build(k)
            .key("host-test"),
    );
    let mut card = Card::new("card-connect").row(plain_row(buttons));
    if let Some((who, r)) = &s.tested
        && who == alias
    {
        let (tone, said) = match r {
            Ok(m) => (Tone::Ok, m),
            Err(e) => (Tone::Bad, e),
        };
        card = card.row(Notice::new(tone, said.clone()).key("host-test-result").build(k));
    }
    card.build(k)
}

/// Address, user, port and jump host: only those the section sets.
fn connection(sec: &Section, k: &Kit) -> Option<El> {
    let rows = [("Address", "HostName"), ("User", "User"), ("Port", "Port"), ("Jump through", "ProxyJump")]
        .into_iter()
        .filter_map(|(label, key)| sec.get(key).map(|v| value_row(text::label(k, label), text::code(k, v.clone()).help(v.clone()).key(format!("value-{}", key.to_lowercase())), &v)));
    Card::new("card-connection").label("Connection").rows(rows).build(k)
}

/// The section's `IdentityFile`s.
fn identities(s: &App, sec: &Section, k: &Kit) -> Option<El> {
    let rows = sec.all("IdentityFile").into_iter().enumerate().map(|(i, v)| identity(s, &v, i, k));
    Card::new("card-keys").label("Keys").rows(rows).build(k)
}

/// One `IdentityFile`: the key it is, whether it's there and loaded.
fn identity(s: &App, value: &str, i: usize, k: &Kit) -> El {
    let key = s.key_at(value);
    let (hint, status) = match key {
        Some(key) if s.in_agent(key) => (Some(key.kind.clone()), Some((Tone::Ok, "In the agent"))),
        Some(key) => (Some(format!("{}{}", key.kind, if key.encrypted == Some(true) { " · passphrase" } else { "" })), None),
        None if s.paths.expand(value).exists() => (Some("not a key Keysmith can read".to_string()), None),
        None => (None, Some((Tone::Bad, "Missing"))),
    };
    let mut trailing = row().gap(ds::space::S2).center_y();
    if let Some((tone, word)) = status {
        trailing = trailing.child(ds::status(k, tone, word));
    }
    if let Some(key) = key {
        let name = key.name.clone();
        trailing = trailing.child(ds::quiet("Show key", Place::Inspector).on_press(move |s: &mut App| s.show_key(&name)).build(k).key(format!("identity-{i}-show")));
    }
    item_row(k, text::path(k, value.to_string()).help(value.to_string()), hint.map(|h| text::hint(k, h)), Some(trailing)).key(format!("identity-{i}"))
}

/// Every other option the section sets.
fn options(sec: &Section, k: &Kit) -> Option<El> {
    const MAIN: [&str; 5] = ["hostname", "user", "port", "proxyjump", "identityfile"];
    let card = Card::new("card-options").label("Options");
    if sec.options.is_empty() {
        return card.row(text::hint(k, "No options yet.")).build(k);
    }
    card.rows(sec.options.iter().filter(|o| !MAIN.contains(&o.key.to_lowercase().as_str())).map(|o| option_row(k, &o.key, o.value.clone()))).build(k)
}

/// An option in the code face.
fn option_row(k: &Kit, key: &str, value: String) -> El {
    let label = text::code(k, key.to_string()).truncate().help(key.to_string());
    value_row(label, text::code(k, value.clone()), &value)
}

/// A label and a value on one line; a value (`text`) too long for the right-hand column goes
/// under the label at full width, where it wraps.
fn value_row(label: El, value: El, text: &str) -> El {
    const LONG: usize = 28;
    if text.chars().count() > LONG { stacked_row_with(label, value) } else { pair_row_with(label, value) }
}

/// What `ssh -G` makes of the host. By default only what the section doesn't set itself (what it
/// inherits or ssh defaults to), since the rest is shown above; everything on request.
fn effective(s: &App, sec: &Section, k: &Kit) -> Option<El> {
    let e = s.effective_for.as_ref()?;
    let all = s.show_all_effective;
    let toggle = ds::quiet(if all { "Fewer" } else { "All options" }, Place::Inspector)
        .on_press(|s: &mut App| s.show_all_effective = !s.show_all_effective)
        .build(k)
        .key("effective-toggle");
    let card = Card::new("card-inherited").label(if all { "Resolved by ssh" } else { "Inherited and defaults" }).trailing(toggle);
    let card = match &e.result {
        None => card.row(text::hint(k, "Asking ssh…")),
        Some(Err(err)) => card.row(Notice::new(Tone::Bad, err.clone()).build(k)),
        Some(Ok(opts)) => {
            const KEY_OPTS: [&str; 9] = ["hostname", "user", "port", "identityfile", "proxyjump", "proxycommand", "identitiesonly", "forwardagent", "addkeystoagent"];
            let set = |key: &str| sec.options.iter().any(|o| o.key.eq_ignore_ascii_case(key));
            let rows: Vec<El> = opts
                .iter()
                .filter(|(key, _)| all || (KEY_OPTS.contains(&key.as_str()) && !set(key)))
                .map(|(key, v)| {
                    let value = if key == "identityfile" { s.paths.tilde(std::path::Path::new(v)) } else { v.clone() };
                    option_row(k, canonical(key), value)
                })
                .collect();
            if rows.is_empty() { card.row(text::hint(k, "Nothing beyond what's set above.")) } else { card.rows(rows) }
        }
    };
    card.build(k)
}
