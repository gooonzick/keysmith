//! Keys: making one, the agent, passphrases and comments, permissions, import and the trash.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use rune::TextEdit;

use super::{App, Confirm, Page, Undo};
use crate::keys::{self as keyops, KeyType};

pub struct NewKeyForm {
    pub ty: KeyType,
    pub type_menu: bool,
    pub name: TextEdit,
    pub comment: TextEdit,
    pub pass: TextEdit,
    pub confirm: TextEdit,
    pub shown: bool,
    pub add_to_agent: bool,
    /// macOS: keep the passphrase in the login keychain.
    pub keychain: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassPurpose {
    /// Load into the agent: asks for the passphrase.
    Unlock,
    Change,
    Comment,
}

pub struct PassForm {
    pub key: String,
    pub purpose: PassPurpose,
    pub old: TextEdit,
    pub new: TextEdit,
    pub confirm: TextEdit,
    pub comment: TextEdit,
    pub shown: bool,
    pub keychain: bool,
    pub error: Option<String>,
}

fn secret() -> TextEdit {
    TextEdit::new("").secure()
}

impl App {
    /// The first of `id_ed25519`, `id_ed25519_2`… that is free.
    fn free_name(&self, ty: KeyType) -> String {
        let base = ty.file();
        (1..).map(|n| if n == 1 { base.to_string() } else { format!("{base}_{n}") }).find(|n| keyops::check_name(&self.paths, n).is_ok()).expect("some name is free")
    }

    pub fn open_new_key(&mut self) {
        self.open_page(Page::Keys);
        let ty = KeyType::Ed25519;
        let mut name = TextEdit::new(self.free_name(ty));
        name.focused = true;
        self.new_key = Some(NewKeyForm {
            ty,
            type_menu: false,
            name,
            comment: TextEdit::new(&self.paths.user_at_host),
            pass: secret(),
            confirm: secret(),
            shown: false,
            add_to_agent: true,
            keychain: cfg!(target_os = "macos"),
            error: None,
        });
    }

    pub fn set_new_key_type(&mut self, ty: KeyType) {
        let free = self.free_name(ty);
        let defaults: Vec<String> = KeyType::ALL.iter().map(|t| self.free_name(*t)).collect();
        let Some(f) = &mut self.new_key else { return };
        // A name the user typed stays; a suggested one follows the type.
        if defaults.contains(&f.name.text) {
            f.name = TextEdit::new(&free);
        }
        f.ty = ty;
        f.type_menu = false;
    }

    pub fn save_new_key(&mut self) {
        let Some(mut f) = self.new_key.take() else { return };
        if f.pass.text != f.confirm.text {
            f.error = Some("The passphrases don't match".into());
            self.new_key = Some(f);
            return;
        }
        let path = match keyops::generate(&self.paths, f.ty, &f.name.text, &f.comment.text, &f.pass.text) {
            Ok(p) => p,
            Err(e) => {
                f.error = Some(e);
                self.new_key = Some(f);
                return;
            }
        };
        let name = f.name.text.trim().to_string();
        let mut detail = String::new();
        if f.add_to_agent
            && matches!(self.agent, keyops::Agent::Running(_))
            && let Err(e) = keyops::agent_add(&self.paths, &path, &f.pass.text, f.keychain && !f.pass.text.is_empty())
        {
            detail = format!("Not added to the agent: {e}");
        }
        self.rescan();
        self.selected_key = Some(name.clone());
        let public = path.with_file_name(format!("{name}.pub"));
        self.undo = Some((format!("Created {name}"), Undo::Files(vec![(path, None), (public, None)])));
        self.notice = Some(super::Notice { title: format!("Created {name}"), detail, error: false, undo: true });
        self.notice_at = None;
    }

    pub fn add_to_agent(&mut self, name: &str) {
        let Some(key) = self.key(name).cloned() else { return };
        let Some(private) = key.private.clone() else { return };
        if key.encrypted == Some(false) {
            match keyops::agent_add(&self.paths, &private, "", false) {
                Ok(()) => self.notify(format!("Added {name} to the agent"), "", false),
                Err(e) => self.notify("Couldn't add the key", e, true),
            }
            self.agent = keyops::agent(&self.paths);
            return;
        }
        self.open_pass_form(name, PassPurpose::Unlock);
    }

    pub fn remove_from_agent(&mut self, name: &str) {
        let Some(key) = self.key(name).cloned() else { return };
        match keyops::agent_remove(&self.paths, &key) {
            Ok(()) => self.notify(format!("Removed {name} from the agent"), "", false),
            Err(e) => self.notify("Couldn't remove the key from the agent", e, true),
        }
        self.agent = keyops::agent(&self.paths);
    }

    pub fn clear_agent(&mut self) {
        match keyops::agent_clear(&self.paths) {
            Ok(()) => self.notify("Removed every key from the agent", "", false),
            Err(e) => self.notify("Couldn't clear the agent", e, true),
        }
        self.agent = keyops::agent(&self.paths);
    }

    pub fn open_pass_form(&mut self, name: &str, purpose: PassPurpose) {
        let Some(key) = self.key(name) else { return };
        let mut old = secret();
        let mut comment = TextEdit::new(&key.comment);
        let needs_old = key.encrypted != Some(false);
        if purpose == PassPurpose::Comment {
            comment.focused = true;
        } else if needs_old || purpose == PassPurpose::Unlock {
            old.focused = true;
        }
        let mut new = secret();
        if purpose == PassPurpose::Change && !needs_old {
            new.focused = true;
        }
        self.pass_form = Some(PassForm { key: name.to_string(), purpose, old, new, confirm: secret(), comment, shown: false, keychain: cfg!(target_os = "macos"), error: None });
    }

    pub fn close_pass_form(&mut self) {
        self.pass_form = None;
    }

    pub fn save_pass_form(&mut self) {
        let Some(mut f) = self.pass_form.take() else { return };
        let Some(key) = self.key(&f.key).cloned() else { return };
        let Some(private) = key.private.clone() else { return };
        let result = match f.purpose {
            PassPurpose::Unlock => keyops::agent_add(&self.paths, &private, &f.old.text, f.keychain).map(|()| format!("Added {} to the agent", key.name)),
            PassPurpose::Change if f.new.text != f.confirm.text => Err("The new passphrases don't match".into()),
            PassPurpose::Change => keyops::change_passphrase(&private, &f.old.text, &f.new.text).map(|()| {
                if f.new.text.is_empty() { format!("Removed the passphrase from {}", key.name) } else { format!("Changed the passphrase of {}", key.name) }
            }),
            PassPurpose::Comment => keyops::change_comment(&private, &f.old.text, &f.comment.text).map(|()| format!("Changed the comment of {}", key.name)),
        };
        match result {
            Ok(title) => {
                self.rescan();
                self.notify(title, "", false);
            }
            Err(e) => {
                f.error = Some(e);
                self.pass_form = Some(f);
            }
        }
    }

    pub fn write_public(&mut self, name: &str) {
        let Some(key) = self.key(name).cloned() else { return };
        match keyops::write_public(&key) {
            Ok(p) => {
                self.rescan();
                self.notify(format!("Wrote {}", self.paths.tilde(&p)), "", false);
            }
            Err(e) => self.notify("Couldn't write the public key", e, true),
        }
    }

    pub fn fix_permissions(&mut self, name: &str) {
        let Some(key) = self.key(name).cloned() else { return };
        match keyops::fix_permissions(&self.paths, &key) {
            Ok(()) => {
                self.rescan();
                self.notify(format!("{name} is private again"), "Only you can read it now (600)", false);
            }
            Err(e) => self.notify("Couldn't change the permissions", e, true),
        }
    }

    pub fn reveal(&mut self, name: &str) {
        let Some(path) = self.key(name).map(|k| k.path().to_path_buf()) else { return };
        let mut cmd = if cfg!(target_os = "macos") {
            let mut c = Command::new("open");
            c.arg("-R").arg(&path);
            c
        } else {
            let mut c = Command::new("xdg-open");
            c.arg(path.parent().unwrap_or(&path));
            c
        };
        if let Err(e) = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            self.notify("Couldn't show the file", e.to_string(), true);
        }
    }

    pub fn confirm_trash_key(&mut self, name: &str) {
        self.confirm = Some(Confirm::TrashKey(name.to_string()));
    }

    pub fn trash_key(&mut self, name: &str) {
        self.confirm = None;
        let Some(key) = self.key(name).cloned() else { return };
        // Out of the agent first: once the file is gone, ssh-add can't name it.
        if self.in_agent(&key) {
            let _ = keyops::agent_remove(&self.paths, &key);
        }
        match keyops::trash(&self.paths, &key) {
            Ok(moved) => {
                self.rescan();
                self.changed(format!("Moved {name} to the Trash"), Undo::Trashed(moved));
            }
            Err(e) => {
                self.rescan();
                self.notify("Couldn't move the key to the Trash", e, true);
            }
        }
    }

    pub fn import_key(&mut self, from: PathBuf) {
        match keyops::import(&self.paths, &from) {
            Ok(to) => {
                let name = to.file_name().unwrap_or_default().to_string_lossy().into_owned();
                self.rescan();
                self.selected_key = Some(name.clone());
                self.notify(format!("Imported {name}"), format!("Copied to {}", self.paths.tilde(&to)), false);
            }
            Err(e) => self.notify("Couldn't import the key", e, true),
        }
    }

    pub fn select_key(&mut self, name: &str) {
        if self.selected_key.as_deref() != Some(name) {
            self.selected_key = Some(name.to_string());
            self.key_menu = false;
        }
        if !self.inspector.is_open() {
            self.inspector.toggle();
        }
    }

    /// From a host's identity to its key.
    pub fn show_key(&mut self, name: &str) {
        self.open_page(Page::Keys);
        self.select_key(name);
    }

    /// From a key to a host that uses it.
    pub fn show_host(&mut self, id: (usize, usize)) {
        self.open_page(Page::Hosts);
        self.file_filter = None;
        self.select_host(id);
    }
}
