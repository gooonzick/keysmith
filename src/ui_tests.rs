//! The window driven headlessly: clicks on real elements against a throwaway home folder.

use std::fs;
use std::path::PathBuf;

use rune::{Env, Runtime, TextEdit};

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
    h.click("host-down");
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
    h.click("host-copy");
    assert!(h.rt.state.copied_at.is_some(), "the button shows the check");
    for _ in 0..90 {
        h.rt.frame(1.0 / 60.0);
    }
    assert!(h.rt.state.copied_at.is_none(), "the check goes after a second or so");
}
