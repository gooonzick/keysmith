use super::*;

fn home() -> (tempfile::TempDir, Paths) {
    let tmp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(tmp.path()).unwrap();
    (tmp, Paths::isolated(&root))
}

fn find<'a>(keys: &'a [Key], name: &str) -> &'a Key {
    keys.iter().find(|k| k.name == name).unwrap_or_else(|| panic!("no key {name} in {keys:?}"))
}

/// An ssh-agent on the isolated socket, killed when dropped.
pub(crate) struct TestAgent(std::process::Child);

impl TestAgent {
    pub(crate) fn start(paths: &Paths) -> TestAgent {
        let sock = paths.agent_sock.clone().unwrap();
        let child = Command::new("ssh-agent").arg("-D").arg("-a").arg(&sock).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        for _ in 0..100 {
            if sock.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        TestAgent(child)
    }
}

impl Drop for TestAgent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn generate_and_scan() {
    let (_t, paths) = home();
    generate(&paths, KeyType::Ed25519, "id_plain", "me@test", "").unwrap();
    generate(&paths, KeyType::Ed25519, "id_locked", "locked key", "s3cret pass").unwrap();
    generate(&paths, KeyType::Ecdsa, "id_ec", "ec", "").unwrap();
    assert_eq!(fsops::mode(&paths.ssh.join("id_plain")), Some(0o600));
    assert_eq!(fsops::mode(&paths.ssh), Some(0o700));

    let (keys, errors) = scan(&paths);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(keys.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["id_ec", "id_locked", "id_plain"]);
    let plain = find(&keys, "id_plain");
    assert_eq!((plain.kind.as_str(), plain.bits, plain.encrypted), ("ED25519", Some(256), Some(false)));
    assert_eq!(plain.comment, "me@test");
    assert!(plain.fingerprint.as_deref().is_some_and(|f| f.starts_with("SHA256:")));
    assert!(plain.public_text.as_deref().is_some_and(|t| t.starts_with("ssh-ed25519 AAAA")));
    let locked = find(&keys, "id_locked");
    assert_eq!(locked.encrypted, Some(true));
    assert_eq!(locked.comment, "locked key");
    assert_eq!(find(&keys, "id_ec").kind, "ECDSA");
    assert_eq!(locked.loose_mode, None);
}

#[test]
fn a_key_without_its_pub_still_shows_its_public_half() {
    let (_t, paths) = home();
    generate(&paths, KeyType::Ed25519, "id_lone", "c", "pw").unwrap();
    let with = scan(&paths).0[0].clone();
    fs::remove_file(paths.ssh.join("id_lone.pub")).unwrap();
    let (keys, _) = scan(&paths);
    let lone = find(&keys, "id_lone");
    assert_eq!(lone.public, None);
    assert_eq!(lone.encrypted, Some(true));
    assert_eq!(lone.fingerprint, with.fingerprint, "read from the OpenSSH container without the passphrase");
    let written = write_public(lone).unwrap();
    assert!(fs::read_to_string(written).unwrap().starts_with("ssh-ed25519 "));
}

#[test]
fn names_are_checked() {
    let (_t, paths) = home();
    generate(&paths, KeyType::Ed25519, "id_a", "", "").unwrap();
    assert!(check_name(&paths, "id_a").unwrap_err().contains("already exists"));
    assert!(check_name(&paths, "../x").is_err());
    assert!(check_name(&paths, "config").is_err());
    assert!(check_name(&paths, "x.pub").is_err());
    assert!(check_name(&paths, " ").is_err());
    assert!(check_name(&paths, "id_new").is_ok());
}

#[test]
fn passphrases_change_and_wrong_ones_fail_fast() {
    let (_t, paths) = home();
    let key = generate(&paths, KeyType::Ed25519, "id_k", "c", "first").unwrap();
    assert_eq!(change_passphrase(&key, "wrong", "second").unwrap_err(), "Wrong passphrase");
    change_passphrase(&key, "first", "second").unwrap();
    change_passphrase(&key, "second", "").unwrap();
    assert_eq!(find(&scan(&paths).0, "id_k").encrypted, Some(false));
    change_passphrase(&key, "", "third").unwrap();
    assert_eq!(find(&scan(&paths).0, "id_k").encrypted, Some(true));
    change_comment(&key, "third", "renamed").unwrap();
    assert_eq!(find(&scan(&paths).0, "id_k").comment, "renamed");
}

#[test]
fn the_agent_loads_and_unloads_keys() {
    let (_t, paths) = home();
    assert!(matches!(agent(&paths), Agent::Down(_)), "nothing listens on the isolated socket");
    let _agent = TestAgent::start(&paths);
    assert_eq!(agent(&paths), Agent::Running(Vec::new()));

    let path = generate(&paths, KeyType::Ed25519, "id_k", "c", "pw").unwrap();
    let key = find(&scan(&paths).0, "id_k").clone();
    assert_eq!(agent_add(&paths, &path, "nope", false).unwrap_err(), "Wrong passphrase");
    agent_add(&paths, &path, "pw", false).unwrap();
    let Agent::Running(loaded) = agent(&paths) else { panic!("the agent stopped") };
    assert_eq!(loaded.iter().map(|(fp, _)| fp.clone()).collect::<Vec<_>>(), [key.fingerprint.clone().unwrap()]);

    agent_remove(&paths, &key).unwrap();
    assert_eq!(agent(&paths), Agent::Running(Vec::new()));
    agent_add(&paths, &path, "pw", false).unwrap();
    agent_clear(&paths).unwrap();
    assert_eq!(agent(&paths), Agent::Running(Vec::new()));
}

#[test]
fn loose_permissions_are_seen_and_fixed() {
    let (_t, paths) = home();
    let path = generate(&paths, KeyType::Ed25519, "id_k", "", "").unwrap();
    fsops::set_mode(&path, 0o644).unwrap();
    let key = find(&scan(&paths).0, "id_k").clone();
    assert_eq!(key.loose_mode, Some(0o644));
    fix_permissions(&paths, &key).unwrap();
    assert_eq!(find(&scan(&paths).0, "id_k").loose_mode, None);
}

#[test]
fn public_only_keys_import_and_trash() {
    let (_t, paths) = home();
    let elsewhere = paths.home.join("Downloads");
    fs::create_dir_all(&elsewhere).unwrap();
    let other = Paths { ssh: elsewhere.clone(), ..paths.clone() };
    generate(&other, KeyType::Ed25519, "id_from", "imported", "").unwrap();
    fs::copy(elsewhere.join("id_from.pub"), paths.ssh.join("token.pub")).unwrap_or_else(|_| {
        fs::create_dir_all(&paths.ssh).unwrap();
        fs::copy(elsewhere.join("id_from.pub"), paths.ssh.join("token.pub")).unwrap()
    });
    let to = import(&paths, &elsewhere.join("id_from")).unwrap();
    assert_eq!(fsops::mode(&to), Some(0o600));
    assert!(import(&paths, &elsewhere.join("id_from.pub")).is_err(), "a public key isn't a private key");

    let (keys, _) = scan(&paths);
    let token = find(&keys, "token");
    assert_eq!((token.private.as_ref(), token.comment.as_str()), (None, "imported"));
    let imported = find(&keys, "id_from").clone();
    assert!(imported.public.is_some(), "the .pub came along");

    trash(&paths, &imported).unwrap();
    assert!(!to.exists() && !to.with_file_name("id_from.pub").exists());
    assert!(paths.trash.read_dir().unwrap().count() >= 1);
}

#[test]
fn base64_and_listings() {
    for s in [&b""[..], b"f", b"fo", b"foo", b"foob", b"\x00\xff\x10"] {
        assert_eq!(base64_decode(&base64_encode(s)).unwrap(), s);
    }
    assert_eq!(parse_listing("256 SHA256:abc me@x (ED25519)"), Some((256, "SHA256:abc".into(), "me@x".into(), "ED25519".into())));
    assert_eq!(parse_listing("4096 SHA256:abc two words (RSA)").unwrap().2, "two words");
    assert_eq!(parse_listing("256 SHA256:abc (ED25519)").unwrap().2, "");
}
