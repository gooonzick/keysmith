//! The keys page: the keys in `~/.ssh` with the agent above them, and the inspector with one
//! key's fingerprint, public half, the hosts that use it and what can be done to it.

use rune::{El, Tx, col, row, text};
use rune_kit::shell::Toolbar;
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, TextInput, glyphs};

use super::{IMPORT, PLUS, REFRESH, dot, empty, field, heading, inspector_body, mono, notice_line, plural, scroll_pane, tag};
use crate::app::{App, PassPurpose};
use crate::keys::{Agent, Key};

pub fn page(s: &App, k: &Kit, _layer: &mut Vec<El>) -> (El, Toolbar, El) {
    (main(s, k), toolbar(k), inspector(s, k))
}

fn toolbar(k: &Kit) -> Toolbar {
    Toolbar::new().title("Keys").trailing(
        row()
            .gap(6.0)
            .center_y()
            .child(
                IconButton::new(&REFRESH)
                    .variant(IconButtonVariant::GhostMuted)
                    .label("Reload")
                    .help("Reload the keys and the agent")
                    .shortcut("cmd+r")
                    .on_press(|s: &mut App| s.rescan())
                    .build(k)
                    .key("rescan"),
            )
            .child(
                IconButton::new(&IMPORT)
                    .variant(IconButtonVariant::GhostMuted)
                    .label("Import a key…")
                    .help("Copy a private key from elsewhere into ~/.ssh")
                    .on_press_tx(|_: &mut App, tx: &mut Tx| {
                        let options = rune::FileDialogOptions { title: "Choose a private key".into(), ..Default::default() };
                        tx.open_file(options, |s: &mut App, result, _| match result {
                            Ok(Some(path)) => s.import_key(path),
                            Ok(None) => {}
                            Err(e) => s.notify("Couldn't open the file picker", e.to_string(), true),
                        });
                    })
                    .build(k)
                    .key("import-key"),
            )
            .child(
                Button::new("New key")
                    .icon(&PLUS)
                    .variant(ButtonVariant::Contrast)
                    .control_size(ControlSize::Regular)
                    .help("Make a key pair with ssh-keygen")
                    .shortcut("cmd+n")
                    .on_press(|s: &mut App| s.open_new_key())
                    .build(k)
                    .key("new-key"),
            ),
    )
}

/// The agent's state, and keys it holds that aren't in `~/.ssh`.
fn agent_bar(s: &App, k: &Kit) -> El {
    let bar = row().key("agent-bar").w_full().px(12.0).py(10.0).gap(10.0).center_y().radius(k.radius.md).fill(k.c.layer_01);
    match &s.agent {
        Agent::Down(why) => bar.child(dot(k.c.warning.icon)).child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text("ssh-agent isn't running").font(k.ty.ui_medium).color(k.c.text_base))
                .child(text(why.clone()).font(k.ty.small).color(k.c.text_faint).wrap()),
        ),
        Agent::Running(loaded) => {
            let foreign = loaded.iter().filter(|(fp, _)| !s.keys.iter().any(|k| k.fingerprint.as_deref() == Some(fp))).count();
            let mut detail = if loaded.is_empty() { "No keys loaded. ssh asks for passphrases every time.".to_string() } else { format!("{} loaded", plural(loaded.len(), "key")) };
            if foreign > 0 {
                detail.push_str(&format!(", {foreign} from outside ~/.ssh"));
            }
            let mut bar = bar.child(dot(k.c.success.icon)).child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(2.0)
                    .child(text("ssh-agent").font(k.ty.ui_medium).color(k.c.text_base))
                    .child(text(detail).font(k.ty.small).color(k.c.text_faint).wrap()),
            );
            if !loaded.is_empty() {
                bar = bar.child(
                    Button::new("Remove all")
                        .variant(ButtonVariant::Outline)
                        .control_size(ControlSize::Small)
                        .help("Unload every key from the agent (ssh-add -D)")
                        .on_press(|s: &mut App| s.clear_agent())
                        .build(k)
                        .key("agent-clear"),
                );
            }
            bar
        }
    }
}

fn main(s: &App, k: &Kit) -> El {
    let mut filter = TextInput::new(&s.filter).placeholder("Filter keys by name, comment or fingerprint").leading_icon(&glyphs::SEARCH);
    if !s.filter.text.is_empty() {
        filter = filter.clear_button();
    }
    let filter = filter
        .label("Filter keys")
        .width(None)
        .on_edit(|s: &mut App| &mut s.filter)
        .build(k)
        .key("filter");
    let visible = s.visible_keys();
    let mut list = col().key("keys").w_full().gap(2.0);
    if s.keys.is_empty() {
        list = list.child(empty("No keys in ~/.ssh", "Make one with New key, or import one you have.", k));
    } else if visible.is_empty() {
        list = list.child(empty("Nothing to show", "Try another word.", k));
    }
    for &i in &visible {
        list = list.child(key_row(s, &s.keys[i], k));
    }
    let mut page = col().key("keys-page").w_full().h_full().min_h(0.0).padding(16.0).gap(10.0).child(filter).child(agent_bar(s, k));
    for e in &s.key_errors {
        page = page.child(notice_line(k.c.warning.icon, e.clone(), k));
    }
    page.child(scroll_pane("keys-scroll", list).grow(1.0))
}

fn key_row(s: &App, key: &Key, k: &Kit) -> El {
    let selected = s.selected_key.as_deref() == Some(key.name.as_str());
    let mut tags = row().key("tags").gap(4.0).center_y();
    if s.in_agent(key) {
        tags = tags.child(tag("in agent", Some(k.c.success.icon), "Loaded in ssh-agent", k));
    }
    match (key.private.is_some(), key.encrypted) {
        (false, _) => tags = tags.child(tag("public only", None, "Only the .pub is here: a hardware key, or a copy", k)),
        (true, Some(true)) => tags = tags.child(tag("passphrase", None, "Protected by a passphrase", k)),
        (true, Some(false)) => tags = tags.child(tag("no passphrase", Some(k.c.warning.icon), "Anyone with the file can use it", k)),
        (true, None) => {}
    }
    if let Some(m) = key.loose_mode {
        tags = tags.child(tag(&format!("{m:o}"), Some(k.c.danger.icon), "Others can read it; ssh refuses such keys", k));
    }
    let used = s.hosts_using(key).len();
    let subtitle = [Some(match key.bits {
        Some(b) if key.kind == "RSA" => format!("RSA {b}"),
        _ => key.kind.clone(),
    }), (!key.comment.is_empty()).then(|| key.comment.clone()), (used > 0).then(|| plural(used, "host"))]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let name = key.name.clone();
    let info = col()
        .gap(2.0)
        .grow(1.0)
        .min_w(120.0)
        .child(text(key.name.clone()).font(k.ty.ui_medium).color(k.c.text_base).truncate())
        .child(text(subtitle).font(k.ty.small).color(k.c.text_muted).truncate());
    let r = row()
        .key(format!("key-{}", key.name))
        .role("button")
        .label(key.name.clone())
        .focusable(true)
        .w_full()
        .px(12.0)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .radius(k.radius.md)
        .on_press(move |s: &mut App| s.select_key(&name))
        .child(rune::icon(&super::KEY, 16.0).color(k.c.icon_muted))
        .child(info)
        .child(tags);
    if selected { r.fill(k.c.layer_02) } else { r.hover(k.c.hover) }
}

fn small(label: &str) -> Button {
    Button::new(label).variant(ButtonVariant::Outline).control_size(ControlSize::Small)
}

fn inspector(s: &App, k: &Kit) -> El {
    let Some(key) = s.selected_key.as_deref().and_then(|n| s.key(n)) else {
        return inspector_body().child(empty("No key selected", "Click a key to see its fingerprint, public key and hosts.", k));
    };
    let mut body = inspector_body().child(
        col()
            .w_full()
            .gap(4.0)
            .child(text(key.name.clone()).font(k.ty.body_medium).color(k.c.text_base).wrap_anywhere())
            .child(text(describe(key)).font(k.ty.small).color(k.c.text_faint).wrap()),
    );
    if let Some(m) = key.loose_mode {
        let name = key.name.clone();
        body = body.child(
            row()
                .key("loose-warning")
                .w_full()
                .gap(8.0)
                .center_y()
                .child(notice_line(k.c.danger.icon, format!("Permissions are {m:o}: others can read this key, and ssh refuses to use it."), k).grow(1.0).min_w(0.0))
                .child(small("Fix").on_press(move |s: &mut App| s.fix_permissions(&name)).build(k).key("key-fix")),
        );
    }
    body = body.child(actions(s, key, k));
    if let Some(text_) = &key.public_text {
        let copy = text_.clone();
        let copy_btn = small("Copy public key").icon(&glyphs::COPY).help("Paste it into GitHub, GitLab or a server's authorized_keys").on_press_tx(move |s: &mut App, tx: &mut Tx| {
            tx.copy(copy.clone());
            s.notify("Copied the public key", "", false);
        });
        body = body.child(
            col()
                .key("public-key")
                .w_full()
                .gap(6.0)
                .child(row().w_full().center_y().child(heading("Public key", k).grow(1.0)).child(copy_btn.build(k).key("key-copy")))
                .child(col().w_full().padding(10.0).radius(k.radius.md).fill(k.c.layer_01).child(mono(text_.clone(), k))),
        );
    }
    let mut facts = col().key("key-facts").w_full().gap(12.0);
    if let Some(fp) = &key.fingerprint {
        facts = facts.child(field("Fingerprint", mono(fp.clone(), k).key("key-fingerprint"), k));
    }
    if !key.comment.is_empty() {
        facts = facts.child(field("Comment", mono(key.comment.clone(), k), k));
    }
    let mut files = col().w_full().gap(2.0);
    for p in [&key.private, &key.public].into_iter().flatten() {
        let mode = crate::fsops::mode(p).map_or_else(String::new, |m| format!("  {m:o}"));
        files = files.child(mono(format!("{}{mode}", s.paths.tilde(p)), k));
    }
    facts = facts.child(field("Files", files, k));
    let hosts = s.hosts_using(key);
    if hosts.is_empty() {
        let none = text("No host names it as its IdentityFile. ssh still offers it to every server unless IdentitiesOnly is set.").font(k.ty.small).color(k.c.text_faint).wrap();
        return body.child(facts.child(field("Used by", col().key("key-hosts").w_full().child(none), k)));
    }
    let mut used = row().key("key-hosts").w_full().gap(4.0).flex_wrap();
    for (i, id) in hosts.into_iter().enumerate() {
        let sec = s.section(id);
        used = used.child(
            Button::new(sec.patterns.clone()).variant(ButtonVariant::Ghost).control_size(ControlSize::Small).on_press(move |s: &mut App| s.show_host(id)).build(k).key(format!("key-host-{i}")),
        );
    }
    facts = facts.child(field("Used by", used, k));
    body.child(facts)
}

fn describe(key: &Key) -> String {
    let mut parts = vec![match key.bits {
        Some(b) => format!("{} · {b} bits", key.kind),
        None => key.kind.clone(),
    }];
    parts.push(
        match (key.private.is_some(), key.encrypted) {
            (false, _) => "public key only",
            (true, Some(true)) => "protected by a passphrase",
            (true, Some(false)) => "no passphrase",
            (true, None) => "passphrase unknown",
        }
        .to_string(),
    );
    parts.join(" · ")
}

fn actions(s: &App, key: &Key, k: &Kit) -> El {
    let n = key.name.clone();
    let mut left = row().gap(6.0).center_y().flex_wrap();
    if key.private.is_some() {
        if s.in_agent(key) {
            let n = n.clone();
            left = left.child(small("Remove from agent").on_press(move |s: &mut App| s.remove_from_agent(&n)).build(k).key("key-agent-remove"));
        } else if matches!(s.agent, Agent::Running(_)) {
            let n = n.clone();
            left = left.child(small("Add to agent").help("ssh-add: ssh stops asking for the passphrase").on_press(move |s: &mut App| s.add_to_agent(&n)).build(k).key("key-agent-add"));
        }
        let (n1, n2) = (n.clone(), n.clone());
        let pass_label = if key.encrypted == Some(false) { "Set passphrase…" } else { "Change passphrase…" };
        left = left.child(small(pass_label).on_press(move |s: &mut App| s.open_pass_form(&n1, PassPurpose::Change)).build(k).key("key-passphrase"));
        left = left.child(small("Edit comment…").on_press(move |s: &mut App| s.open_pass_form(&n2, PassPurpose::Comment)).build(k).key("key-comment"));
        if key.public.is_none() && key.public_text.is_some() {
            let n = n.clone();
            left = left.child(small("Write .pub").help("Save the public half next to the key").on_press(move |s: &mut App| s.write_public(&n)).build(k).key("key-write-pub"));
        }
    }
    let n3 = n.clone();
    left = left.child(small(if cfg!(target_os = "macos") { "Show in Finder" } else { "Show folder" }).on_press(move |s: &mut App| s.reveal(&n3)).build(k).key("key-reveal"));
    row()
        .key("key-actions")
        .w_full()
        .gap(8.0)
        .items_start()
        .child(left.grow(1.0).min_w(0.0))
        .child(Button::new("Move to Trash…").variant(ButtonVariant::Danger).control_size(ControlSize::Small).on_press(move |s: &mut App| s.confirm_trash_key(&n)).build(k).key("key-trash"))
}
