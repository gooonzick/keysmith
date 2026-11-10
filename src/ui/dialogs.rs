//! Dialogs: a host's form (add, edit, duplicate), a new key, a passphrase or comment, and the
//! confirmations before removing a host or trashing a key. All of them sit in the `ds` dialog
//! frame; forms group their fields in cards.

use rune::{El, ScrollSpec, TextEdit, Tx, col, row};
use rune_kit::{Checkbox, Kit, Placement, Select, TextInput, glyphs};

use super::PLUS;
use crate::app::{App, Confirm, HostForm, NewKeyForm, PassForm, PassPurpose};
use crate::ds::{self, Card, DialogFrame, DialogKind, Place, field, field_pair, plain_row, space, text};
use crate::keys::KeyType;
use crate::sshconfig::Kind;

fn host_mut(s: &mut App) -> &mut HostForm {
    s.host_form.as_mut().expect("the host form is open")
}

fn key_mut(s: &mut App) -> &mut NewKeyForm {
    s.new_key.as_mut().expect("the new key form is open")
}

fn pass_mut(s: &mut App) -> &mut PassForm {
    s.pass_form.as_mut().expect("the passphrase form is open")
}

/// A text field the form edits through `edit`, submitting with Enter.
fn input(edit: &TextEdit, label: &str, placeholder: &str, key: &str, k: &Kit, on_edit: fn(&mut App) -> &mut TextEdit, submit: fn(&mut App)) -> El {
    TextInput::new(edit).label(label).placeholder(placeholder).width(None).on_edit(on_edit).on_submit(move |s: &mut App, _: &mut Tx| submit(s)).build(k).key(key.to_string())
}

/// A field with a faint line of help under its control.
fn with_hint(k: &Kit, control: El, hint: &str) -> El {
    col().w_full().gap(space::S1).child(control).child(text::hint(k, hint))
}

/// The show / hide button of the secret fields beside it.
fn eye(shown: bool, key: &str, on: impl Fn(&mut App) + 'static, k: &Kit) -> El {
    let label = if shown { "Hide" } else { "Show" };
    ds::icon_button(if shown { &glyphs::EYE_OFF } else { &glyphs::EYE }, label, Place::Inspector).on_press(on).build(k).key(key.to_string())
}

/// Cancel and the dialog's one main action.
fn footer(k: &Kit, cancel: (&str, fn(&mut App)), ok: El) -> (El, El) {
    (ds::button("Cancel", Place::Bar).on_press(cancel.1).build(k).key(cancel.0.to_string()), ok)
}

pub fn host_form(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(f) = &s.host_form else { return };
    let save: fn(&mut App) = |s| s.save_host_form();
    // A dropdown's menu anchors to its field, which has to be laid out first: the frame puts
    // the menus into the layer after the dialog that holds the fields.
    let mut menus = Vec::new();

    let mut host = Card::new("host-form-host").label("Host");
    if f.editing.is_none() && s.docs.len() > 1 {
        let names: Vec<String> = s.docs.iter().map(|d| s.paths.tilde(&d.path)).collect();
        let current = s.paths.tilde(&f.file);
        let pick = Select::new("host-file", &names)
            .label("Save in")
            .width(320.0)
            .placement(Placement::BottomStart)
            .current(Some(&current))
            .open(f.file_menu)
            .on_toggle(|s: &mut App| host_mut(s).file_menu = !host_mut(s).file_menu)
            .on_close(|s: &mut App| host_mut(s).file_menu = false)
            .on_choose(|s: &mut App, v: &str, _: &mut Tx| {
                let path = s.docs.iter().find(|d| s.paths.tilde(&d.path) == v).map(|d| d.path.clone());
                let f = host_mut(s);
                if let Some(p) = path {
                    f.file = p;
                }
                f.file_menu = false;
            })
            .build(k, &mut menus);
        host = host.row(field(k, "Save in", row().w_full().child(pick)));
    }
    let (name_label, name_hint) = if f.kind == Kind::Match { ("Match criteria", "host *.corp user admin") } else { ("Name", "work  (or several: work w, or a pattern: *.example.com)") };
    host = host
        .row(field(k, name_label, input(&f.patterns, name_label, name_hint, "host-name", k, |s| &mut host_mut(s).patterns, save)))
        .row(field(k, "Description", input(&f.description, "Description", "Written as a comment above the host", "host-description", k, |s| &mut host_mut(s).description, save)));

    let connection = Card::new("host-form-connection")
        .label("Connection")
        .row(field_pair(
            field(k, "Address (HostName)", input(&f.hostname, "Address", "10.0.0.5 or server.example.com", "host-hostname", k, |s| &mut host_mut(s).hostname, save)),
            field(k, "Port", input(&f.port, "Port", "22", "host-port", k, |s| &mut host_mut(s).port, save)),
            Some(96.0),
        ))
        .row(field_pair(
            field(k, "User", input(&f.user, "User", "deploy", "host-user", k, |s| &mut host_mut(s).user, save)),
            field(k, "Jump through (ProxyJump)", input(&f.proxy, "Jump host", "bastion", "host-proxy", k, |s| &mut host_mut(s).proxy, save)),
            None,
        ));

    let names: Vec<String> = s.keys.iter().filter(|k| k.private.is_some() || k.public.is_some()).map(|k| k.name.clone()).collect();
    let details: Vec<Option<String>> = s.keys.iter().map(|key| Some(if s.in_agent(key) { format!("{} · in agent", key.kind) } else { key.kind.clone() })).collect();
    let key_pick = Select::new("host-identity-pick", &names)
        .label("Choose a key")
        .placeholder(if names.is_empty() { "No keys in ~/.ssh" } else { "Choose a key" })
        .details(&details)
        .width(170.0)
        .disabled(names.is_empty())
        .open(f.identity_menu)
        .on_toggle(|s: &mut App| host_mut(s).identity_menu = !host_mut(s).identity_menu)
        .on_close(|s: &mut App| host_mut(s).identity_menu = false)
        .on_choose(|s: &mut App, v: &str, _: &mut Tx| s.pick_identity(v))
        .build(k, &mut menus);
    let identity = row()
        .w_full()
        .gap(space::S2)
        .center_y()
        .child(input(&f.identity, "Key file", "~/.ssh/id_ed25519", "host-identity", k, |s| &mut host_mut(s).identity, save).grow(1.0).min_w(0.0))
        .child(key_pick);
    let key = Card::new("host-form-key").label("Key").row(field(k, "File (IdentityFile)", identity));

    let editing = f.editing.is_some();
    let ok = ds::primary(if editing { "Save" } else { "Add host" }, Place::Bar).on_press(|s: &mut App| s.save_host_form()).build(k).key("host-save");
    let (cancel, ok) = footer(k, ("host-cancel", |s| s.close_host_form()), ok);
    let title = match &f.editing {
        Some(r) => format!("Edit {}", r.patterns),
        None => "Add a host".to_string(),
    };
    DialogFrame::new("host-form", title, DialogKind::Form)
        .body(host.build(k).expect("the name row"))
        .body(connection.build(k).expect("two rows"))
        .body(key.build(k).expect("one row"))
        .body(extra(f, k))
        .error(f.error.clone())
        .footer(cancel, ok)
        .menus(menus)
        .build(k, |s: &mut App| s.close_host_form(), layer);
}

/// Every other option as a name and a value.
fn extra(f: &HostForm, k: &Kit) -> El {
    let mut card = Card::new("host-form-options").label("Other options");
    if !f.extra.is_empty() {
        let mut rows = col().key("host-extra").w_full().gap(space::S2);
        for (i, p) in f.extra.iter().enumerate() {
            let key = TextInput::new(&p.key)
                .label("Option")
                .placeholder("ForwardAgent")
                .width(Some(180.0))
                .on_edit(move |s: &mut App| &mut host_mut(s).extra[i].key)
                .build(k)
                .key(format!("host-extra-{i}-key"));
            let value = TextInput::new(&p.value)
                .label("Value")
                .placeholder("yes")
                .width(None)
                .on_edit(move |s: &mut App| &mut host_mut(s).extra[i].value)
                .on_submit(|s: &mut App, _: &mut Tx| s.save_host_form())
                .build(k)
                .key(format!("host-extra-{i}-value"));
            let remove = ds::icon_button(&glyphs::CLOSE, "Remove option", Place::Inspector).on_press(move |s: &mut App| s.remove_host_pair(i)).build(k).key(format!("host-extra-{i}-remove"));
            rows = rows.child(row().w_full().gap(space::S2).center_y().child(key).child(value.grow(1.0).min_w(0.0)).child(remove));
        }
        // The scroll view clips at its edges, and a field's focus ring is drawn just outside the
        // field: the rows get a little room inside it, and the view sits that much further out
        // so the fields still line up with the rest of the form.
        const RING: f32 = 3.0;
        let rows = rows.padding(RING);
        card = card.row(col().w_full().mx(-RING).my(-RING).max_h(170.0 + 2.0 * RING).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rows));
    }
    let add = ds::quiet("Add option", Place::Inspector).icon(&PLUS).on_press(|s: &mut App| s.add_host_pair()).build(k).key("host-extra-add");
    card.row(plain_row(with_hint(k, row().child(add), "Any ssh_config option: IdentitiesOnly, ForwardAgent, LocalForward, ServerAliveInterval…"))).build(k).expect("the add row")
}

pub fn new_key(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(f) = &s.new_key else { return };
    let save: fn(&mut App) = |s| s.save_new_key();
    let labels: Vec<&str> = KeyType::ALL.iter().map(|t| t.label()).collect();
    let mut menus = Vec::new();
    let ty = Select::new("key-type", &labels)
        .label("Type")
        .width(220.0)
        .placement(Placement::BottomStart)
        .current(Some(f.ty.label()))
        .open(f.type_menu)
        .on_toggle(|s: &mut App| key_mut(s).type_menu = !key_mut(s).type_menu)
        .on_close(|s: &mut App| key_mut(s).type_menu = false)
        .on_choose(|s: &mut App, v: &str, _: &mut Tx| {
            if let Some(t) = KeyType::ALL.iter().find(|t| t.label() == v) {
                s.set_new_key_type(*t);
            }
        })
        .build(k, &mut menus);
    let key = Card::new("new-key-key")
        .label("Key")
        .row(field(k, "Type", with_hint(k, row().w_full().child(ty), "Ed25519 suits nearly every server. RSA only for old ones that lack it.")))
        .row(field(k, "File name in ~/.ssh", input(&f.name, "File name", "id_ed25519_work", "key-name", k, |s| &mut key_mut(s).name, save)))
        .row(field(k, "Comment", input(&f.comment, "Comment", "you@laptop", "key-comment-input", k, |s| &mut key_mut(s).comment, save)));

    let pass = TextInput::new(&f.pass).label("Passphrase").placeholder("Empty for none").width(None).secure().revealed(f.shown).on_edit(|s: &mut App| &mut key_mut(s).pass).on_submit(|s: &mut App, _: &mut Tx| s.save_new_key()).build(k).key("key-pass");
    let confirm = TextInput::new(&f.confirm).label("Again").placeholder("Repeat it").width(None).secure().revealed(f.shown).on_edit(|s: &mut App| &mut key_mut(s).confirm).on_submit(|s: &mut App, _: &mut Tx| s.save_new_key()).build(k).key("key-pass-confirm");
    let secrets = row()
        .w_full()
        .gap(space::S2)
        .center_y()
        .child(pass.grow(1.0).min_w(0.0))
        .child(confirm.grow(1.0).min_w(0.0))
        .child(eye(f.shown, "key-pass-show", |s: &mut App| key_mut(s).shown = !key_mut(s).shown, k));
    let passphrase = Card::new("new-key-passphrase").label("Passphrase").row(plain_row(with_hint(k, secrets, "It encrypts the key on disk. The agent remembers it, so you type it once.")));

    let agent_ok = matches!(s.agent, crate::keys::Agent::Running(_));
    let mut agent = Card::new("new-key-agent").label("Agent").row(
        Checkbox::new(f.add_to_agent && agent_ok)
            .label("Add to ssh-agent")
            .description(if agent_ok { "Ready to use right away" } else { "No agent is running" })
            .disabled(!agent_ok)
            .on_toggle(|s: &mut App| key_mut(s).add_to_agent = !key_mut(s).add_to_agent)
            .build(k)
            .key("key-add-agent"),
    );
    if cfg!(target_os = "macos") {
        agent = agent.row(
            Checkbox::new(f.keychain && f.add_to_agent && agent_ok && !f.pass.text.is_empty())
                .label("Remember the passphrase in Keychain")
                .description("The agent loads it again after a restart without asking")
                .disabled(!f.add_to_agent || !agent_ok || f.pass.text.is_empty())
                .on_toggle(|s: &mut App| key_mut(s).keychain = !key_mut(s).keychain)
                .build(k)
                .key("key-keychain"),
        );
    }

    let ok = ds::primary("Create key", Place::Bar).on_press(|s: &mut App| s.save_new_key()).build(k).key("key-create");
    let (cancel, ok) = footer(k, ("key-cancel", |s| s.new_key = None), ok);
    DialogFrame::new("new-key", "New SSH key", DialogKind::Form)
        .body(key.build(k).expect("three rows"))
        .body(passphrase.build(k).expect("one row"))
        .body(agent.build(k).expect("one row"))
        .error(f.error.clone())
        .footer(cancel, ok)
        .menus(menus)
        .build(k, |s: &mut App| s.new_key = None, layer);
}

pub fn pass_form(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(f) = &s.pass_form else { return };
    let Some(key) = s.key(&f.key) else { return };
    let needs_old = key.encrypted != Some(false);
    let secret = |edit: &TextEdit, label: &str, placeholder: &str, kk: &str, on: fn(&mut App) -> &mut TextEdit| {
        TextInput::new(edit).label(label).placeholder(placeholder).width(None).secure().revealed(f.shown).on_edit(on).on_submit(|s: &mut App, _: &mut Tx| s.save_pass_form()).build(k).key(kk.to_string())
    };
    let show = || eye(f.shown, "pass-show", |s: &mut App| pass_mut(s).shown = !pass_mut(s).shown, k);
    let mut intro = None;
    let mut card = Card::new("pass-form-fields");
    let (title, ok) = match f.purpose {
        PassPurpose::Unlock => {
            intro = Some(text::body(k, format!("{} is protected. Its passphrase unlocks it for ssh-agent.", key.name)));
            card = card.row(field(k, "Passphrase", row().w_full().gap(space::S2).center_y().child(secret(&f.old, "Passphrase", "", "pass-old", |s| &mut pass_mut(s).old).grow(1.0).min_w(0.0)).child(show())));
            if cfg!(target_os = "macos") {
                card = card.row(Checkbox::new(f.keychain).label("Remember in Keychain").description("Loads again after a restart without asking").on_toggle(|s: &mut App| pass_mut(s).keychain = !pass_mut(s).keychain).build(k).key("pass-keychain"));
            }
            (format!("Add {} to the agent", key.name), "Add to agent")
        }
        PassPurpose::Change => {
            if needs_old {
                card = card.row(field(k, "Current passphrase", secret(&f.old, "Current passphrase", "", "pass-old", |s| &mut pass_mut(s).old)));
            }
            card = card.row(field(
                k,
                "New passphrase",
                row()
                    .w_full()
                    .gap(space::S2)
                    .center_y()
                    .child(secret(&f.new, "New passphrase", if needs_old { "Empty removes it" } else { "New passphrase" }, "pass-new", |s| &mut pass_mut(s).new).grow(1.0).min_w(0.0))
                    .child(secret(&f.confirm, "Again", "Repeat it", "pass-confirm", |s| &mut pass_mut(s).confirm).grow(1.0).min_w(0.0))
                    .child(show()),
            ));
            (format!("Passphrase of {}", key.name), "Change passphrase")
        }
        PassPurpose::Comment => {
            card = card.row(field(k, "Comment", TextInput::new(&f.comment).label("Comment").width(None).on_edit(|s: &mut App| &mut pass_mut(s).comment).on_submit(|s: &mut App, _: &mut Tx| s.save_pass_form()).build(k).key("pass-comment")));
            if needs_old {
                card = card.row(field(k, "Passphrase", secret(&f.old, "Passphrase", "", "pass-old", |s| &mut pass_mut(s).old)));
            }
            (format!("Comment of {}", key.name), "Save comment")
        }
    };
    let ok = ds::primary(ok, Place::Bar).on_press(|s: &mut App| s.save_pass_form()).build(k).key("pass-save");
    let (cancel, ok) = footer(k, ("pass-cancel", |s| s.close_pass_form()), ok);
    let mut frame = DialogFrame::new("pass-form", title, DialogKind::Confirm);
    if let Some(i) = intro {
        frame = frame.body(i);
    }
    frame.body(card.build(k).expect("a field")).error(f.error.clone()).footer(cancel, ok).build(k, |s: &mut App| s.close_pass_form(), layer);
}

pub fn confirm(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(c) = &s.confirm else { return };
    let (title, detail, ok) = match c {
        Confirm::RemoveHost(r) => (format!("Remove {}?", r.patterns), format!("Its section and description come out of {}. A backup stays beside it, and Undo puts it back.", s.paths.tilde(&r.path)), "Remove host"),
        Confirm::TrashKey(name) => {
            let users: Vec<String> = s.key(name).map(|key| s.hosts_using(key).into_iter().map(|id| s.section(id).patterns.clone()).collect()).unwrap_or_default();
            let mut d = format!("{name} and its .pub go to the Trash; Undo brings them back.");
            if !users.is_empty() {
                d.push_str(&format!(" {} still name it: {}.", if users.len() == 1 { "One host" } else { "These hosts" }, users.join(", ")));
            }
            (format!("Move {name} to the Trash?"), d, "Move to Trash")
        }
    };
    let ok = ds::danger(ok, Place::Bar)
        .on_press(|s: &mut App| match s.confirm.take() {
            Some(Confirm::RemoveHost(r)) => s.remove_host(&r),
            Some(Confirm::TrashKey(n)) => s.trash_key(&n),
            None => {}
        })
        .build(k)
        .key("confirm-ok");
    let (cancel, ok) = footer(k, ("confirm-cancel", |s| s.confirm = None), ok);
    DialogFrame::new("confirm", title, DialogKind::Confirm).body(text::body(k, detail)).footer(cancel, ok).build(k, |s: &mut App| s.confirm = None, layer);
}
