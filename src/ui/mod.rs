//! The window: a sidebar (hosts, keys, the agent, the config files), the open page's list, and
//! an inspector with the selected host or key.

mod dialogs;
mod hosts;
mod keys;

use rune::{Chrome, Cx, El, Icon, KeyEvent, ScrollSpec, Tx, col};
use rune_kit::shell::{self, Sidebar, SidebarRow, SidebarSection, SidebarToggle, SplitView};
use rune_kit::{Kit, Toast, ToastRegion, glyphs};

use crate::app::{App, Page};
use crate::ds::{Place, motion, size, space};
use crate::keys::Agent;

macro_rules! icon {
    ($id:ident, $name:literal, $body:literal) => {
        pub static $id: Icon = Icon { name: $name, view: 16.0, body: $body };
    };
}

icon!(PLUS, "ks-plus", r#"<path d="M8 3.5V12.5M3.5 8H12.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#);
icon!(REFRESH, "ks-refresh", r#"<path d="M12.5 8A4.5 4.5 0 1 1 11.2 4.8M12.5 3V5.5H10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#);
icon!(SERVER, "ks-server", r#"<rect x="2.5" y="3" width="11" height="4" rx="1" stroke="currentColor" stroke-width="1.2" fill="none"/><rect x="2.5" y="9" width="11" height="4" rx="1" stroke="currentColor" stroke-width="1.2" fill="none"/><circle cx="5" cy="5" r="0.8" fill="currentColor"/><circle cx="5" cy="11" r="0.8" fill="currentColor"/>"#);
icon!(KEY, "ks-key", r#"<circle cx="5.5" cy="10.5" r="3" stroke="currentColor" stroke-width="1.2" fill="none"/><path d="M7.7 8.3L13 3M11 5L12.5 6.5M9.5 6.5L10.8 7.8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#);
icon!(AGENT, "ks-agent", r#"<path d="M8 2.5L13 4.5V8C13 10.8 10.9 12.9 8 13.5C5.1 12.9 3 10.8 3 8V4.5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/><path d="M6 8L7.5 9.5L10 6.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#);
icon!(TERMINAL, "ks-terminal", r#"<rect x="2" y="3" width="12" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2" fill="none"/><path d="M5 6.5L7 8L5 9.5M8.5 10H11" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#);
icon!(PULSE, "ks-pulse", r#"<path d="M2 8H5L6.5 4.5L9.5 11.5L11 8H14" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#);
icon!(IMPORT, "ks-import", r#"<path d="M8 2.5V9.5M5 6.5L8 9.5L11 6.5M3.5 12.5H12.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#);

pub fn chrome() -> Chrome {
    Chrome { full_size_content: true, traffic_lights: Some((16.0, 25.0)), drag_regions: true }
}

/// The key of a host's row in the list.
pub fn host_key((d, s): (usize, usize)) -> String {
    format!("host-{d}-{s}")
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = Kit::neutral(cx.system_dark());
    let mut layer = Vec::new();
    if s.gallery {
        let gallery = crate::ds::gallery::<App>(&k, &mut layer);
        return rune_kit::root(gallery, layer);
    }
    let (main, toolbar, inspector) = match s.page {
        Page::Hosts => hosts::page(s, &k, &mut layer, cx.is_reduced_motion()),
        Page::Keys => keys::page(s, &k, &mut layer, cx.is_reduced_motion()),
    };
    dialogs::host_form(s, &k, &mut layer);
    dialogs::new_key(s, &k, &mut layer);
    dialogs::pass_form(s, &k, &mut layer);
    dialogs::confirm(s, &k, &mut layer);
    if let Some(n) = &s.notice {
        let mut toast = Toast::new("notice", n.title.clone()).closable().on_close(|s: &mut App| s.notice = None);
        if !n.detail.is_empty() {
            toast = toast.description(n.detail.clone());
        }
        if n.undo {
            toast = toast.action(&k, "Undo", false, |s: &mut App, _: &mut Tx| s.undo());
        }
        toast = toast.icon(&k, if n.error { &glyphs::CIRCLE_X } else { &glyphs::CHECK });
        layer.push(ToastRegion::new().toast(toast.build(&k)).build(&k));
    }
    let mut split = SplitView::new(main).sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, sidebar(s, &k));
    if inspector_fits(s, cx) {
        split = split.inspector(&s.inspector, |s: &mut App| &mut s.inspector, scroll_pane("inspector-scroll", inspector));
    }
    let split = split
        .toolbar(toolbar)
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(space::S2)
        .toolbar_in_panel()
        .sidebar_padding(space::S2, space::S2)
        .sidebar_toggle(SidebarToggle::new().size(Place::Bar.control()).past_lights(space::S4).edge(space::S4).gap(space::S1).in_header())
        .pane_motion(motion::panes())
        .reduced_motion(cx.is_reduced_motion())
        .build(&k);
    rune_kit::root(split, layer)
}

/// Whether the window has room for the inspector beside the sidebar and a list of at least
/// `size::LIST_MIN`. In a narrower window the inspector goes first, and comes back when the
/// window widens. Before the first layout there is no width yet, and the inspector shows.
fn inspector_fits(s: &App, cx: &Cx<App>) -> bool {
    let needed = s.sidebar.shown_width() + size::LIST_MIN + size::INSPECTOR_MIN;
    cx.layout_with("kit-root", move |r| r[2] >= needed).unwrap_or(true)
}

fn sidebar(s: &App, k: &Kit) -> El {
    let hosts = SidebarRow::new("Hosts")
        .icon(&SERVER)
        .badge(s.host_count().to_string())
        .height(size::ROW_1)
        .selected(s.page == Page::Hosts && s.file_filter.is_none())
        .on_select(|s: &mut App, _| s.set_file_filter(None))
        .build(k)
        .key("nav-hosts");
    let keys = SidebarRow::new("Keys")
        .icon(&KEY)
        .badge(s.keys.len().to_string())
        .height(size::ROW_1)
        .selected(s.page == Page::Keys)
        .on_select(|s: &mut App, _| s.open_page(Page::Keys))
        .build(k)
        .key("nav-keys");

    let (agent_title, agent_sub) = match &s.agent {
        Agent::Running(l) => ("ssh-agent".to_string(), format!("{} key{} loaded", l.len(), if l.len() == 1 { "" } else { "s" })),
        Agent::Down(_) => ("ssh-agent".to_string(), "not running".to_string()),
    };
    let agent = SidebarRow::new(agent_title).icon(&AGENT).subtitle(agent_sub).height(size::ROW_2).select_only().on_select(|s: &mut App, _| s.open_page(Page::Keys)).build(k).key("nav-agent");

    let files = s.docs.iter().enumerate().map(|(i, d)| {
        let path = d.path.clone();
        let n = d.sections.len();
        SidebarRow::new(d.path.file_name().map_or_else(|| s.paths.tilde(&d.path), |n| n.to_string_lossy().into_owned()))
            .icon(&glyphs::FILE_TEXT)
            .subtitle(format!("{} · {n} host{}", s.paths.tilde(d.path.parent().unwrap_or(&d.path)), if n == 1 { "" } else { "s" }))
            .height(size::ROW_2)
            .selected(s.page == Page::Hosts && s.file_filter.as_ref() == Some(&d.path))
            .on_select(move |s: &mut App, _| s.set_file_filter(Some(path.clone())))
            .build(k)
            .key(format!("file-{i}"))
    });
    Sidebar::new()
        .label("Keysmith")
        .inset(0.0)
        .header_gap(space::S1)
        .header_padding(0.0, space::S3)
        .row_gap(space::ROW_GAP)
        .header(hosts)
        .header(keys)
        .header(agent)
        .section(SidebarSection::new("Config files").key("files").rows(files))
        .build(k)
}

/// A pane's content in a vertical scroll view that fills the pane.
pub fn scroll_pane(key: &str, content: El) -> El {
    col().key(key.to_string()).w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(content)
}

pub fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

pub fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
        return true;
    }
    if s.modal() {
        return false;
    }
    if ev.is_command('z') && s.undo.is_some() {
        s.undo();
        return true;
    }
    if ev.is_command('n') {
        // A shortcut is for speed: the dialog is there at once, without its enter motion.
        tx.instant();
        match s.page {
            Page::Hosts => s.open_add_host(),
            Page::Keys => s.open_new_key(),
        }
        return true;
    }
    if ev.is_command('r') {
        s.rescan();
        s.refresh_effective();
        return true;
    }
    if ev.is_command('f') {
        s.filter.focused = true;
        return true;
    }
    if ev.is_command('1') {
        s.set_file_filter(None);
        return true;
    }
    if ev.is_command('2') {
        s.open_page(Page::Keys);
        return true;
    }
    if !s.filter.focused && matches!(ev.key, rune::Key::Up | rune::Key::Down) && !ev.mods.cmd {
        let delta = if ev.key == rune::Key::Down { 1 } else { -1 };
        // Done many times a day: the selection and the inspector change at once.
        tx.instant();
        if let Some(key) = s.step_selection(delta) {
            tx.scroll_into_view(&key);
        }
        return true;
    }
    if !s.filter.focused && ev.key == rune::Key::Enter && s.page == Page::Hosts {
        if let Some(id) = s.selected_host_id() {
            tx.instant();
            s.open_edit_host(id);
        }
        return true;
    }
    false
}
