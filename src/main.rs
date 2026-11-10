//! Keysmith: your SSH config and SSH keys in one native window. Hosts are edited in place, so
//! the rest of `~/.ssh/config` stays as you wrote it; keys are made, unlocked and changed with
//! OpenSSH's own `ssh-keygen` and `ssh-add`.
//!
//! `cargo run` opens the window. `KEYSMITH_HOME=dir` uses `dir` as home (its `.ssh`, its
//! trash) instead of yours. `cargo run -- --snap out.png --scene NAME` renders headless.

mod app;
mod ds;
mod fsops;
mod keys;
mod paths;
mod sshconfig;
mod ui;
#[cfg(test)]
mod ui_tests;

use rune::{AppSpec, Tx, Waker};

use crate::app::{App, PassPurpose};
use crate::ds::motion::COPY_CONFIRM_SECS;
use crate::paths::Paths;

fn init() -> App {
    let paths = match std::env::var_os("KEYSMITH_HOME") {
        Some(dir) => Paths::isolated(&std::path::PathBuf::from(dir)),
        None => Paths::from_env(),
    };
    App::new(paths)
}

fn setup(s: &mut App, waker: Waker) {
    s.set_waker(waker);
}

/// How long a notice stays up.
const NOTICE_SECS: f64 = 5.0;

fn poll(s: &mut App, clock: f64, tx: &mut Tx) -> bool {
    let mut changed = s.poll();
    match (&s.notice, s.notice_at) {
        (Some(_), None) => {
            s.notice_at = Some(clock);
            tx.wake_after(std::time::Duration::from_secs_f64(NOTICE_SECS));
        }
        (Some(_), Some(at)) if clock - at >= NOTICE_SECS => {
            s.notice = None;
            s.notice_at = None;
            changed = true;
        }
        (None, Some(_)) => s.notice_at = None,
        _ => {}
    }
    match &mut s.copied_at {
        Some((_, at)) if at.is_nan() => {
            *at = clock;
            tx.wake_after(std::time::Duration::from_secs_f64(COPY_CONFIRM_SECS));
        }
        Some((_, at)) if clock - *at >= COPY_CONFIRM_SECS => {
            s.copied_at = None;
            changed = true;
        }
        _ => {}
    }
    changed
}

/// Headless scenes (`--scene NAME`) for checking the layout without clicking.
type Scene = (&'static str, fn(&mut App));

static SCENES: &[Scene] = &[
    ("hosts", |_| {}),
    ("ds", |s| s.gallery = true),
    ("host-form", |s| {
        if let Some(id) = s.selected_host_id() {
            s.open_edit_host(id);
        }
    }),
    ("add-host", |s| s.open_add_host()),
    ("keys", |s| s.open_page(app::Page::Keys)),
    ("new-key", |s| s.open_new_key()),
    ("passphrase", |s| {
        s.open_page(app::Page::Keys);
        if let Some(n) = s.selected_key.clone() {
            s.open_pass_form(&n, PassPurpose::Change);
        }
    }),
    ("remove", |s| {
        if let Some(id) = s.selected_host_id() {
            s.confirm_remove_host(id);
        }
    }),
];

fn main() {
    rune::prewarm_gpu();
    let spec = AppSpec {
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(ui::on_key),
        min_size: Some((860.0, 540.0)),
        scenes: SCENES,
        ..AppSpec::new("Keysmith", (1200.0, 760.0), init, ui::view)
    };
    rune::App::new(spec).chrome(ui::chrome()).run();
}
