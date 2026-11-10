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
