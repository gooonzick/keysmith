//! Hosts: selecting, the Add/Edit form, removing, reordering, `ssh -G` and a connection test.
//! Every write reads the file fresh, changes only its lines, keeps a backup and can be undone.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rune::TextEdit;

use super::{App, Page, Undo};
use crate::fsops;
use crate::sshconfig::{Document, HostSpec, Kind, Section, valid_key};

/// A section by file and patterns, so a selection survives rescans that renumber lines.
#[derive(Clone, Debug, PartialEq)]
pub struct HostRef {
    pub path: PathBuf,
    pub patterns: String,
    /// Which of the sections with these patterns in that file.
    pub nth: usize,
}

/// `ssh -G` for one alias: `None` while it runs.
pub struct Effective {
    pub alias: String,
    pub result: Option<Result<Vec<(String, String)>, String>>,
}

pub struct Pair {
    pub key: TextEdit,
    pub value: TextEdit,
}

impl Pair {
    pub fn new(key: &str, value: &str) -> Pair {
        Pair { key: TextEdit::new(key), value: TextEdit::new(value) }
    }
}

pub struct HostForm {
    pub editing: Option<HostRef>,
    /// The file it goes in.
    pub file: PathBuf,
    pub kind: Kind,
    pub patterns: TextEdit,
    pub description: TextEdit,
    pub hostname: TextEdit,
    pub user: TextEdit,
    pub port: TextEdit,
    pub identity: TextEdit,
    pub proxy: TextEdit,
    /// Every other option, repeats of the fields above included.
    pub extra: Vec<Pair>,
    pub identity_menu: bool,
    pub file_menu: bool,
    pub error: Option<String>,
}

/// The fields the form has a box for, in the order a new host writes them.
const MAIN: [&str; 5] = ["HostName", "User", "Port", "IdentityFile", "ProxyJump"];

pub enum Confirm {
    RemoveHost(HostRef),
    TrashKey(String),
}

impl HostForm {
    fn main_field(&mut self, i: usize) -> &mut TextEdit {
        match i {
            0 => &mut self.hostname,
            1 => &mut self.user,
            2 => &mut self.port,
            3 => &mut self.identity,
            _ => &mut self.proxy,
        }
    }

    fn blank(file: PathBuf) -> HostForm {
        HostForm {
            editing: None,
            file,
            kind: Kind::Host,
            patterns: TextEdit::new(""),
            description: TextEdit::new(""),
            hostname: TextEdit::new(""),
            user: TextEdit::new(""),
            port: TextEdit::new(""),
            identity: TextEdit::new(""),
            proxy: TextEdit::new(""),
            extra: Vec::new(),
            identity_menu: false,
            file_menu: false,
            error: None,
        }
    }

    fn filled(file: PathBuf, s: &Section) -> HostForm {
        let mut f = HostForm::blank(file);
        f.kind = s.kind;
        f.patterns = TextEdit::new(&s.patterns);
        f.description = TextEdit::new(s.doc.join("\n"));
        let mut taken = [false; MAIN.len()];
        for o in &s.options {
            match MAIN.iter().position(|m| m.eq_ignore_ascii_case(&o.key)) {
                Some(i) if !taken[i] => {
                    taken[i] = true;
                    *f.main_field(i) = TextEdit::new(&o.value);
                }
                _ => f.extra.push(Pair::new(&o.key, &o.value)),
            }
        }
        f
    }

    /// The form as a section, or what's wrong with it. Keys keep the case the file had.
    fn spec(&mut self, original: Option<&Section>) -> Result<HostSpec, String> {
        let patterns = self.patterns.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if patterns.is_empty() {
            return Err(if self.kind == Kind::Match { "Write the Match criteria".into() } else { "Give the host a name".into() });
        }
        let port = self.port.text.trim();
        if !port.is_empty() && port.parse::<u16>().map_or(true, |p| p == 0) {
            return Err("The port is a number from 1 to 65535".into());
        }
        let key_case = |k: &str| original.and_then(|s| s.options.iter().find(|o| o.key.eq_ignore_ascii_case(k)).map(|o| o.key.clone())).unwrap_or_else(|| k.to_string());
        let mut options = Vec::new();
        // The fields keep their place among the options the section already had.
        let mut mains: Vec<(String, String)> = Vec::new();
        for (i, m) in MAIN.iter().enumerate() {
            let v = self.main_field(i).text.trim().to_string();
            if !v.is_empty() {
                mains.push((key_case(m), v));
            }
        }
        for p in &self.extra {
            let (k, v) = (p.key.text.trim(), p.value.text.trim());
            if k.is_empty() && v.is_empty() {
                continue;
            }
            if !valid_key(k) {
                return Err(format!("“{k}” isn't an option name: letters and digits only"));
            }
            if k.eq_ignore_ascii_case("host") || k.eq_ignore_ascii_case("match") {
                return Err(format!("{k} starts a new section; add it as its own host"));
            }
            if v.is_empty() {
                return Err(format!("{k} needs a value"));
            }
            options.push((k.to_string(), v.to_string()));
        }
        // A main field goes before any repeat of its key, so it stays the first one.
        for (k, v) in mains.into_iter().rev() {
            let at = options.iter().position(|(o, _)| o.eq_ignore_ascii_case(&k)).unwrap_or(0);
            options.insert(at, (k, v));
        }
        // New hosts list the fields in the usual order, then the rest.
        if original.is_none() {
            options.sort_by_key(|(k, _)| MAIN.iter().position(|m| m.eq_ignore_ascii_case(k)).unwrap_or(MAIN.len()));
        }
        let doc = if self.description.text.trim().is_empty() { Vec::new() } else { self.description.text.lines().map(|l| l.trim_end().to_string()).collect() };
        Ok(HostSpec { kind: Some(self.kind), patterns, doc, options })
    }
}

impl App {
    pub fn host_ref(&self, (d, s): (usize, usize)) -> HostRef {
        let doc = &self.docs[d];
        let sec = &doc.sections[s];
        let nth = doc.sections[..s].iter().filter(|x| x.patterns == sec.patterns).count();
        HostRef { path: doc.path.clone(), patterns: sec.patterns.clone(), nth }
    }

    pub fn resolve(&self, r: &HostRef) -> Option<(usize, usize)> {
        let d = self.docs.iter().position(|d| d.path == r.path)?;
        let s = find_section(&self.docs[d], r)?;
        Some((d, s))
    }

    pub fn selected_host_id(&self) -> Option<(usize, usize)> {
        self.selected_host.as_ref().and_then(|r| self.resolve(r))
    }

    pub fn select_host(&mut self, id: (usize, usize)) {
        let r = self.host_ref(id);
        if self.selected_host.as_ref() != Some(&r) {
            self.selected_host = Some(r);
            self.host_menu = false;
            self.show_all_effective = false;
            self.refresh_effective();
        }
        if !self.inspector.is_open() {
            self.inspector.toggle();
        }
    }

    /// Asks `ssh -G` what the selected host resolves to, in the background.
    pub fn refresh_effective(&mut self) {
        let alias = self.selected_host_id().and_then(|id| self.section(id).alias().map(str::to_string));
        let Some(alias) = alias else {
            self.effective_for = None;
            self.effective.cancel();
            return;
        };
        self.effective_for = Some(Effective { alias: alias.clone(), result: None });
        let config = self.paths.config.clone();
        self.effective.request(alias, move |alias| {
            let r = effective(&config, &alias);
            (alias, r)
        });
    }

    pub fn open_add_host(&mut self) {
        self.open_page(Page::Hosts);
        let file = self.file_filter.clone().unwrap_or_else(|| self.paths.config.clone());
        let mut f = HostForm::blank(file);
        f.patterns.focused = true;
        self.host_form = Some(f);
    }

    pub fn open_edit_host(&mut self, id: (usize, usize)) {
        let mut f = HostForm::filled(self.docs[id.0].path.clone(), self.section(id));
        f.editing = Some(self.host_ref(id));
        self.host_form = Some(f);
    }

    pub fn open_duplicate_host(&mut self, id: (usize, usize)) {
        let mut f = HostForm::filled(self.docs[id.0].path.clone(), self.section(id));
        let name = self.section(id).alias().unwrap_or("host").to_string();
        f.patterns = TextEdit::new(format!("{name}-copy"));
        f.patterns.focused = true;
        self.host_form = Some(f);
    }

    pub fn close_host_form(&mut self) {
        self.host_form = None;
    }

    /// Fills the identity box with a key's path, as the config should name it.
    pub fn pick_identity(&mut self, key_name: &str) {
        let value = self.key(key_name).and_then(|k| k.private.clone().or(k.public.clone())).map(|p| self.paths.config_path(&p)).unwrap_or_default();
        if let Some(f) = &mut self.host_form {
            f.identity = TextEdit::new(&value);
            f.identity_menu = false;
        }
    }

    pub fn add_host_pair(&mut self) {
        if let Some(f) = &mut self.host_form {
            let mut p = Pair::new("", "");
            p.key.focused = true;
            f.extra.push(p);
        }
    }

    pub fn remove_host_pair(&mut self, i: usize) {
        if let Some(f) = self.host_form.as_mut().filter(|f| i < f.extra.len()) {
            f.extra.remove(i);
        }
    }

    pub fn save_host_form(&mut self) {
        let Some(mut form) = self.host_form.take() else { return };
        match self.apply_form(&mut form) {
            Ok(r) => {
                let what = if form.editing.is_some() { "Saved" } else { "Added" };
                let title = format!("{what} {}", r.patterns);
                self.rescan();
                self.selected_host = Some(r);
                self.refresh_effective();
                self.notify_change(title);
            }
            Err(e) => {
                form.error = Some(e);
                self.host_form = Some(form);
            }
        }
    }

    fn apply_form(&mut self, form: &mut HostForm) -> Result<HostRef, String> {
        let mut doc = Document::read(&form.file).map_err(|e| format!("Couldn't read {}: {e}", self.paths.tilde(&form.file)))?;
        let at = match &form.editing {
            Some(r) => Some(find_section(&doc, r).ok_or("The host changed on disk; close this and try again")?),
            None => None,
        };
        let spec = form.spec(at.map(|i| &doc.sections[i]))?;
        // A name ssh would reach through another section first.
        for alias in spec.patterns.split_whitespace().filter(|p| !p.contains(['*', '?', '!'])) {
            for (d, other) in self.docs.iter().enumerate() {
                for (s, sec) in other.sections.iter().enumerate() {
                    let same = form.editing.as_ref().is_some_and(|r| self.resolve(r) == Some((d, s)));
                    if !same && spec.kind == Some(Kind::Host) && sec.kind == Kind::Host && sec.patterns.split_whitespace().any(|p| p == alias) {
                        return Err(format!("{alias} is already a host in {}", self.paths.tilde(&other.path)));
                    }
                }
            }
        }
        let before = std::fs::read_to_string(&form.file).ok();
        let index = match at {
            Some(i) => {
                doc.replace(i, &spec);
                i
            }
            None => doc.add(&spec),
        };
        fsops::write_with_backup(&form.file, &doc.text()).map_err(|e| format!("Couldn't save {}: {e}", self.paths.tilde(&form.file)))?;
        self.pending_undo = Some(Undo::Files(vec![(form.file.clone(), before)]));
        let sec = &doc.sections[index];
        let nth = doc.sections[..index].iter().filter(|x| x.patterns == sec.patterns).count();
        Ok(HostRef { path: form.file.clone(), patterns: sec.patterns.clone(), nth })
    }

    /// Reports a written change, with Undo.
    fn notify_change(&mut self, title: String) {
        if let Some(u) = self.pending_undo.take() {
            self.changed(title, u);
        }
    }

    pub fn confirm_remove_host(&mut self, id: (usize, usize)) {
        self.confirm = Some(Confirm::RemoveHost(self.host_ref(id)));
    }

    pub fn remove_host(&mut self, r: &HostRef) {
        self.confirm = None;
        let result = self.edit_file(&r.path, |doc| {
            let i = find_section(doc, r).ok_or("The host changed on disk")?;
            doc.remove(i);
            Ok(())
        });
        match result {
            Ok(()) => {
                if self.selected_host.as_ref() == Some(r) {
                    self.selected_host = None;
                }
                self.rescan();
                self.refresh_effective();
                self.notify_change(format!("Removed {}", r.patterns));
            }
            Err(e) => self.notify("Couldn't remove the host", e, true),
        }
    }

    /// Moves a host up or down within its file. ssh uses the first value it finds for an
    /// option, so order decides which section wins.
    pub fn move_host(&mut self, id: (usize, usize), delta: i32) {
        let to = id.1 as i32 + delta;
        if to < 0 || to as usize >= self.docs[id.0].sections.len() {
            return;
        }
        let r = self.host_ref(id);
        let other = self.host_ref((id.0, to as usize));
        let result = self.edit_file(&r.path, |doc| {
            let (a, b) = (find_section(doc, &r), find_section(doc, &other));
            let (Some(a), Some(b)) = (a, b) else { return Err("The file changed on disk".into()) };
            doc.swap(a, b);
            Ok(())
        });
        match result {
            Ok(()) => {
                self.rescan();
                // Same patterns, same file; its index among equal names is unchanged by a swap
                // with a different name.
                self.selected_host = Some(r.clone());
                self.notify_change(format!("Moved {}", r.patterns));
            }
            Err(e) => self.notify("Couldn't move the host", e, true),
        }
    }

    /// Reads `path`, lets `f` change it, writes it back with a backup and keeps the old text
    /// for Undo.
    fn edit_file(&mut self, path: &Path, f: impl FnOnce(&mut Document) -> Result<(), String>) -> Result<(), String> {
        let before = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut doc = Document::parse(path.to_path_buf(), &before);
        f(&mut doc)?;
        fsops::write_with_backup(path, &doc.text()).map_err(|e| e.to_string())?;
        self.pending_undo = Some(Undo::Files(vec![(path.to_path_buf(), Some(before))]));
        Ok(())
    }

    /// Connects with ssh in batch mode (no prompts) and reports what happened.
    pub fn test_connection(&mut self, alias: String) {
        if self.testing.is_some() {
            return;
        }
        self.testing = Some(alias.clone());
        self.tested = None;
        let (config, sock) = (self.paths.config.clone(), self.paths.agent_sock.clone());
        self.conn_test.request(alias, move |alias| {
            let r = test_connection(&config, sock.as_deref(), &alias);
            (alias, r)
        });
    }

    /// Opens Terminal running `ssh alias` (macOS).
    pub fn open_in_terminal(&mut self, alias: &str) {
        if !alias.chars().all(|c| c.is_ascii_alphanumeric() || "._-@".contains(c)) {
            self.notify("Can't open that name in Terminal", "Use Copy command instead", true);
            return;
        }
        let script = format!("tell application \"Terminal\"\n activate\n do script \"ssh {alias}\"\nend tell");
        if let Err(e) = Command::new("osascript").arg("-e").arg(script).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            self.notify("Couldn't open Terminal", e.to_string(), true);
        }
    }

    pub fn set_file_filter(&mut self, path: Option<PathBuf>) {
        self.open_page(Page::Hosts);
        self.file_filter = path;
    }
}

fn find_section(doc: &Document, r: &HostRef) -> Option<usize> {
    doc.sections.iter().enumerate().filter(|(_, s)| s.patterns == r.patterns).nth(r.nth).map(|(i, _)| i)
}

/// `ssh -G alias`: every option as ssh resolves it for that host.
pub fn effective(config: &Path, alias: &str) -> Result<Vec<(String, String)>, String> {
    let mut cmd = Command::new("ssh");
    if config.is_file() {
        cmd.arg("-F").arg(config);
    } else {
        cmd.args(["-F", "/dev/null"]);
    }
    let out = cmd.arg("-G").arg("--").arg(alias).stdin(Stdio::null()).output().map_err(|e| format!("Couldn't run ssh: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_once(' ').map(|(k, v)| (k.to_string(), v.to_string()))).collect())
}

fn test_connection(config: &Path, sock: Option<&Path>, alias: &str) -> Result<String, String> {
    let mut cmd = Command::new("ssh");
    if config.is_file() {
        cmd.arg("-F").arg(config);
    }
    if let Some(s) = sock {
        cmd.env("SSH_AUTH_SOCK", s);
    }
    let out = cmd
        .args(["-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "--"])
        .arg(alias)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Couldn't run ssh: {e}"))?;
    let said = format!("{}\n{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    let lines: Vec<&str> = said.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("Warning: Permanently added")).collect();
    // Git hosts authenticate, then refuse a shell.
    let greeted = lines.iter().find(|l| l.contains("successfully authenticated") || l.starts_with("Welcome to") || l.starts_with("Hi "));
    if out.status.success() || greeted.is_some() {
        Ok(greeted.map_or_else(|| "Connected and logged in".to_string(), |l| l.to_string()))
    } else {
        Err(lines.last().map_or_else(|| format!("ssh exited with {}", out.status), |l| l.to_string()))
    }
}
