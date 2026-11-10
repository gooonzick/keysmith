use super::*;

const SAMPLE: &str = "\
# My config
Include ~/.ssh/config.d/*
AddKeysToAgent yes

# Work box
# behind the VPN
Host work
  HostName 10.0.0.5
  User deploy
  IdentityFile ~/.ssh/id_work
  # keep alive
  ServerAliveInterval 30

Host github.com gh
  HostName github.com
  IdentityFile ~/.ssh/id_ed25519
  IdentityFile ~/.ssh/id_backup

Match host *.corp exec \"true\"
  User admin

Host *
  ServerAliveCountMax 3
";

fn doc() -> Document {
    Document::parse(PathBuf::from("/h/.ssh/config"), SAMPLE)
}

fn spec(d: &Document, i: usize) -> HostSpec {
    HostSpec::of(&d.sections[i])
}

#[test]
fn parses_sections_options_and_descriptions() {
    let d = doc();
    assert_eq!(d.text(), SAMPLE, "an untouched file writes back byte for byte");
    assert_eq!(d.global.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["Include", "AddKeysToAgent"]);
    assert_eq!(d.includes(), ["~/.ssh/config.d/*"]);
    let names: Vec<&str> = d.sections.iter().map(|s| s.patterns.as_str()).collect();
    assert_eq!(names, ["work", "github.com gh", "host *.corp exec \"true\"", "*"]);
    let work = &d.sections[0];
    assert_eq!(work.doc, ["Work box", "behind the VPN"]);
    assert_eq!(work.get("hostname").as_deref(), Some("10.0.0.5"));
    assert_eq!(work.alias(), Some("work"));
    assert!(!work.is_pattern());
    assert_eq!(d.sections[1].all("IdentityFile"), ["~/.ssh/id_ed25519", "~/.ssh/id_backup"]);
    assert_eq!(d.sections[2].kind, Kind::Match);
    assert!(d.sections[2].is_pattern() && d.sections[3].is_pattern());
    assert!(d.sections[0].doc_start < d.sections[0].header);
}

#[test]
fn key_value_forms() {
    assert_eq!(split_option("  Port=22"), Some(("Port".into(), "22".into())));
    assert_eq!(split_option("Port = 22"), Some(("Port".into(), "22".into())));
    assert_eq!(split_option("\tUser\tme"), Some(("User".into(), "me".into())));
    assert_eq!(split_option("  # Port 22"), None);
    assert_eq!(split_option("   "), None);
}

#[test]
fn editing_a_value_touches_only_its_line() {
    let mut d = doc();
    let mut s = spec(&d, 0);
    s.options[1].1 = "root".into();
    d.replace(0, &s);
    assert_eq!(d.text(), SAMPLE.replace("  User deploy", "  User root"));
}

#[test]
fn adding_removing_and_repeated_keys() {
    let mut d = doc();
    let mut s = spec(&d, 1);
    // Drop the second IdentityFile, add a Port.
    s.options.retain(|(_, v)| v != "~/.ssh/id_backup");
    s.options.push(("Port".into(), "2222".into()));
    d.replace(1, &s);
    let gh = &d.sections[1];
    assert_eq!(gh.all("IdentityFile"), ["~/.ssh/id_ed25519"]);
    assert_eq!(gh.get("Port").as_deref(), Some("2222"));
    assert!(d.text().contains("  IdentityFile ~/.ssh/id_ed25519\n  Port 2222\n\nMatch"), "{}", d.text());
}

#[test]
fn new_options_go_before_trailing_comments_and_keep_comments_inside() {
    let mut d = doc();
    let mut s = spec(&d, 0);
    s.options.push(("Port".into(), "22".into()));
    d.replace(0, &s);
    assert!(d.text().contains("  # keep alive\n  ServerAliveInterval 30\n  Port 22\n\nHost github.com"), "{}", d.text());
}

#[test]
fn renaming_and_redescribing() {
    let mut d = doc();
    let mut s = spec(&d, 0);
    s.patterns = "work w".into();
    s.doc = vec!["The work box".into()];
    d.replace(0, &s);
    assert!(d.text().contains("\n# The work box\nHost work w\n  HostName 10.0.0.5"), "{}", d.text());
    assert!(!d.text().contains("behind the VPN"));
    assert_eq!(d.sections.len(), 4);
}

#[test]
fn add_goes_above_a_closing_host_star() {
    let mut d = doc();
    let i = d.add(&HostSpec { kind: None, patterns: "pi".into(), doc: vec!["Raspberry".into()], options: vec![("HostName".into(), "192.168.1.9".into()), ("User".into(), "pi".into())] });
    assert_eq!(d.sections[i].patterns, "pi");
    assert_eq!(d.sections[i + 1].patterns, "*");
    assert!(d.text().contains("  User admin\n\n# Raspberry\nHost pi\n  HostName 192.168.1.9\n  User pi\n\nHost *\n"), "{}", d.text());
}

#[test]
fn add_to_an_empty_file_and_to_one_without_host_star() {
    let mut d = Document::empty(PathBuf::from("/c"));
    d.add(&HostSpec { patterns: "a".into(), options: vec![("User".into(), "x".into())], ..Default::default() });
    assert_eq!(d.text(), "Host a\n    User x\n");
    d.add(&HostSpec { patterns: "b".into(), ..Default::default() });
    assert_eq!(d.text(), "Host a\n    User x\n\nHost b\n");
}

#[test]
fn remove_takes_the_description_and_one_blank_line() {
    let mut d = doc();
    d.remove(0);
    assert!(d.text().starts_with("# My config\nInclude ~/.ssh/config.d/*\nAddKeysToAgent yes\n\nHost github.com gh\n"), "{}", d.text());
    let mut d = doc();
    d.remove(3);
    assert!(d.text().ends_with("  User admin\n\n") || d.text().ends_with("  User admin\n"), "{:?}", d.text());
    assert_eq!(d.sections.len(), 3);
}

#[test]
fn swap_moves_whole_sections() {
    let mut d = doc();
    d.swap(0, 1);
    assert_eq!(d.sections[0].patterns, "github.com gh");
    assert_eq!(d.sections[1].patterns, "work");
    assert_eq!(d.sections[1].doc, ["Work box", "behind the VPN"]);
    d.swap(0, 1);
    assert_eq!(d.text(), SAMPLE);
}

#[test]
fn no_final_newline_and_crlf() {
    let d = Document::parse(PathBuf::from("/c"), "Host a\n  User b");
    assert_eq!(d.text(), "Host a\n  User b");
    let d = Document::parse(PathBuf::from("/c"), "Host a\r\n  User b\r\n");
    assert_eq!(d.sections[0].get("User").as_deref(), Some("b"));
}

#[test]
fn includes_resolve_globs_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::isolated(tmp.path());
    std::fs::create_dir_all(paths.ssh.join("config.d")).unwrap();
    std::fs::write(&paths.config, "Include config.d/*.conf\nHost main\n").unwrap();
    std::fs::write(paths.ssh.join("config.d/b.conf"), "Host b\n").unwrap();
    std::fs::write(paths.ssh.join("config.d/a.conf"), "Host a\nInclude ~/.ssh/config\n").unwrap();
    std::fs::write(paths.ssh.join("config.d/skip.txt"), "Host no\n").unwrap();
    let (docs, errors) = load(&paths);
    assert!(errors.is_empty());
    let names: Vec<String> = docs.iter().flat_map(|d| d.sections.iter().map(|s| s.patterns.clone())).collect();
    assert_eq!(names, ["main", "a", "b"], "each file once, a loop back to the config is ignored");
}

#[test]
fn glob_matches_like_ssh() {
    assert!(glob("*.conf", "a.conf"));
    assert!(glob("h?st", "host"));
    assert!(!glob("*.conf", "a.txt"));
    assert!(glob("*", ""));
}

/// `cargo test -- --ignored`: your own config and its includes read back byte for byte.
#[test]
#[ignore]
fn the_real_config_round_trips() {
    let paths = Paths::from_env();
    let (docs, errors) = load(&paths);
    assert!(errors.is_empty(), "{errors:?}");
    for d in docs {
        if let Ok(text) = std::fs::read_to_string(&d.path) {
            assert_eq!(d.text(), text, "{}", d.path.display());
        }
    }
}

#[test]
fn keywords_in_the_manuals_spelling() {
    assert_eq!(canonical("addkeystoagent"), "AddKeysToAgent");
    assert_eq!(canonical("hostname"), "HostName");
    assert_eq!(canonical("canonicalizePermittedcnames"), "CanonicalizePermittedCNAMEs");
    assert_eq!(canonical("applemultipath"), "applemultipath");
}
