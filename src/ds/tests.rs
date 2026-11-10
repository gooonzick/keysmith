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
