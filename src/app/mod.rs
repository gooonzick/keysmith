//! The app's state and what its controls do. The view (`ui`) only reads this; `hosts` and
//! `keys` own their pages' changes.

mod hosts;
mod keys;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use notify::{RecursiveMode, Watcher};
use rune::{Latest, TextEdit, Waker};
use rune_kit::shell::PaneState;

use crate::fsops;
use crate::keys::{self as keyops, Agent, Key};
use crate::paths::Paths;
use crate::sshconfig::{self, Document, Section};

pub use hosts::{Confirm, Effective, HostForm, HostRef};
pub use keys::{NewKeyForm, PassForm, PassPurpose};

/// `ssh -G`'s options for one alias, or why it failed.
pub type EffectiveResult = Result<Vec<(String, String)>, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Hosts,
    Keys,
}

pub struct Notice {
    pub title: String,
    pub detail: String,
    pub error: bool,
    /// Offers to undo the change it reports.
    pub undo: bool,
}

/// What Undo puts back.
pub enum Undo {
    /// Files as they were before: their text, or `None` where there was no file.
    Files(Vec<(PathBuf, Option<String>)>),
    /// Files moved to the trash: (where they are now, where they were).
    Trashed(Vec<(PathBuf, PathBuf)>),
}

pub struct App {
    pub paths: Paths,
    pub sidebar: PaneState,
    pub inspector: PaneState,
    pub page: Page,
    pub filter: TextEdit,
    /// Show the design system's gallery instead of the app (`--scene ds`).
    pub gallery: bool,

    // ~/.ssh/config and what it includes.
    pub docs: Vec<Document>,
    pub config_errors: Vec<(PathBuf, String)>,
    /// Only hosts from this file, by path.
    pub file_filter: Option<PathBuf>,
    pub selected_host: Option<HostRef>,
    pub host_form: Option<HostForm>,
    /// `ssh -G` for the selected host.
    pub effective: Latest<(String, EffectiveResult)>,
    pub effective_for: Option<Effective>,
    pub show_all_effective: bool,
    /// The selected host's "…" menu (Move up, Move down) is open.
    pub host_menu: bool,
    /// A connection test: the alias it ran for and what ssh said.
    pub conn_test: Latest<(String, Result<String, String>)>,
    pub testing: Option<String>,
    pub tested: Option<(String, Result<String, String>)>,

    // Keys.
    pub keys: Vec<Key>,
    pub key_errors: Vec<String>,
    pub agent: Agent,
    pub selected_key: Option<String>,
    pub new_key: Option<NewKeyForm>,
    pub pass_form: Option<PassForm>,

    pub confirm: Option<Confirm>,
    pub notice: Option<Notice>,
    /// When the notice went up, on the app clock.
    pub notice_at: Option<f64>,
    pub undo: Option<(String, Undo)>,
    /// What was just copied and when, on the app clock: its Copy button shows a check until
    /// `poll` clears it. The time is NaN until `poll` stamps it.
    pub copied_at: Option<(String, f64)>,
    /// What the change being made can be undone with, until it is reported.
    pending_undo: Option<Undo>,

    waker: Option<Waker>,
    watcher: Option<notify::RecommendedWatcher>,
    dirty: Arc<AtomicBool>,
}

impl App {
    pub fn new(paths: Paths) -> App {
        let mut app = App {
            sidebar: PaneState::new(232.0).range(180.0, 380.0),
            inspector: PaneState::new(400.0).range(320.0, 640.0),
            page: Page::Hosts,
            gallery: false,
            filter: TextEdit::new(""),
            docs: Vec::new(),
            config_errors: Vec::new(),
            file_filter: None,
            selected_host: None,
            host_form: None,
            effective: Latest::default(),
            effective_for: None,
            show_all_effective: false,
            host_menu: false,
            conn_test: Latest::default(),
            testing: None,
            tested: None,
            keys: Vec::new(),
            key_errors: Vec::new(),
            agent: Agent::Running(Vec::new()),
            selected_key: None,
            new_key: None,
            pass_form: None,
            confirm: None,
            notice: None,
            notice_at: None,
            undo: None,
            copied_at: None,
            pending_undo: None,
            waker: None,
            watcher: None,
            dirty: Arc::new(AtomicBool::new(false)),
            paths,
        };
        app.rescan();
        app.select_first();
        app
    }

    pub fn set_waker(&mut self, waker: Waker) {
        self.effective.set_waker(waker.clone());
        self.conn_test.set_waker(waker.clone());
        self.waker = Some(waker);
        self.watch();
        self.refresh_effective();
    }

    /// Reads the config, the keys and the agent again. Selections that still exist stay.
    pub fn rescan(&mut self) {
        let (docs, errors) = sshconfig::load(&self.paths);
        self.docs = docs;
        self.config_errors = errors;
        let (keys, key_errors) = keyops::scan(&self.paths);
        self.keys = keys;
        self.key_errors = key_errors;
        self.agent = keyops::agent(&self.paths);
        if self.selected_host.as_ref().is_some_and(|r| self.resolve(r).is_none()) {
            self.selected_host = None;
            self.host_menu = false;
        }
        if self.selected_key.as_ref().is_some_and(|n| self.key(n).is_none()) {
            self.selected_key = None;
        }
        if self.file_filter.as_ref().is_some_and(|f| !self.docs.iter().any(|d| &d.path == f)) {
            self.file_filter = None;
        }
    }

    fn select_first(&mut self) {
        if let Some(r) = self.visible_hosts().first() {
            self.selected_host = Some(self.host_ref(*r));
        }
        self.selected_key = self.keys.first().map(|k| k.name.clone());
    }

    /// Runs after the waker fires: a watched file changed, or background work finished.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if self.dirty.swap(false, Ordering::AcqRel) {
            self.watch();
            self.rescan();
            self.refresh_effective();
            changed = true;
        }
        if let Some((alias, r)) = self.effective.take() {
            if self.effective_for.as_ref().is_some_and(|e| e.alias == alias) {
                self.effective_for = Some(Effective { alias, result: Some(r) });
            }
            changed = true;
        }
        if let Some((alias, r)) = self.conn_test.take() {
            if self.testing.as_deref() == Some(alias.as_str()) {
                self.testing = None;
            }
            self.tested = Some((alias, r));
            changed = true;
        }
        changed
    }

    /// Watches `~/.ssh` and every folder an included file is in, so edits from a terminal or
    /// an editor show up on their own.
    fn watch(&mut self) {
        self.watcher = None;
        let Some(waker) = self.waker.clone() else { return };
        let dirty = self.dirty.clone();
        let Ok(mut watcher) = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|e| !e.kind.is_access()) && !dirty.swap(true, Ordering::AcqRel) {
                waker();
            }
        }) else {
            return;
        };
        let mut dirs: Vec<PathBuf> = vec![self.paths.ssh.clone()];
        for d in &self.docs {
            if let Some(p) = fsops::real_target(&d.path).parent() {
                dirs.push(p.to_path_buf());
            }
        }
        dirs.sort();
        dirs.dedup();
        for d in dirs.iter().filter(|d| d.is_dir() && **d != self.paths.home) {
            let _ = watcher.watch(d, RecursiveMode::NonRecursive);
        }
        self.watcher = Some(watcher);
    }

    pub fn open_page(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.filter = TextEdit::new("");
        }
    }

    pub fn notify(&mut self, title: impl Into<String>, detail: impl Into<String>, error: bool) {
        self.notice = Some(Notice { title: title.into(), detail: detail.into(), error, undo: false });
        self.notice_at = None;
    }

    /// Confirms a copy of `what` on its button.
    pub fn copied(&mut self, what: impl Into<String>) {
        self.copied_at = Some((what.into(), f64::NAN));
    }

    /// Whether `what` was just copied.
    pub fn just_copied(&self, what: &str) -> bool {
        self.copied_at.as_ref().is_some_and(|(w, _)| w == what)
    }

    /// Reports a change that Undo can take back.
    fn changed(&mut self, title: String, undo: Undo) {
        self.undo = Some((title.clone(), undo));
        self.notice = Some(Notice { title, detail: String::new(), error: false, undo: true });
        self.notice_at = None;
    }

    pub fn undo(&mut self) {
        let Some((title, undo)) = self.undo.take() else { return };
        let result: Result<(), String> = match undo {
            Undo::Files(files) => files.iter().try_for_each(|(path, text)| match text {
                Some(t) => fsops::write_atomic(path, t).map_err(|e| e.to_string()),
                None => std::fs::remove_file(path).map_err(|e| e.to_string()),
            }),
            Undo::Trashed(moves) => moves.iter().try_for_each(|(now, was)| {
                if was.exists() {
                    return Err(format!("{} is there again", self.paths.tilde(was)));
                }
                std::fs::rename(now, was).map_err(|e| e.to_string())
            }),
        };
        self.rescan();
        self.refresh_effective();
        match result {
            Ok(()) => self.notify(format!("Undone: {title}"), "", false),
            Err(e) => self.notify("Couldn't undo", e, true),
        }
    }

    // Lookups the view uses.

    pub fn key(&self, name: &str) -> Option<&Key> {
        self.keys.iter().find(|k| k.name == name)
    }

    pub fn section(&self, (d, s): (usize, usize)) -> &Section {
        &self.docs[d].sections[s]
    }

    /// Whether the agent holds `key`.
    pub fn in_agent(&self, key: &Key) -> bool {
        match (&self.agent, &key.fingerprint) {
            (Agent::Running(loaded), Some(fp)) => loaded.iter().any(|(f, _)| f == fp),
            _ => false,
        }
    }

    /// The key a config path names, if it is one of the scanned keys.
    pub fn key_at(&self, value: &str) -> Option<&Key> {
        let path = self.paths.expand(value);
        self.keys.iter().find(|k| k.private.as_deref() == Some(path.as_path()) || k.public.as_deref() == Some(path.as_path()))
    }

    /// The hosts (aliases, or patterns) whose `IdentityFile` is `key`.
    pub fn hosts_using(&self, key: &Key) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (d, doc) in self.docs.iter().enumerate() {
            for (s, sec) in doc.sections.iter().enumerate() {
                if sec.all("IdentityFile").iter().any(|v| self.key_at(v).is_some_and(|k| k.name == key.name)) {
                    out.push((d, s));
                }
            }
        }
        out
    }

    /// The text filter's words, all of which must appear.
    fn matches(&self, haystack: &str) -> bool {
        let hay = haystack.to_lowercase();
        self.filter.text.to_lowercase().split_whitespace().all(|w| hay.contains(w))
    }

    pub fn visible_hosts(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (d, doc) in self.docs.iter().enumerate() {
            if self.file_filter.as_ref().is_some_and(|f| f != &doc.path) {
                continue;
            }
            for (s, sec) in doc.sections.iter().enumerate() {
                let words = format!("{} {} {}", sec.patterns, sec.doc.join(" "), sec.options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>().join(" "));
                if self.matches(&words) {
                    out.push((d, s));
                }
            }
        }
        out
    }

    pub fn visible_keys(&self) -> Vec<usize> {
        (0..self.keys.len())
            .filter(|&i| {
                let k = &self.keys[i];
                self.matches(&format!("{} {} {} {}", k.name, k.comment, k.kind, k.fingerprint.as_deref().unwrap_or("")))
            })
            .collect()
    }

    pub fn host_count(&self) -> usize {
        self.docs.iter().map(|d| d.sections.len()).sum()
    }

    /// Moves the selection in the open page's list by `delta`; returns the new row's key.
    pub fn step_selection(&mut self, delta: i32) -> Option<String> {
        match self.page {
            Page::Hosts => {
                let list = self.visible_hosts();
                let at = self.selected_host.as_ref().and_then(|r| self.resolve(r)).and_then(|id| list.iter().position(|x| *x == id));
                let next = step(at, delta, list.len())?;
                self.select_host(list[next]);
                Some(crate::ui::host_key(list[next]))
            }
            Page::Keys => {
                let list = self.visible_keys();
                let at = self.selected_key.as_ref().and_then(|n| list.iter().position(|&i| &self.keys[i].name == n));
                let next = step(at, delta, list.len())?;
                let name = self.keys[list[next]].name.clone();
                self.selected_key = Some(name.clone());
                Some(format!("key-{name}"))
            }
        }
    }

    /// Whether a dialog or menu is open: keys then belong to it. The host menu counts only
    /// while it can show: on the hosts page, with a host selected.
    pub fn modal(&self) -> bool {
        (self.host_menu && self.page == Page::Hosts && self.selected_host_id().is_some()) || self.host_form.is_some() || self.new_key.is_some() || self.pass_form.is_some() || self.confirm.is_some()
    }
}

fn step(at: Option<usize>, delta: i32, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match at {
        None => 0,
        Some(i) => (i as i32 + delta).clamp(0, len as i32 - 1) as usize,
    })
}
