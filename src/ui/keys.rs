//! The keys page: the keys in `~/.ssh` with the agent above them, and the inspector with one
//! key's fingerprint, public half, the hosts that use it and what can be done to it.

use rune::{El, Tx, col, row};
use rune_kit::shell::Toolbar;
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit};

use super::{IMPORT, KEY, PLUS, REFRESH, plural};
use crate::app::{App, PassPurpose};
use crate::ds::{self, Card, Inspector, ListRow, MenuAction, Notice, Place, Tone, item_row, stacked_row, text, value_row};
use crate::keys::{Agent, Key};

pub fn page(s: &App, k: &Kit, layer: &mut Vec<El>, reduced: bool) -> (El, Toolbar, El) {
    (main(s, k, reduced), toolbar(k), inspector(s, k, layer, reduced))
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

fn main(s: &App, k: &Kit, reduced: bool) -> El {
    let filter = ds::filter(&s.filter, "Filter keys by name, comment or fingerprint").label("Filter keys").on_edit(|s: &mut App| &mut s.filter).build(k).key("filter");
    let visible = s.visible_keys();
    let mut page = ds::Page::new("keys-page").filter(filter).count(plural(visible.len(), "key")).notice(agent(s, k));
    for (i, e) in s.key_errors.iter().enumerate() {
        page = page.notice(Notice::new(Tone::Warn, e.clone()).key(format!("key-error-{i}")).build(k));
    }
    if s.keys.is_empty() {
        page = page.empty(ds::empty(k, "No keys in ~/.ssh", "Make one with New key, or import one you have."));
    } else if visible.is_empty() {
        page = page.empty(ds::empty(k, "Nothing to show", "Try another word."));
    }
    page.rows(visible.iter().map(|&i| key_row(s, &s.keys[i], k, reduced))).build(k)
}

/// The agent's state, and keys it holds that aren't in `~/.ssh`.
fn agent(s: &App, k: &Kit) -> El {
    let notice = match &s.agent {
        Agent::Down(why) => Notice::new(Tone::Warn, "ssh-agent isn't running").detail(why.clone()),
        Agent::Running(loaded) if loaded.is_empty() => Notice::new(Tone::Ok, "ssh-agent holds no keys").detail("ssh asks for passphrases every time."),
        Agent::Running(loaded) => {
            let foreign = loaded.iter().filter(|(fp, _)| !s.keys.iter().any(|k| k.fingerprint.as_deref() == Some(fp))).count();
            let mut n = Notice::new(Tone::Ok, format!("ssh-agent holds {}", plural(loaded.len(), "key")));
            if foreign > 0 {
                n = n.detail(format!("{foreign} from outside ~/.ssh"));
            }
            n.action(ds::button("Remove all", Place::List).help("Unload every key from the agent (ssh-add -D)").on_press(|s: &mut App| s.clear_agent()).build(k).key("agent-clear"))
        }
    };
    notice.key("agent").build(k)
}

fn key_row(s: &App, key: &Key, k: &Kit, reduced: bool) -> El {
    let mut tags = Vec::new();
    if s.in_agent(key) {
        tags.push(ds::tag(k, "in agent", Some(Tone::Ok), "Loaded in ssh-agent"));
    }
    match (key.private.is_some(), key.encrypted) {
        (false, _) => tags.push(ds::tag(k, "public only", None, "Only the .pub is here: a hardware key, or a copy")),
        (true, Some(true)) => tags.push(ds::tag(k, "passphrase", None, "Protected by a passphrase")),
        (true, Some(false)) => tags.push(ds::tag(k, "no passphrase", Some(Tone::Warn), "Anyone with the file can use it")),
        (true, None) => {}
    }
    if let Some(m) = key.loose_mode {
        tags.push(ds::tag(k, format!("{m:o}"), Some(Tone::Bad), "Others can read it; ssh refuses such keys"));
    }
    let used = s.hosts_using(key).len();
    let meta = [Some(match key.bits {
        Some(b) if key.kind == "RSA" => format!("RSA {b}"),
        _ => key.kind.clone(),
    }), (!key.comment.is_empty()).then(|| key.comment.clone()), (used > 0).then(|| plural(used, "host"))]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let name = key.name.clone();
    let selected = s.selected_key.as_deref() == Some(key.name.as_str());
    ListRow::new(format!("key-{}", key.name), key.name.clone()).icon(&KEY).meta(meta).tags(tags).selected(selected).reduced(reduced).build(k, move |s: &mut App| s.select_key(&name))
}

fn inspector(s: &App, k: &Kit, layer: &mut Vec<El>, reduced: bool) -> El {
    let Some(key) = s.selected_key.as_deref().and_then(|n| s.key(n)) else {
        return col().key("inspector").w_full().padding(ds::size::PANE_EDGE).child(ds::empty(k, "No key selected", "Click a key to see its fingerprint, public key and hosts."));
    };
    let (left, right) = actions(s, key, k, layer);
    let mut ins = Inspector::new().subject(format!("key-{}", key.name)).title(key.name.clone()).meta(describe(key)).actions(left, Some(right));
    if let Some(m) = key.loose_mode {
        let name = key.name.clone();
        let fix = ds::button("Fix", Place::Inspector).help("Make it readable only by you (600)").on_press(move |s: &mut App| s.fix_permissions(&name)).build(k).key("key-fix");
        ins = ins.notice(Notice::new(Tone::Bad, "Others can read this key").detail(format!("Its permissions are {m:o}, and ssh refuses to use it.")).action(fix).key("loose-warning").build(k));
    }
    ins.card(public(s, key, k)).card(details(key, k)).card(files(s, key, k)).card(used_by(s, key, k, reduced)).build(k)
}

/// `ED25519 · 256 bits · no passphrase`.
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

/// The agent toggle; a "…" menu with the passphrase, the comment, Write .pub file and Show in
/// Finder; Move to Trash on the right. One row that fits the narrowest inspector.
fn actions(s: &App, key: &Key, k: &Kit, layer: &mut Vec<El>) -> (Vec<El>, El) {
    let n = key.name.clone();
    let private = key.private.is_some();
    let mut left = Vec::new();
    if private {
        if s.in_agent(key) {
            let n = n.clone();
            left.push(ds::button("Remove from agent", Place::Inspector).on_press(move |s: &mut App| s.remove_from_agent(&n)).build(k).key("key-agent-remove"));
        } else if matches!(s.agent, Agent::Running(_)) {
            let n = n.clone();
            left.push(ds::button("Add to agent", Place::Inspector).help("ssh-add: ssh stops asking for the passphrase").on_press(move |s: &mut App| s.add_to_agent(&n)).build(k).key("key-agent-add"));
        }
    }
    // Every item is always there, so each keeps its place; what doesn't apply is disabled.
    let can_write = private && key.public.is_none() && key.public_text.is_some();
    let pass = if key.encrypted == Some(false) { "Set passphrase…" } else { "Change passphrase…" };
    let (n1, n2, n3, n4) = (n.clone(), n.clone(), n.clone(), n.clone());
    let more = ds::overflow::<App>(
        k,
        "key-more",
        Place::Inspector,
        s.key_menu,
        |s: &mut App| s.key_menu = !s.key_menu,
        |s: &mut App| s.key_menu = false,
        vec![
            MenuAction::new(pass, move |s: &mut App| s.open_pass_form(&n1, PassPurpose::Change)).disabled(!private),
            MenuAction::new("Edit comment…", move |s: &mut App| s.open_pass_form(&n2, PassPurpose::Comment)).disabled(!private),
            MenuAction::new("Write .pub file", move |s: &mut App| s.write_public(&n3))
                .description(if key.public.is_some() { "The .pub file is already there" } else { "Save the public half next to the key" })
                .disabled(!can_write),
            MenuAction::new(if cfg!(target_os = "macos") { "Show in Finder" } else { "Show folder" }, move |s: &mut App| s.reveal(&n4)),
        ],
        layer,
    );
    left.push(more);
    let trash = ds::danger("Move to Trash…", Place::Inspector).on_press(move |s: &mut App| s.confirm_trash_key(&n)).build(k).key("key-trash");
    (left, trash)
}

/// The public half, to paste into GitHub, GitLab or `authorized_keys`.
fn public(s: &App, key: &Key, k: &Kit) -> Option<El> {
    let public = key.public_text.as_ref()?;
    let copy = public.clone();
    let button = ds::copy_button("Copy", Place::Inspector, s.just_copied(public))
        .help("Paste it into GitHub, GitLab or a server's authorized_keys")
        .on_press_tx(move |s: &mut App, tx: &mut Tx| {
            tx.copy(copy.clone());
            s.copied(copy.clone());
        })
        .build(k)
        .key("key-copy");
    Card::new("card-public").label("Public key").trailing(button).row(stacked_row(k, key.kind.to_lowercase(), text::code(k, public.clone()).key("key-public"))).build(k)
}

/// The fingerprint and the comment, as far as the key has them.
fn details(key: &Key, k: &Kit) -> Option<El> {
    let mut card = Card::new("card-details").label("Details");
    if let Some(fp) = &key.fingerprint {
        card = card.row(stacked_row(k, "Fingerprint", text::code(k, fp.clone()).key("key-fingerprint")));
    }
    if !key.comment.is_empty() {
        card = card.row(value_row(text::label(k, "Comment"), text::code(k, key.comment.clone()).key("key-comment-value"), &key.comment));
    }
    card.build(k)
}

/// The private and public files with their modes. The path is the label; it gets the room a
/// pair row's fixed label column wouldn't give it.
fn files(s: &App, key: &Key, k: &Kit) -> Option<El> {
    let rows = [&key.private, &key.public].into_iter().flatten().enumerate().map(|(i, p)| {
        let path = s.paths.tilde(p);
        let mode = crate::fsops::mode(p).map(|m| text::code(k, format!("{m:o}")));
        item_row(k, text::path(k, path.clone()).help(path), None, mode).key(format!("key-file-{i}"))
    });
    Card::new("card-files").label("Files").rows(rows).build(k)
}

/// The hosts whose IdentityFile is this key; each opens its host. An unused key says so.
fn used_by(s: &App, key: &Key, k: &Kit, reduced: bool) -> Option<El> {
    let card = Card::new("card-used-by").label("Used by");
    let hosts = s.hosts_using(key);
    if hosts.is_empty() {
        let hint = "No host uses this key. ssh still offers it to every server unless IdentitiesOnly is set.";
        return card.row(text::hint(k, hint).key("key-hosts-none")).build(k);
    }
    card.rows(hosts.into_iter().enumerate().map(|(i, id)| {
        let name = s.section(id).patterns.clone();
        ds::pressable(k, item_row(k, text::name(k, name.clone()), None, None), name, reduced, move |s: &mut App| s.show_host(id)).key(format!("key-host-{i}"))
    }))
    .build(k)
}
