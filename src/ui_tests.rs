//! The window driven headlessly: clicks on real elements against a throwaway home folder.

use std::fs;
use std::path::PathBuf;

use rune::{Env, Key, KeyEvent, Mods, Runtime, TextEdit};

use crate::app::{App, Page};
use crate::keys::{self, KeyType};
use crate::paths::Paths;

const CONFIG: &str = "\
# Work box
Host work
  HostName 10.0.0.5
  User deploy
  IdentityFile ~/.ssh/id_work

Host *
  ServerAliveInterval 30
";

struct Harness {
    _home: tempfile::TempDir,
    paths: Paths,
    rt: Runtime<App>,
}

impl Harness {
    fn new() -> Harness {
        let home = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(home.path()).unwrap();
        let paths = Paths::isolated(&root);
        fs::create_dir_all(&paths.ssh).unwrap();
        fs::write(&paths.config, CONFIG).unwrap();
        keys::generate(&paths, KeyType::Ed25519, "id_work", "work", "").unwrap();
        let app = App::new(paths.clone());
        let env = Env { system_dark: false, scale: 2.0 };
        let mut rt = Runtime::new(app, crate::ui::view, (1200.0, 760.0), env);
        rt.on_key = Some(crate::ui::on_key);
        rt.frame(0.0);
        Harness { _home: home, paths, rt }
    }

    fn click(&mut self, key: &str) {
        let (x, y) = self.rt.find(key).unwrap_or_else(|| panic!("no element keyed {key}"));
        self.rt.pointer_down(x, y);
        self.rt.pointer_up(x, y);
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..40 {
            self.rt.frame(1.0 / 60.0);
        }
    }

    /// Elements on screen (not leaving) whose key starts with `prefix`: (key, opacity).
    fn keyed(&self, prefix: &str) -> Vec<(String, f32)> {
        self.rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key.filter(|k| k.starts_with(prefix)).map(|k| (k, e.opacity))).collect()
    }

    /// Replaces the config and reads everything again.
    fn rewrite(&mut self, text: &str) {
        fs::write(&self.paths.config, text).unwrap();
        self.rt.state.rescan();
        self.rt.invalidate();
        self.settle();
    }

    fn config(&self) -> String {
        fs::read_to_string(&self.paths.config).unwrap()
    }

    fn ssh(&self, name: &str) -> PathBuf {
        self.paths.ssh.join(name)
    }
}

#[test]
fn opens_on_the_first_host_with_its_key() {
    let mut h = Harness::new();
    assert_eq!(h.rt.state.page, Page::Hosts);
    assert!(h.rt.find("host-0-0").is_some() && h.rt.find("host-0-1").is_some());
    assert_eq!(h.rt.state.selected_host.as_ref().map(|r| r.patterns.as_str()), Some("work"));
    // The host's IdentityFile is the scanned key, so the inspector links to it.
    h.click("identity-0-show");
    assert_eq!(h.rt.state.page, Page::Keys);
    assert_eq!(h.rt.state.selected_key.as_deref(), Some("id_work"));
    h.click("key-host-0");
    assert_eq!(h.rt.state.page, Page::Hosts);
}

#[test]
fn add_a_host_with_a_picked_key_then_undo() {
    let mut h = Harness::new();
    h.click("add-host");
    {
        let f = h.rt.state.host_form.as_mut().expect("the form is open");
        f.patterns = TextEdit::new("pi");
        f.hostname = TextEdit::new("192.168.1.9");
        f.user = TextEdit::new("pi");
    }
    h.rt.state.pick_identity("id_work");
    h.click("host-extra-add");
    {
        let f = h.rt.state.host_form.as_mut().unwrap();
        f.extra[0].key = TextEdit::new("IdentitiesOnly");
        f.extra[0].value = TextEdit::new("yes");
    }
    h.settle();
    h.click("host-save");
    assert!(h.rt.state.host_form.is_none(), "saved: {:?}", h.rt.state.host_form.as_ref().and_then(|f| f.error.clone()));
    let text = h.config();
    assert!(text.contains("  IdentityFile ~/.ssh/id_work\n\nHost pi\n  HostName 192.168.1.9\n  User pi\n  IdentityFile ~/.ssh/id_work\n  IdentitiesOnly yes\n\nHost *\n"), "{text}");
    assert_eq!(fs::read_to_string(h.ssh("config.keysmith.bak")).unwrap(), CONFIG, "a backup of the old file");
    assert_eq!(h.rt.state.selected_host.as_ref().map(|r| r.patterns.as_str()), Some("pi"));

    h.rt.state.undo();
    assert_eq!(h.config(), CONFIG);
}

#[test]
fn the_form_refuses_bad_input() {
    let mut h = Harness::new();
    h.click("add-host");
    h.click("host-save");
    assert!(h.rt.state.host_form.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("name")));
    h.rt.state.host_form.as_mut().unwrap().patterns = TextEdit::new("work");
    h.click("host-save");
    assert!(h.rt.state.host_form.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("already a host")));
    let f = h.rt.state.host_form.as_mut().unwrap();
    f.patterns = TextEdit::new("new");
    f.port = TextEdit::new("99999");
    h.click("host-save");
    assert!(h.rt.state.host_form.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("port")));
    assert_eq!(h.config(), CONFIG, "nothing written");
}

#[test]
fn edit_keeps_everything_else() {
    let mut h = Harness::new();
    h.click("host-0-0");
    h.click("host-edit");
    h.rt.state.host_form.as_mut().unwrap().user = TextEdit::new("root");
    h.rt.state.host_form.as_mut().unwrap().port = TextEdit::new("2222");
    h.click("host-save");
    assert_eq!(h.config(), CONFIG.replace("  User deploy\n", "  User root\n").replace("id_work\n\n", "id_work\n  Port 2222\n\n"));
}

#[test]
fn remove_asks_first_and_move_reorders() {
    let mut h = Harness::new();
    h.click("host-0-0");
    h.click("host-more");
    // rune-kit's Menu keys its items `<menu>-item<i>`: 0 is Move up, 1 is Move down.
    h.click("host-more-popup-item1");
    assert!(h.config().starts_with("Host *\n"), "{}", h.config());
    assert_eq!(h.rt.state.selected_host_id(), Some((0, 1)), "the selection follows the host");
    h.rt.state.undo();
    assert_eq!(h.config(), CONFIG);

    h.rt.state.select_host((0, 0));
    h.settle();
    h.click("host-remove");
    assert!(h.rt.state.confirm.is_some());
    h.click("confirm-cancel");
    assert_eq!(h.config(), CONFIG);
    h.click("host-remove");
    h.click("confirm-ok");
    assert_eq!(h.config(), "Host *\n  ServerAliveInterval 30\n");
}

#[test]
fn new_key_from_the_dialog_then_trash_and_undo() {
    let mut h = Harness::new();
    h.click("nav-keys");
    h.click("new-key");
    {
        let f = h.rt.state.new_key.as_mut().expect("the dialog is open");
        assert_eq!(f.name.text, "id_ed25519");
        f.pass = TextEdit::new("one").secure();
        f.confirm = TextEdit::new("two").secure();
    }
    h.click("key-create");
    assert!(h.rt.state.new_key.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("match")));
    h.rt.state.new_key.as_mut().unwrap().confirm = TextEdit::new("one").secure();
    h.click("key-create");
    assert!(h.rt.state.new_key.is_none());
    let key = h.rt.state.key("id_ed25519").expect("scanned").clone();
    assert_eq!(key.encrypted, Some(true));
    assert_eq!(key.comment, "me@test");
    assert!(h.rt.find("key-id_ed25519").is_some());

    h.click("key-trash");
    h.click("confirm-ok");
    assert!(!h.ssh("id_ed25519").exists());
    assert!(h.rt.state.key("id_ed25519").is_none());
    h.rt.state.undo();
    assert!(h.ssh("id_ed25519").exists() && h.ssh("id_ed25519.pub").exists());
}

#[test]
fn passphrase_dialog_changes_it() {
    let mut h = Harness::new();
    h.click("nav-keys");
    h.click("key-id_work");
    h.click("key-passphrase");
    {
        let f = h.rt.state.pass_form.as_mut().expect("open");
        f.new = TextEdit::new("secret").secure();
        f.confirm = TextEdit::new("secret").secure();
    }
    h.click("pass-save");
    assert!(h.rt.state.pass_form.is_none(), "{:?}", h.rt.state.pass_form.as_ref().and_then(|f| f.error.clone()));
    assert_eq!(h.rt.state.key("id_work").unwrap().encrypted, Some(true));

    h.click("key-passphrase");
    h.rt.state.pass_form.as_mut().unwrap().old = TextEdit::new("wrong").secure();
    h.click("pass-save");
    assert_eq!(h.rt.state.pass_form.as_ref().unwrap().error.as_deref(), Some("Wrong passphrase"));
}

#[test]
fn filter_and_keyboard() {
    let mut h = Harness::new();
    h.rt.state.filter = TextEdit::new("10.0.0");
    h.settle();
    assert_eq!(h.rt.state.visible_hosts(), vec![(0, 0)]);
    h.rt.state.filter = TextEdit::new("");
    h.rt.state.step_selection(1);
    assert_eq!(h.rt.state.selected_host_id(), Some((0, 1)));
    h.rt.state.open_page(Page::Keys);
    h.rt.state.filter = TextEdit::new("nothing-like-this");
    h.settle();
    assert!(h.rt.state.visible_keys().is_empty());
}

#[test]
fn a_dialogs_dropdown_opens_under_its_field() {
    let mut h = Harness::new();
    h.click("add-host");
    let (tx, ty) = h.rt.find("host-identity-pick-trigger").expect("the key picker");
    h.click("host-identity-pick-trigger");
    assert!(h.rt.state.host_form.as_ref().unwrap().identity_menu, "open");
    let (ox, oy) = h.rt.find("option0").expect("the menu's first row");
    assert!(oy > ty && oy - ty < 60.0, "the menu sits right under the field: field at {ty}, row at {oy}");
    assert!((ox - tx).abs() < 200.0, "and beside it: field at {tx}, row at {ox}");
    h.click("option0");
    assert_eq!(h.rt.state.host_form.as_ref().unwrap().identity.text, "~/.ssh/id_work");
}

#[test]
fn copy_button_shows_a_check_then_reverts() {
    let mut h = Harness::new();
    h.rt.poll = Some(crate::poll);
    let name = |h: &Harness| h.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("host-copy")).and_then(|e| e.label);
    assert_eq!(name(&h).as_deref(), Some("Copy"));
    h.click("host-copy");
    assert!(h.rt.state.copied_at.is_some());
    assert_eq!(name(&h).as_deref(), Some("Copied"), "the button shows the check");
    for _ in 0..90 {
        h.rt.frame(1.0 / 60.0);
    }
    assert!(h.rt.state.copied_at.is_none());
    assert_eq!(name(&h).as_deref(), Some("Copy"), "the check goes after a second or so");
}

#[test]
fn hosts_inspector_is_cards() {
    let mut h = Harness::new();
    h.rt.state.refresh_effective();
    h.rt.invalidate();
    h.settle();
    for card in ["card-connect", "card-connection", "card-keys", "card-inherited"] {
        assert!(h.rt.find(card).is_some(), "work has {card}");
    }
    // `Host *` has no alias (no connection, no ssh -G) and no keys: only its options remain.
    h.click("host-0-1");
    let cards: Vec<String> = h.keyed("card-").into_iter().map(|(k, _)| k).collect();
    assert_eq!(cards, ["card-options"], "no empty card frames");
    assert!(h.rt.find("card-connection").is_none());
}

#[test]
fn long_values_stay_inside_the_inspector() {
    let mut h = Harness::new();
    let (before, _) = h.rt.presented("inspector").expect("the inspector");
    let name = "a-very-long-host-name-".repeat(8);
    h.rewrite(&format!("{CONFIG}\nHost {name}\n  HostName {}\n  ProxyCommand {}\n", "h".repeat(200), "p".repeat(400)));
    h.rt.state.select_host((0, 2));
    h.rt.invalidate();
    h.settle();
    assert!(h.rt.find("card-options").is_some(), "the ProxyCommand is shown");
    let (address, _) = h.rt.presented("value-hostname").expect("the address");
    assert!(address[3] > 18.0 * 1.5, "a long address wraps instead of being cut: {address:?}");
    let (after, _) = h.rt.presented("inspector").unwrap();
    assert!(after[2] <= 400.0, "inspector is {} wide", after[2]);
    assert_eq!(after[2], before[2], "a long value leaves the inspector's width alone");
}

#[test]
fn unreadable_include_shows_as_a_notice() {
    use std::os::unix::fs::PermissionsExt;
    let mut h = Harness::new();
    let dir = h.ssh("config.d");
    fs::create_dir_all(&dir).unwrap();
    for name in ["a.conf", "b.conf", "c.conf"] {
        let file = dir.join(name);
        fs::write(&file, "Host hidden\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    }
    h.rewrite(&format!("Include ~/.ssh/config.d/*\n\n{CONFIG}"));
    let texts: Vec<String> = h.rt.elements().into_iter().filter_map(|e| e.text).collect();
    for name in ["a.conf", "b.conf", "c.conf"] {
        assert!(texts.iter().any(|t| t.contains(name)), "a notice names {name}: {texts:?}");
    }
    assert_eq!(h.keyed("config-error-").into_iter().filter(|(k, _)| k.ends_with("-text")).count(), 3, "one notice each");
    assert!(h.rt.find("host-0-0").is_some(), "the list still shows");
}

#[test]
fn inspector_fades_on_click_but_arrows_are_instant() {
    let mut h = Harness::new();
    h.settle();
    let (x, y) = h.rt.find("host-0-1").unwrap();
    h.rt.pointer_down(x, y);
    h.rt.pointer_up(x, y);
    h.rt.frame(1.0 / 60.0);
    let shown = h.keyed("inspector-host-");
    assert!(shown.iter().any(|(_, o)| *o < 1.0), "a click fades the content in: {shown:?}");
    h.settle();
    h.rt.key(KeyEvent { key: Key::Up, mods: Mods::default() });
    h.rt.frame(1.0 / 60.0);
    assert_eq!(h.rt.state.selected_host_id(), Some((0, 0)));
    let shown = h.keyed("inspector-host-");
    assert_eq!(shown.len(), 1, "the old content is gone at once: {shown:?}");
    assert_eq!(shown[0].1, 1.0, "and the new is there at once");
}

#[test]
fn a_host_menu_left_open_doesnt_swallow_keys() {
    let mut h = Harness::new();
    h.click("host-more");
    assert!(h.rt.state.host_menu && h.rt.state.modal(), "the open menu takes the keys");
    // The host goes away on disk while its menu is open: the menu goes with it.
    h.rewrite("Host *\n  ServerAliveInterval 30\n");
    assert!(h.rt.state.selected_host_id().is_none());
    assert!(!h.rt.state.modal(), "no menu shows, so none holds the keys");
    h.rt.key(KeyEvent { key: Key::Command('2'), mods: Mods { cmd: true, ..Mods::default() } });
    h.settle();
    assert_eq!(h.rt.state.page, Page::Keys, "⌘2 still works");
}

#[test]
fn keys_page_matches_hosts_page_skeleton() {
    let mut h = Harness::new();
    h.settle();
    let (filter_hosts, _) = h.rt.presented("filter").expect("the hosts filter");
    let (name_hosts, _) = h.rt.presented("host-0-0-name").expect("the first host's name");
    h.click("nav-keys");
    let (filter_keys, _) = h.rt.presented("filter").expect("the keys filter");
    let (name_keys, _) = h.rt.presented("key-id_work-name").expect("the first key's name");
    assert_eq!(filter_keys, filter_hosts, "one filter in one place");
    assert_eq!(name_keys[0], name_hosts[0], "row names on one edge");
}

#[test]
fn agent_notice_keeps_remove_all() {
    let mut h = Harness::new();
    let _agent = crate::keys::tests::TestAgent::start(&h.paths);
    let private = h.rt.state.key("id_work").and_then(|k| k.private.clone()).unwrap();
    keys::agent_add(&h.paths, &private, "", false).unwrap();
    h.rt.state.rescan();
    h.click("nav-keys");
    let texts: Vec<String> = h.rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(texts.iter().any(|t| t == "ssh-agent holds 1 key"), "the agent's notice: {texts:?}");
    assert!(h.rt.find("agent-clear").is_some(), "Remove all stays");
    h.click("agent-clear");
    assert_eq!(keys::agent(&h.paths), keys::Agent::Running(Vec::new()));
}

#[test]
fn key_more_menu_reveals_and_writes_pub() {
    let mut h = Harness::new();
    fs::remove_file(h.ssh("id_work.pub")).unwrap();
    h.rt.state.rescan();
    h.click("nav-keys");
    h.click("key-more");
    assert!(h.rt.state.key_menu && h.rt.state.modal(), "the open menu takes the keys");
    let label = |h: &Harness, key: &str| h.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label);
    assert_eq!(label(&h, "key-more-popup-item0").as_deref(), Some("Write .pub file"));
    assert!(label(&h, "key-more-popup-item1").is_some(), "Show in Finder");
    h.click("key-more-popup-item0");
    assert!(h.ssh("id_work.pub").exists(), "the .pub is back");
    assert!(!h.rt.state.key_menu, "choosing closes the menu");
    // The key goes away on disk while its menu is open: the menu goes with it.
    h.click("key-more");
    assert!(h.rt.state.key_menu);
    fs::remove_file(h.ssh("id_work")).unwrap();
    fs::remove_file(h.ssh("id_work.pub")).unwrap();
    h.rt.state.rescan();
    h.rt.invalidate();
    h.settle();
    assert!(h.rt.state.selected_key.is_none());
    assert!(!h.rt.state.modal(), "no menu shows, so none holds the keys");
}

#[test]
fn unused_key_says_so() {
    let mut h = Harness::new();
    keys::generate(&h.paths, KeyType::Ed25519, "id_spare", "spare", "").unwrap();
    h.rt.state.rescan();
    h.click("nav-keys");
    h.click("key-id_spare");
    let texts: Vec<String> = h.rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(texts.iter().any(|t| t.contains("No host uses this key.")), "the hint row: {texts:?}");
    assert!(h.rt.find("card-used-by").is_some(), "the card stays, with the hint in it");
}

#[test]
fn key_actions_stay_inside_the_inspector() {
    let mut h = Harness::new();
    let _agent = crate::keys::tests::TestAgent::start(&h.paths);
    h.rt.state.rescan();
    h.click("nav-keys");
    let (pane, _) = h.rt.presented("inspector").expect("the inspector");
    let (trash, _) = h.rt.presented("key-trash").expect("Move to Trash");
    for key in ["key-agent-add", "key-passphrase", "key-comment", "key-more", "key-trash"] {
        let (r, _) = h.rt.presented(key).unwrap_or_else(|| panic!("{key} is shown"));
        assert!(r[0] >= pane[0] && r[0] + r[2] <= pane[0] + pane[2], "{key} at {r:?} is inside the inspector {pane:?}");
    }
    let (first, _) = h.rt.presented("key-agent-add").unwrap();
    assert_eq!(trash[1], first[1], "Move to Trash stays on the first line");
}
