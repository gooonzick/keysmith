//! Dialogs: a host's form (add, edit, duplicate), a new key, a passphrase or comment, and the
//! confirmations before removing a host or trashing a key.

use rune::{El, ScrollSpec, Tx, col, icon, row, text};
use rune_kit::{Button, ButtonVariant, Checkbox, ControlSize, Dialog, IconButton, IconButtonVariant, Kit, Placement, Select, TextInput, glyphs};

use crate::app::{App, Confirm, HostForm, NewKeyForm, PassForm, PassPurpose};
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

fn body() -> El {
    col().w_full().px(16.0).pb(16.0).gap(14.0)
}

fn note(t: impl Into<String>, k: &Kit) -> El {
    text(t.into()).font(k.ty.small).color(k.c.text_muted).wrap()
}

fn error_line(e: &str, key: &str, k: &Kit) -> El {
    row().key(key.to_string()).w_full().gap(6.0).items_start().child(icon(&glyphs::CIRCLE_X, 14.0).color(k.c.danger.icon)).child(text(e.to_string()).font(k.ty.small).color(k.c.text_muted).grow(1.0).min_w(0.0).wrap())
}

fn block(label: &str, content: El, k: &Kit) -> El {
    col().w_full().gap(6.0).child(text(label.to_string()).font(k.ty.small_medium).color(k.c.text_base)).child(content)
}

fn buttons(cancel: El, ok: El) -> El {
    row().w_full().gap(8.0).justify_end().child(cancel).child(ok)
}

/// A text field the form edits through `edit`, submitting with Enter.
fn input(edit: &rune::TextEdit, label: &str, placeholder: &str, key: &str, k: &Kit, on_edit: fn(&mut App) -> &mut rune::TextEdit, submit: fn(&mut App)) -> El {
    TextInput::new(edit).label(label).placeholder(placeholder).width(None).on_edit(on_edit).on_submit(move |s: &mut App, _: &mut Tx| submit(s)).build(k).key(key.to_string())
}

pub fn host_form(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(f) = &s.host_form else { return };
    let save: fn(&mut App) = |s| s.save_host_form();
    // A dropdown's menu anchors to its field, which has to be laid out first: the menus go
    // into the layer after the dialog that holds the fields.
    let mut menus = Vec::new();
    let mut b = body();
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
        b = b.child(block("Save in", row().w_full().child(pick), k));
    }
    let (name_label, name_hint) = if f.kind == Kind::Match { ("Match criteria", "host *.corp user admin") } else { ("Name", "work  (or several: work w, or a pattern: *.example.com)") };
    b = b.child(block(name_label, input(&f.patterns, name_label, name_hint, "host-name", k, |s| &mut host_mut(s).patterns, save), k));
    b = b.child(block("Description", input(&f.description, "Description", "Written as a comment above the host", "host-description", k, |s| &mut host_mut(s).description, save), k));
    b = b.child(
        row()
            .w_full()
            .gap(10.0)
            .items_end()
            .child(block("Address (HostName)", input(&f.hostname, "Address", "10.0.0.5 or server.example.com", "host-hostname", k, |s| &mut host_mut(s).hostname, save), k).grow(1.0).min_w(0.0))
            .child(block("Port", input(&f.port, "Port", "22", "host-port", k, |s| &mut host_mut(s).port, save), k).w(96.0)),
    );
    b = b.child(
        row()
            .w_full()
            .gap(10.0)
            .items_end()
            .child(block("User", input(&f.user, "User", "deploy", "host-user", k, |s| &mut host_mut(s).user, save), k).grow(1.0).min_w(0.0))
            .child(block("Jump through (ProxyJump)", input(&f.proxy, "Jump host", "bastion", "host-proxy", k, |s| &mut host_mut(s).proxy, save), k).grow(1.0).min_w(0.0)),
    );
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
    let identity = row().w_full().gap(8.0).center_y().child(input(&f.identity, "Key file", "~/.ssh/id_ed25519", "host-identity", k, |s| &mut host_mut(s).identity, save).grow(1.0).min_w(0.0)).child(key_pick);
    b = b.child(block("Key (IdentityFile)", identity, k));
    b = b.child(extra(f, k));
    if let Some(e) = &f.error {
        b = b.child(error_line(e, "host-error", k));
    }
    let editing = f.editing.is_some();
    let b = b.child(buttons(
        Button::new("Cancel").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.close_host_form()).build(k).key("host-cancel"),
        Button::new(if editing { "Save" } else { "Add host" }).variant(ButtonVariant::Contrast).on_press(|s: &mut App| s.save_host_form()).build(k).key("host-save"),
    ));
    let title = match &f.editing {
        Some(r) => format!("Edit {}", r.patterns),
        None => "Add a host".to_string(),
    };
    Dialog::new("host-form", title).width(620.0).fit().open(true).on_close(|s: &mut App| s.close_host_form()).child(b).build(k, layer);
    layer.append(&mut menus);
}

/// Every other option as a key and a value.
fn extra(f: &HostForm, k: &Kit) -> El {
    let mut rows = col().key("host-extra").w_full().gap(6.0);
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
        rows = rows.child(
            row().w_full().gap(6.0).center_y().child(key).child(value.grow(1.0).min_w(0.0)).child(
                IconButton::new(&glyphs::CLOSE)
                    .variant(IconButtonVariant::GhostMuted)
                    .label("Remove option")
                    .help("Remove option")
                    .on_press(move |s: &mut App| s.remove_host_pair(i))
                    .build(k)
                    .key(format!("host-extra-{i}-remove")),
            ),
        );
    }
    // The scroll view clips at its edges, and a field's focus ring is drawn just outside the
    // field: the rows get a little room inside it, and the view sits that much further out so
    // the fields still line up with the rest of the form.
    const RING: f32 = 3.0;
    let rows = rows.padding(RING);
    let list = col().w_full().mx(-RING).my(-RING).max_h(170.0 + 2.0 * RING).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rows);
    let add = Button::new("Add option").icon(&super::PLUS).variant(ButtonVariant::Ghost).control_size(ControlSize::Small).on_press(|s: &mut App| s.add_host_pair()).build(k).key("host-extra-add");
    block(
        "Other options",
        col().w_full().gap(6.0).child(list).child(row().child(add)).child(note("Any ssh_config option: IdentitiesOnly, ForwardAgent, LocalForward, ServerAliveInterval…", k)),
        k,
    )
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
    let mut b = body()
        .child(block("Type", col().w_full().gap(6.0).child(row().w_full().child(ty)).child(note("Ed25519 suits nearly every server. RSA only for old ones that lack it.", k)), k))
        .child(block("File name in ~/.ssh", input(&f.name, "File name", "id_ed25519_work", "key-name", k, |s| &mut key_mut(s).name, save), k));
    b = b.child(block("Comment", input(&f.comment, "Comment", "you@laptop", "key-comment-input", k, |s| &mut key_mut(s).comment, save), k));
    let pass = TextInput::new(&f.pass).label("Passphrase").placeholder("Empty for none").width(None).secure().revealed(f.shown).on_edit(|s: &mut App| &mut key_mut(s).pass).on_submit(|s: &mut App, _: &mut Tx| s.save_new_key()).build(k).key("key-pass");
    let confirm = TextInput::new(&f.confirm).label("Again").placeholder("Repeat it").width(None).secure().revealed(f.shown).on_edit(|s: &mut App| &mut key_mut(s).confirm).on_submit(|s: &mut App, _: &mut Tx| s.save_new_key()).build(k).key("key-pass-confirm");
    b = b.child(block(
        "Passphrase",
        col()
            .w_full()
            .gap(6.0)
            .child(row().w_full().gap(8.0).center_y().child(pass.grow(1.0).min_w(0.0)).child(confirm.grow(1.0).min_w(0.0)).child(eye(f.shown, "key-pass-show", |s: &mut App| key_mut(s).shown = !key_mut(s).shown, k)))
            .child(note("It encrypts the key on disk. The agent remembers it, so you type it once.", k)),
        k,
    ));
    let agent_ok = matches!(s.agent, crate::keys::Agent::Running(_));
    let mut opts = col().w_full().gap(6.0).child(
        Checkbox::new(f.add_to_agent && agent_ok)
            .label("Add to ssh-agent")
            .description(if agent_ok { "Ready to use right away" } else { "No agent is running" })
            .disabled(!agent_ok)
            .on_toggle(|s: &mut App| key_mut(s).add_to_agent = !key_mut(s).add_to_agent)
            .build(k)
            .key("key-add-agent"),
    );
    if cfg!(target_os = "macos") {
        opts = opts.child(
            Checkbox::new(f.keychain && f.add_to_agent && agent_ok && !f.pass.text.is_empty())
                .label("Remember the passphrase in Keychain")
                .description("The agent loads it again after a restart without asking")
                .disabled(!f.add_to_agent || !agent_ok || f.pass.text.is_empty())
                .on_toggle(|s: &mut App| key_mut(s).keychain = !key_mut(s).keychain)
                .build(k)
                .key("key-keychain"),
        );
    }
    b = b.child(opts);
    if let Some(e) = &f.error {
        b = b.child(error_line(e, "key-error", k));
    }
    let b = b.child(buttons(
        Button::new("Cancel").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.new_key = None).build(k).key("key-cancel"),
        Button::new("Create key").variant(ButtonVariant::Contrast).on_press(|s: &mut App| s.save_new_key()).build(k).key("key-create"),
    ));
    Dialog::new("new-key", "New SSH key").width(580.0).fit().open(true).on_close(|s: &mut App| s.new_key = None).child(b).build(k, layer);
    layer.append(&mut menus);
}

fn eye(shown: bool, key: &str, on: impl Fn(&mut App) + 'static, k: &Kit) -> El {
    let label = if shown { "Hide" } else { "Show" };
    IconButton::new(if shown { &glyphs::EYE_OFF } else { &glyphs::EYE }).variant(IconButtonVariant::GhostMuted).label(label).help(label).on_press(on).build(k).key(key.to_string())
}

pub fn pass_form(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(f) = &s.pass_form else { return };
    let Some(key) = s.key(&f.key) else { return };
    let needs_old = key.encrypted != Some(false);
    let secret = |edit: &rune::TextEdit, label: &str, placeholder: &str, kk: &str, on: fn(&mut App) -> &mut rune::TextEdit| {
        TextInput::new(edit).label(label).placeholder(placeholder).width(None).secure().revealed(f.shown).on_edit(on).on_submit(|s: &mut App, _: &mut Tx| s.save_pass_form()).build(k).key(kk.to_string())
    };
    let mut b = body();
    let (title, ok) = match f.purpose {
        PassPurpose::Unlock => {
            b = b.child(note(format!("{} is protected. Its passphrase unlocks it for ssh-agent.", key.name), k));
            b = b.child(block("Passphrase", row().w_full().gap(8.0).center_y().child(secret(&f.old, "Passphrase", "", "pass-old", |s| &mut pass_mut(s).old).grow(1.0).min_w(0.0)).child(eye(f.shown, "pass-show", |s: &mut App| pass_mut(s).shown = !pass_mut(s).shown, k)), k));
            if cfg!(target_os = "macos") {
                b = b.child(Checkbox::new(f.keychain).label("Remember in Keychain").description("Loads again after a restart without asking").on_toggle(|s: &mut App| pass_mut(s).keychain = !pass_mut(s).keychain).build(k).key("pass-keychain"));
            }
            (format!("Add {} to the agent", key.name), "Add to agent")
        }
        PassPurpose::Change => {
            if needs_old {
                b = b.child(block("Current passphrase", secret(&f.old, "Current passphrase", "", "pass-old", |s| &mut pass_mut(s).old), k));
            }
            b = b.child(block(
                "New passphrase",
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(secret(&f.new, "New passphrase", if needs_old { "Empty removes it" } else { "New passphrase" }, "pass-new", |s| &mut pass_mut(s).new).grow(1.0).min_w(0.0))
                    .child(secret(&f.confirm, "Again", "Repeat it", "pass-confirm", |s| &mut pass_mut(s).confirm).grow(1.0).min_w(0.0))
                    .child(eye(f.shown, "pass-show", |s: &mut App| pass_mut(s).shown = !pass_mut(s).shown, k)),
                k,
            ));
            (format!("Passphrase of {}", key.name), "Change passphrase")
        }
        PassPurpose::Comment => {
            b = b.child(block("Comment", TextInput::new(&f.comment).label("Comment").width(None).on_edit(|s: &mut App| &mut pass_mut(s).comment).on_submit(|s: &mut App, _: &mut Tx| s.save_pass_form()).build(k).key("pass-comment"), k));
            if needs_old {
                b = b.child(block("Passphrase", secret(&f.old, "Passphrase", "", "pass-old", |s| &mut pass_mut(s).old), k));
            }
            (format!("Comment of {}", key.name), "Save comment")
        }
    };
    if let Some(e) = &f.error {
        b = b.child(error_line(e, "pass-error", k));
    }
    let b = b.child(buttons(
        Button::new("Cancel").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.close_pass_form()).build(k).key("pass-cancel"),
        Button::new(ok).variant(ButtonVariant::Contrast).on_press(|s: &mut App| s.save_pass_form()).build(k).key("pass-save"),
    ));
    Dialog::new("pass-form", title).width(520.0).fit().open(true).on_close(|s: &mut App| s.close_pass_form()).child(b).build(k, layer);
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
    let b = body().child(note(detail, k)).child(buttons(
        Button::new("Cancel").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.confirm = None).build(k).key("confirm-cancel"),
        Button::new(ok)
            .variant(ButtonVariant::Danger)
            .on_press(|s: &mut App| match s.confirm.take() {
                Some(Confirm::RemoveHost(r)) => s.remove_host(&r),
                Some(Confirm::TrashKey(n)) => s.trash_key(&n),
                None => {}
            })
            .build(k)
            .key("confirm-ok"),
    ));
    Dialog::new("confirm", title).width(460.0).fit().open(true).on_close(|s: &mut App| s.confirm = None).child(b).build(k, layer);
}
