//! The hosts page: the list of `Host` and `Match` sections, and the inspector with one host's
//! settings, its keys, what ssh makes of it, and what can be done to it.

use rune::{El, Tx, col, row, text};
use rune_kit::shell::Toolbar;
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, TextInput, glyphs};

use super::{DOWN, PLUS, PULSE, REFRESH, TERMINAL, UP, dot, empty, field, heading, host_key, inspector_body, kv_row, mono, notice_line, plural, scroll_pane, tag};
use crate::app::App;
use crate::ds::{self, Place};
use crate::paths::Paths;
use crate::sshconfig::{Kind, Section, canonical};

pub fn page(s: &App, k: &Kit, _layer: &mut Vec<El>) -> (El, Toolbar, El) {
    (main(s, k), toolbar(s, k), inspector(s, k))
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

fn main(s: &App, k: &Kit) -> El {
    let mut filter = TextInput::new(&s.filter).placeholder("Filter hosts by name, address, user or key").leading_icon(&glyphs::SEARCH);
    if !s.filter.text.is_empty() {
        filter = filter.clear_button();
    }
    let filter = filter
        .label("Filter hosts")
        .width(None)
        .on_edit(|s: &mut App| &mut s.filter)
        .build(k)
        .key("filter");
    let visible = s.visible_hosts();
    let mut list = col().key("hosts").w_full().gap(2.0);
    if s.host_count() == 0 {
        list = list.child(empty("No hosts yet", &format!("Add one, and ssh {} will know where to go.", "<name>"), k));
    } else if visible.is_empty() {
        list = list.child(empty("Nothing to show", "Try another word.", k));
    }
    for &id in &visible {
        list = list.child(host_row(s, id, k));
    }
    let mut page = col().key("hosts-page").w_full().h_full().min_h(0.0).padding(16.0).gap(10.0).child(filter);
    if !s.config_errors.is_empty() {
        let mut errors = col().key("config-errors").w_full().px(12.0).gap(4.0);
        for (file, e) in &s.config_errors {
            errors = errors.child(notice_line(k.c.warning.icon, format!("Couldn't read {}: {e}", s.paths.tilde(file)), k));
        }
        page = page.child(errors);
    }
    let summary = format!("{} in {}", plural(visible.len(), "host"), if s.file_filter.is_some() { "this file".to_string() } else { plural(s.docs.len().max(1), "file") });
    let header = row().w_full().h(28.0).px(12.0).center_y().child(text(summary).font(k.ty.small).color(k.c.text_faint).truncate());
    page.child(header).child(scroll_pane("hosts-scroll", list).grow(1.0))
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

fn host_row(s: &App, id: (usize, usize), k: &Kit) -> El {
    let sec = s.section(id);
    let selected = s.selected_host_id() == Some(id);
    let mut tags = row().key("tags").gap(4.0).center_y();
    match sec.kind {
        Kind::Match => tags = tags.child(tag("match", None, "A Match block: applies when its criteria hold", k)),
        Kind::Host if sec.is_pattern() => tags = tags.child(tag("pattern", None, "Sets defaults for every host it matches", k)),
        Kind::Host => {}
    }
    for v in sec.all("IdentityFile") {
        let (label, color, help) = match s.key_at(&v) {
            Some(key) if s.in_agent(key) => (key.name.clone(), Some(k.c.success.icon), format!("{v} (in the agent)")),
            Some(key) => (key.name.clone(), None, v.clone()),
            None if s.paths.expand(&v).exists() => (short_path(&s.paths, &v), None, v.clone()),
            None => (short_path(&s.paths, &v), Some(k.c.danger.icon), format!("{v} isn't there")),
        };
        tags = tags.child(tag(&label, color, &help, k));
    }
    if id.0 > 0 {
        let name = s.docs[id.0].path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        tags = tags.child(tag(&name, None, &format!("In {}", s.paths.tilde(&s.docs[id.0].path)), k));
    }
    // The description stays in the inspector: here it would crowd the name.
    let mut info = col().gap(2.0).grow(1.0).min_w(120.0).child(text(sec.patterns.clone()).font(k.ty.ui_medium).color(k.c.text_base).truncate());
    let target = target(sec);
    if !target.is_empty() {
        info = info.child(text(target).font(k.ty.small).color(k.c.text_muted).truncate());
    }
    let r = row()
        .key(host_key(id))
        .role("button")
        .label(sec.patterns.clone())
        .focusable(true)
        .w_full()
        .px(12.0)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .radius(k.radius.md)
        .on_press(move |s: &mut App| s.select_host(id))
        .child(info)
        .child(tags);
    let r = if selected { r.fill(k.c.layer_02) } else { r.hover(k.c.hover) };
    if sec.is_pattern() { r.dim(0.75) } else { r }
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

fn small(label: &str) -> Button {
    Button::new(label).variant(ButtonVariant::Outline).control_size(ControlSize::Small)
}

fn inspector(s: &App, k: &Kit) -> El {
    let Some(id) = s.selected_host_id() else {
        return inspector_body().child(empty("No host selected", "Click a host to see its settings and keys.", k));
    };
    let sec = s.section(id);
    let doc = &s.docs[id.0];
    let mut body = inspector_body();
    let kind = match sec.kind {
        Kind::Match => "Match",
        Kind::Host if sec.is_pattern() => "Host pattern",
        Kind::Host => "Host",
    };
    body = body.child(
        col()
            .w_full()
            .gap(4.0)
            .child(text(sec.patterns.clone()).font(k.ty.body_medium).color(k.c.text_base).wrap_anywhere())
            .child(text(format!("{kind} · {}, line {}", s.paths.tilde(&doc.path), sec.header + 1)).font(k.ty.small).color(k.c.text_faint).wrap_anywhere()),
    );
    if !sec.doc.is_empty() {
        body = body.child(text(sec.doc.join("\n")).font(k.ty.small).color(k.c.text_muted).wrap());
    }
    body = body.child(actions(s, id, sec, k));
    if let Some(alias) = sec.alias() {
        body = body.child(connect(s, alias, k));
    }

    let mut values = col().key("host-values").w_full().gap(12.0);
    let plain = |key: &str| sec.get(key);
    for (label, key) in [("Address", "HostName"), ("User", "User"), ("Port", "Port"), ("Jump through", "ProxyJump")] {
        if let Some(v) = plain(key) {
            values = values.child(field(label, mono(v, k).key(format!("value-{}", key.to_lowercase())), k));
        }
    }
    let ids = sec.all("IdentityFile");
    if !ids.is_empty() {
        let mut list = col().w_full().gap(6.0);
        for (i, v) in ids.iter().enumerate() {
            list = list.child(identity(s, v, i, k));
        }
        values = values.child(field("Keys", list, k));
    }
    let main = ["hostname", "user", "port", "proxyjump", "identityfile"];
    let others: Vec<_> = sec.options.iter().filter(|o| !main.contains(&o.key.to_lowercase().as_str())).collect();
    if !others.is_empty() {
        let mut list = col().w_full().gap(3.0);
        for o in others {
            list = list.child(kv_row(&o.key, o.value.clone(), k));
        }
        values = values.child(field("Other options", list, k));
    }
    if sec.options.is_empty() {
        values = values.child(text("No options yet.").font(k.ty.small).color(k.c.text_faint));
    }
    body = body.child(values);
    if let Some(e) = effective(s, sec, k) {
        body = body.child(e);
    }
    body
}

fn actions(s: &App, id: (usize, usize), sec: &Section, k: &Kit) -> El {
    let count = s.docs[id.0].sections.len();
    let mut left = row().gap(6.0).center_y().flex_wrap();
    left = left.child(small("Edit…").help("Change its settings (Enter)").on_press(move |s: &mut App| s.open_edit_host(id)).build(k).key("host-edit"));
    left = left.child(small("Duplicate…").help("A new host with the same settings").on_press(move |s: &mut App| s.open_duplicate_host(id)).build(k).key("host-duplicate"));
    left = left.child(
        IconButton::new(&UP)
            .variant(IconButtonVariant::GhostMuted)
            .label("Move up")
            .help("Move up: ssh takes the first value it finds")
            .disabled(id.1 == 0)
            .on_press(move |s: &mut App| s.move_host(id, -1))
            .build(k)
            .key("host-up"),
    );
    left = left.child(
        IconButton::new(&DOWN)
            .variant(IconButtonVariant::GhostMuted)
            .label("Move down")
            .help("Move down")
            .disabled(id.1 + 1 >= count)
            .on_press(move |s: &mut App| s.move_host(id, 1))
            .build(k)
            .key("host-down"),
    );
    let _ = sec;
    row()
        .key("host-actions")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(left)
        .child(row().grow(1.0))
        .child(Button::new("Remove…").variant(ButtonVariant::Danger).control_size(ControlSize::Small).on_press(move |s: &mut App| s.confirm_remove_host(id)).build(k).key("host-remove"))
}

/// Copy the command, open it in Terminal, test the connection.
fn connect(s: &App, alias: &str, k: &Kit) -> El {
    let command = format!("ssh {alias}");
    let (a1, a2) = (alias.to_string(), alias.to_string());
    let testing = s.testing.as_deref() == Some(alias);
    let mut buttons = row().gap(6.0).center_y().flex_wrap().child(
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
        buttons = buttons.child(small("Open in Terminal").icon(&TERMINAL).on_press(move |s: &mut App| s.open_in_terminal(&a1)).build(k).key("host-terminal"));
    }
    buttons = buttons.child(
        small(if testing { "Testing…" } else { "Test connection" })
            .icon(&PULSE)
            .help("Connects without prompts (BatchMode) and logs straight out")
            .disabled(s.testing.is_some())
            .on_press(move |s: &mut App| s.test_connection(a2.clone()))
            .build(k)
            .key("host-test"),
    );
    let mut c = col().key("host-connect").w_full().gap(8.0).child(buttons);
    if let Some((who, r)) = &s.tested
        && who == alias
    {
        c = c.child(match r {
            Ok(m) => notice_line(k.c.success.icon, m.clone(), k),
            Err(e) => notice_line(k.c.danger.icon, e.clone(), k),
        });
    }
    c
}

/// One `IdentityFile`: the key it is, whether it's there and loaded.
fn identity(s: &App, value: &str, i: usize, k: &Kit) -> El {
    let path = s.paths.expand(value);
    let (color, status) = match s.key_at(value) {
        Some(key) if s.in_agent(key) => (k.c.success.icon, format!("{} · in the agent", key.kind)),
        Some(key) => (k.c.text_faint, format!("{}{}", key.kind, if key.encrypted == Some(true) { " · passphrase" } else { "" })),
        None if path.exists() => (k.c.text_faint, "not a key Keysmith can read".into()),
        None => (k.c.danger.icon, "missing".into()),
    };
    let mut line = row().w_full().gap(8.0).center_y().child(dot(color)).child(
        col().grow(1.0).min_w(0.0).gap(1.0).child(mono(value.to_string(), k)).child(text(status).font(k.ty.small).color(k.c.text_faint)),
    );
    if let Some(key) = s.key_at(value) {
        let name = key.name.clone();
        line = line.child(Button::new("Show key").variant(ButtonVariant::Ghost).control_size(ControlSize::Small).on_press(move |s: &mut App| s.show_key(&name)).build(k).key(format!("identity-{i}-show")));
    }
    line.key(format!("identity-{i}"))
}

/// What `ssh -G` makes of the host. By default only what the section doesn't set itself (what it
/// inherits or ssh defaults to), since the rest is shown above; everything on request.
fn effective(s: &App, sec: &Section, k: &Kit) -> Option<El> {
    let e = s.effective_for.as_ref()?;
    let mut c = col().key("effective").w_full().gap(3.0);
    let all = s.show_all_effective;
    let toggle = Button::new(if all { "Fewer" } else { "All options" })
        .variant(ButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .on_press(|s: &mut App| s.show_all_effective = !s.show_all_effective)
        .build(k)
        .key("effective-toggle");
    let title = if all { "Resolved by ssh" } else { "Inherited and defaults" };
    c = c.child(row().w_full().center_y().child(heading(title, k).grow(1.0)).child(toggle));
    match &e.result {
        None => c = c.child(text("Asking ssh…").font(k.ty.small).color(k.c.text_faint)),
        Some(Err(err)) => c = c.child(notice_line(k.c.danger.icon, err.clone(), k)),
        Some(Ok(opts)) => {
            const KEY_OPTS: [&str; 9] = ["hostname", "user", "port", "identityfile", "proxyjump", "proxycommand", "identitiesonly", "forwardagent", "addkeystoagent"];
            let set = |key: &str| sec.options.iter().any(|o| o.key.eq_ignore_ascii_case(key));
            let mut shown = 0;
            for (key, v) in opts.iter().filter(|(key, _)| all || (KEY_OPTS.contains(&key.as_str()) && !set(key))) {
                let value = if key == "identityfile" { s.paths.tilde(std::path::Path::new(v)) } else { v.clone() };
                c = c.child(kv_row(canonical(key), value, k));
                shown += 1;
            }
            if shown == 0 {
                c = c.child(text("Nothing beyond what's set above.").font(k.ty.small).color(k.c.text_faint));
            }
        }
    }
    Some(c)
}
