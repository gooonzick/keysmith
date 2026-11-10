use super::*;
use rune_kit::Kit;

#[test]
fn scales() {
    assert_eq!([space::S1, space::S2, space::S3, space::S4, space::S6], [4.0, 8.0, 12.0, 16.0, 24.0]);
    assert_eq!([radius::TAG, radius::CONTROL, radius::CARD, radius::DIALOG], [4.0, 6.0, 8.0, 12.0]);
}

#[test]
fn reduced_motion_keeps_only_opacity() {
    assert_eq!(motion::press_scale(true), 1.0);
    assert_eq!(motion::press_scale(false), 0.97);
}

#[test]
fn type_ladder() {
    let k = Kit::neutral(false);
    let t = ty::title(&k);
    assert_eq!((t.size, t.weight, t.line_height), (15.0, 600.0, 20.0));
    let v = ty::value(&k);
    assert_eq!((v.size, v.weight, v.line_height), (13.0, 400.0, 20.0));
    assert_eq!(ty::name(&k).weight, 500.0);
    let c = ty::code(&k);
    assert_eq!((c.size, c.line_height), (12.0, 18.0));
    assert_eq!(c.family, k.ty.mono.family);
    let l = ty::label(&k);
    assert_eq!((l.size, l.weight, l.line_height), (11.0, 400.0, 16.0));
    assert_eq!(ty::hint(&k), l);
}

#[test]
fn roles_differ_in_both_themes() {
    for dark in [false, true] {
        let k = Kit::neutral(dark);
        assert_ne!(color::text(&k), color::text_muted(&k));
        assert_ne!(color::text_muted(&k), color::text_faint(&k));
        assert_ne!(color::text(&k), color::text_faint(&k));
        assert_ne!(color::card(&k), color::selected(&k));
    }
}

#[test]
fn places() {
    assert_eq!(Place::Inspector.height(), 24.0);
    assert_eq!(Place::List.height(), 28.0);
    assert_eq!(Place::Bar.height(), 28.0);
}

mod blocks {
    use super::*;
    use rune::{Cx, El, Env, Runtime, col};

    const W: f32 = 400.0;

    fn rt(view: fn(&(), &mut Cx<()>) -> El) -> Runtime<()> {
        let mut rt = Runtime::new((), view, (W, 600.0), Env { system_dark: false, scale: 1.0 });
        rt.frame(0.0);
        rt
    }

    fn long() -> String {
        "x".repeat(300)
    }

    fn shell(card: Option<El>) -> El {
        col().w_full().h_full().padding(16.0).child(card.expect("card has rows"))
    }

    fn three_rows(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        shell(
            Card::new("card")
                .label("Group")
                .row(pair_row_with(text::label(&k, "Port").key("r1-label"), text::value(&k, "22")))
                .row(stacked_row_with(text::label(&k, "Path").key("r2-label"), text::code(&k, "abc")))
                .row(item_row(&k, text::name(&k, "work").key("r3-name"), Some(text::hint(&k, "deploy")), None))
                .build(&k),
        )
    }

    #[test]
    fn card_rows_share_one_edge() {
        let rt = rt(three_rows);
        let x = |key: &str| rt.presented(key).unwrap_or_else(|| panic!("no {key}")).0[0];
        assert_eq!(x("r1-label"), x("r2-label"));
        assert_eq!(x("r2-label"), x("r3-name"));
    }

    fn long_pair(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        shell(Card::new("card").row(pair_row(&k, "Path", text::value(&k, long()).key("val")).key("row")).build(&k))
    }

    #[test]
    fn pair_row_is_one_line_and_truncates() {
        let rt = rt(long_pair);
        let row = rt.presented("row").unwrap().0;
        assert!(row[3] < 40.0, "row is {} high", row[3]);
        let card = rt.presented("card").unwrap().0;
        assert!(card[2] <= W, "card is {} wide", card[2]);
        let v = rt.presented("val").unwrap().0;
        assert!(v[0] + v[2] <= card[0] + card[2], "value ends at {}, card at {}", v[0] + v[2], card[0] + card[2]);
    }

    fn long_stacked(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        shell(Card::new("card").row(stacked_row(&k, "Key", text::code(&k, long()).key("val")).key("row")).build(&k))
    }

    #[test]
    fn stacked_row_wraps_long_value() {
        let rt = rt(long_stacked);
        let row = rt.presented("row").unwrap().0;
        assert!(row[3] > 40.0, "row is {} high", row[3]);
        let card = rt.presented("card").unwrap().0;
        assert!(card[2] <= W, "card is {} wide", card[2]);
        let v = rt.presented("val").unwrap().0;
        assert!(v[0] + v[2] <= card[0] + card[2], "value ends at {}, card at {}", v[0] + v[2], card[0] + card[2]);
    }

    #[test]
    fn empty_card_is_none() {
        let k = Kit::neutral(false);
        assert!(Card::new("c").label("X").build(&k).is_none());
    }

    fn notice(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        col().w_full().h_full().padding(16.0).child(Notice::new(Tone::Warn, "word ".repeat(40)).key("n").action(rune::el().key("fix").size(60.0, 24.0)).build(&k))
    }

    #[test]
    fn notice_wraps_and_keeps_its_button() {
        let rt = rt(notice);
        let t = rt.presented("n-text").unwrap().0;
        assert!(t[3] > 40.0, "notice text is {} high", t[3]);
        let (x, y) = rt.find("fix").expect("fix button");
        assert!(x > 0.0 && x < W && y > 0.0, "button centre at {x},{y}");
        let b = rt.presented("fix").unwrap().0;
        assert!(b[0] + b[2] <= W, "button ends at {}", b[0] + b[2]);
    }
}

mod skeletons {
    use super::*;
    use rune::{Cx, El, Env, Runtime, col};
    use rune_kit::glyphs;

    fn env() -> Env {
        Env { system_dark: false, scale: 1.0 }
    }

    fn page_view(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        let edit = rune::TextEdit::new("");
        Page::new("page")
            .filter(filter(&edit, "Filter").build(&k).key("filter"))
            .count("1 host")
            .notice(Notice::new(Tone::Warn, "Careful").key("notice").build(&k).key("notice-box"))
            .rows([ListRow::new("row", "work").build(&k, |_: &mut ()| {})])
            .build(&k)
    }

    #[test]
    fn page_has_one_content_edge() {
        let mut rt = Runtime::new((), page_view, (400.0, 600.0), env());
        rt.frame(0.0);
        for key in ["filter", "count", "notice-box", "row-name"] {
            let x = rt.presented(key).unwrap_or_else(|| panic!("no {key}")).0[0];
            assert!((x - 16.0).abs() <= 0.5, "{key} starts at {x}");
        }
    }

    fn rows_view(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        col()
            .w_full()
            .child(ListRow::new("with", "a").meta("b").build(&k, |_: &mut ()| {}))
            .child(ListRow::new("without", "a").icon(&glyphs::SEARCH).build(&k, |_: &mut ()| {}))
    }

    #[test]
    fn list_row_heights() {
        let mut rt = Runtime::new((), rows_view, (400.0, 600.0), env());
        rt.frame(0.0);
        assert_eq!(rt.presented("with").unwrap().0[3], 44.0);
        assert_eq!(rt.presented("without").unwrap().0[3], 32.0);
    }

    fn crowded(_: &(), _: &mut Cx<()>) -> El {
        let k = Kit::neutral(false);
        let mut page = Page::new("page").count("40 hosts");
        for i in 0..6 {
            page = page.notice(Notice::new(Tone::Info, format!("notice {i} {}", "word ".repeat(12))).key(format!("n{i}")).build(&k));
        }
        page.rows((0..40).map(|i| ListRow::new(format!("row{i}"), format!("host {i}")).build(&k, |_: &mut ()| {}))).build(&k)
    }

    #[test]
    fn many_notices_leave_the_list_scrollable() {
        let mut rt = Runtime::new((), crowded, (400.0, 600.0), env());
        rt.frame(0.0);
        let last = rt.presented("n5-text").unwrap().0;
        assert!(last[1] + last[3] <= 600.0, "last notice ends at {}", last[1] + last[3]);
        assert!(rt.find("row0").is_some(), "first row is not on screen");
        let page = rt.presented("page").unwrap().0;
        assert!(page[3] <= 600.0, "page is {} high", page[3]);
        let list = rt.presented("list").unwrap().0;
        assert!(list[3] > 100.0, "list is only {} high", list[3]);
    }

    #[derive(Default)]
    struct Menus {
        open: bool,
        ran: u32,
    }

    fn actions_view(s: &Menus, _: &mut Cx<Menus>) -> El {
        let k = Kit::neutral(false);
        let mut layer = Vec::new();
        let more = overflow(
            &k,
            "more",
            s.open,
            |s: &mut Menus| s.open = !s.open,
            |s: &mut Menus| s.open = false,
            vec![MenuAction { label: "Copy".into(), disabled: false, run: Box::new(|s: &mut Menus| s.ran += 1) }],
            &mut layer,
        );
        let pane = Inspector::new()
            .actions(
                vec![
                    button("One", Place::Inspector).build(&k).key("b1"),
                    button("Two", Place::Inspector).build(&k).key("b2"),
                    button("Three", Place::Inspector).build(&k).key("b3"),
                    more,
                ],
                Some(danger("Delete", Place::Inspector).build(&k).key("del")),
            )
            .build(&k);
        rune_kit::root(col().w_full().h_full().child(pane), layer)
    }

    fn click(rt: &mut Runtime<Menus>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.0);
    }

    #[test]
    fn inspector_actions_stay_on_one_row() {
        let mut rt = Runtime::new(Menus::default(), actions_view, (320.0, 400.0), env());
        rt.frame(0.0);
        let ys: Vec<f32> = ["b1", "b2", "b3", "more-button", "del"].iter().map(|k| rt.find(k).unwrap_or_else(|| panic!("no {k}")).1).collect();
        assert!(ys.iter().all(|y| (y - ys[0]).abs() < 0.5), "centres {ys:?}");
        let del = rt.presented("del").unwrap().0;
        assert!((del[0] + del[2] - (320.0 - 16.0)).abs() <= 0.5, "delete ends at {}", del[0] + del[2]);
    }

    #[test]
    fn overflow_opens_and_runs() {
        let mut rt = Runtime::new(Menus::default(), actions_view, (320.0, 400.0), env());
        rt.frame(0.0);
        click(&mut rt, "more-button");
        assert!(rt.state.open, "trigger did not toggle");
        click(&mut rt, "more-item0");
        assert_eq!(rt.state.ran, 1);
        assert!(!rt.state.open, "choosing an item closes the menu");
    }
}
