//! The keys in `~/.ssh`, and what OpenSSH's own tools do to them: `ssh-keygen` makes keys,
//! changes passphrases and prints fingerprints; `ssh-add` loads keys into the agent.
//!
//! Passphrases never go on a command line, where `ps` shows them to every user: the tools ask
//! for them through `SSH_ASKPASS`, a tiny script that reads them from its environment.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::fsops;
use crate::paths::Paths;

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// The private key's file name, or the public key's without `.pub`.
    pub name: String,
    pub private: Option<PathBuf>,
    pub public: Option<PathBuf>,
    /// `ssh-ed25519 AAAA… comment`.
    pub public_text: Option<String>,
    /// `ED25519`, `RSA`, `ECDSA`…
    pub kind: String,
    pub bits: Option<u32>,
    /// `SHA256:…`.
    pub fingerprint: Option<String>,
    pub comment: String,
    /// `None` when the file couldn't say.
    pub encrypted: Option<bool>,
    /// The private key's mode, when group or others can read it (ssh refuses such keys).
    pub loose_mode: Option<u32>,
}

impl Key {
    pub fn path(&self) -> &Path {
        self.private.as_deref().or(self.public.as_deref()).expect("a key has a file")
    }
}

/// Files in `~/.ssh` that are never keys.
fn skipped(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with("known_hosts")
        || name.starts_with("authorized_keys")
        || name.starts_with("config")
        || name.ends_with(".bak")
        || name.ends_with(".old")
        || name == "environment"
        || name == "rc"
}

/// The PEM body of a private key file, if it is one.
fn private_pem(text: &str) -> Option<&str> {
    let start = text.find("-----BEGIN ")?;
    let head_end = text[start..].find('\n')? + start;
    let head = &text[start..head_end];
    head.contains("PRIVATE KEY").then_some(&text[start..])
}

pub fn scan(paths: &Paths) -> (Vec<Key>, Vec<String>) {
    let mut keys = Vec::new();
    let mut errors = Vec::new();
    let Ok(rd) = fs::read_dir(&paths.ssh) else { return (keys, errors) };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    entries.sort();
    for path in &entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if skipped(&name) || name.ends_with(".pub") {
            continue;
        }
        // Keys are small; a big file is something else.
        if fs::metadata(path).map(|m| m.len() > 64 * 1024).unwrap_or(true) {
            continue;
        }
        let Ok(text) = fs::read_to_string(path) else { continue };
        let Some(pem) = private_pem(&text) else { continue };
        let public = path.with_file_name(format!("{name}.pub"));
        let public = public.is_file().then_some(public);
        match read_key(name, Some(path.clone()), public, Some(pem)) {
            Ok(k) => keys.push(k),
            Err(e) => errors.push(e),
        }
    }
    // Public keys whose private half isn't here (a key on a hardware token, or copied in).
    for path in &entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".pub") else { continue };
        if keys.iter().any(|k| k.public.as_deref() == Some(path.as_path())) || skipped(stem) {
            continue;
        }
        if let Ok(k) = read_key(stem.to_string(), None, Some(path.clone()), None) {
            keys.push(k);
        }
    }
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    (keys, errors)
}

fn read_key(name: String, private: Option<PathBuf>, public: Option<PathBuf>, pem: Option<&str>) -> Result<Key, String> {
    let mut public_text = public.as_ref().and_then(|p| fs::read_to_string(p).ok()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let (mut encrypted, mut kind) = (None, String::new());
    if let Some(pem) = pem {
        let info = inspect_pem(pem);
        encrypted = info.encrypted;
        if public_text.is_none() {
            public_text = info.public;
        }
        kind = info.kind.unwrap_or_default();
    }
    let mut key = Key {
        name,
        loose_mode: private.as_deref().and_then(fsops::mode).filter(|m| m & 0o077 != 0),
        private,
        public,
        public_text,
        kind,
        bits: None,
        fingerprint: None,
        comment: String::new(),
        encrypted,
    };
    if let Some(text) = &key.public_text {
        let mut parts = text.split_whitespace();
        let (_, _) = (parts.next(), parts.next());
        key.comment = parts.collect::<Vec<_>>().join(" ");
        if let Some((bits, fp, kind)) = fingerprint_of(text) {
            key.bits = Some(bits);
            key.fingerprint = Some(fp);
            key.kind = kind;
        }
    }
    if key.kind.is_empty() {
        key.kind = "unknown".into();
    }
    Ok(key)
}

/// What a private key file tells without its passphrase.
#[derive(Debug, Default, PartialEq)]
struct PemInfo {
    encrypted: Option<bool>,
    /// The public half, which the OpenSSH format keeps in the clear.
    public: Option<String>,
    kind: Option<String>,
}

fn inspect_pem(pem: &str) -> PemInfo {
    let head = pem.lines().next().unwrap_or_default();
    if head.contains("ENCRYPTED PRIVATE KEY") || pem.contains("Proc-Type: 4,ENCRYPTED") {
        return PemInfo { encrypted: Some(true), ..Default::default() };
    }
    if !head.contains("OPENSSH PRIVATE KEY") {
        let kind = ["RSA", "EC", "DSA"].iter().find(|k| head.contains(&format!("BEGIN {k} PRIVATE"))).map(|k| if *k == "EC" { "ECDSA".to_string() } else { k.to_string() });
        return PemInfo { encrypted: Some(false), kind, ..Default::default() };
    }
    let b64: String = pem.lines().skip(1).take_while(|l| !l.starts_with("-----END")).collect();
    let Some(blob) = base64_decode(&b64) else { return PemInfo::default() };
    let Some(rest) = blob.strip_prefix(b"openssh-key-v1\0") else { return PemInfo::default() };
    let mut r = Reader(rest);
    let (Some(cipher), Some(_kdf), Some(_opts), Some(n)) = (r.string(), r.string(), r.string(), r.u32()) else { return PemInfo::default() };
    let mut info = PemInfo { encrypted: Some(cipher != b"none"), ..Default::default() };
    if n >= 1
        && let Some(pubblob) = r.string()
    {
        let ty = Reader(pubblob).string().map(|t| String::from_utf8_lossy(t).into_owned());
        if let Some(ty) = ty {
            info.kind = Some(kind_of(&ty));
            info.public = Some(format!("{ty} {}", base64_encode(pubblob)));
        }
    }
    info
}

fn kind_of(ty: &str) -> String {
    match ty {
        "ssh-ed25519" => "ED25519".into(),
        "ssh-rsa" => "RSA".into(),
        t if t.starts_with("ecdsa-") => "ECDSA".into(),
        t if t.starts_with("sk-ssh-ed25519") => "ED25519-SK".into(),
        t if t.starts_with("sk-ecdsa") => "ECDSA-SK".into(),
        t => t.to_uppercase(),
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn u32(&mut self) -> Option<u32> {
        let (b, rest) = self.0.split_first_chunk::<4>()?;
        self.0 = rest;
        Some(u32::from_be_bytes(*b))
    }
    fn string(&mut self) -> Option<&'a [u8]> {
        let n = self.u32()? as usize;
        if n > self.0.len() {
            return None;
        }
        let (s, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(s)
    }
}

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|&b| b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &b)| acc | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `ssh-keygen -l` on a public key line: (bits, `SHA256:…`, type).
pub fn fingerprint_of(public_line: &str) -> Option<(u32, String, String)> {
    let mut child = Command::new("ssh-keygen").args(["-l", "-E", "sha256", "-f", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    child.stdin.take()?.write_all(format!("{public_line}\n").as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_listing(String::from_utf8_lossy(&out.stdout).lines().next()?).map(|(bits, fp, _, kind)| (bits, fp, kind))
}

/// A line as `ssh-keygen -l` and `ssh-add -l` print one: `256 SHA256:x comment (ED25519)`.
pub fn parse_listing(line: &str) -> Option<(u32, String, String, String)> {
    let line = line.trim();
    let (bits, rest) = line.split_once(' ')?;
    let (fp, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let (comment, kind) = match rest.rfind(" (") {
        Some(i) if rest.ends_with(')') => (&rest[..i], &rest[i + 2..rest.len() - 1]),
        _ if rest.starts_with('(') && rest.ends_with(')') => ("", &rest[1..rest.len() - 1]),
        _ => (rest, ""),
    };
    Some((bits.parse().ok()?, fp.to_string(), comment.to_string(), kind.to_string()))
}

// The agent.

#[derive(Clone, Debug, PartialEq)]
pub enum Agent {
    /// Fingerprints of the loaded keys, with their comments.
    Running(Vec<(String, String)>),
    /// No agent answers on the socket.
    Down(String),
}

fn ssh_add(paths: &Paths) -> Command {
    let mut c = Command::new("ssh-add");
    if let Some(sock) = &paths.agent_sock {
        c.env("SSH_AUTH_SOCK", sock);
    }
    c.stdin(Stdio::null());
    c
}

pub fn agent(paths: &Paths) -> Agent {
    let out = match ssh_add(paths).args(["-l", "-E", "sha256"]).output() {
        Ok(o) => o,
        Err(e) => return Agent::Down(format!("Couldn't run ssh-add: {e}")),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    match out.status.code() {
        Some(0) => Agent::Running(stdout.lines().filter_map(parse_listing).map(|(_, fp, c, _)| (fp, c)).collect()),
        // "The agent has no identities."
        Some(1) => Agent::Running(Vec::new()),
        _ => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Agent::Down(if err.is_empty() { "No ssh-agent is running".into() } else { err })
        }
    }
}

/// The askpass script: answers "new passphrase" prompts with `KEYSMITH_NEW` and every other
/// prompt with `KEYSMITH_OLD` once, then gives up, so a wrong passphrase ends the tool's retry
/// loop instead of feeding it forever.
const ASKPASS: &str = r#"#!/bin/sh
case "$1" in
  *[Nn]ew\ passphrase*|*same\ passphrase*|*"empty for no passphrase"*) printf '%s\n' "$KEYSMITH_NEW"; exit 0 ;;
  *[Bb]ad\ passphrase*) : > "$KEYSMITH_MARK.bad"; exit 1 ;;
esac
if [ -e "$KEYSMITH_MARK" ]; then exit 1; fi
: > "$KEYSMITH_MARK"
printf '%s\n' "$KEYSMITH_OLD"
"#;

/// A private folder with the askpass script, removed when dropped.
struct Askpass {
    dir: PathBuf,
}

impl Askpass {
    fn new() -> std::io::Result<Askpass> {
        let base = std::env::temp_dir();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let dir = base.join(format!("keysmith-{}-{stamp}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&dir)?;
        let script = dir.join("askpass");
        fs::OpenOptions::new().write(true).create_new(true).mode(0o700).open(&script)?.write_all(ASKPASS.as_bytes())?;
        Ok(Askpass { dir })
    }

    /// Points `cmd`'s prompts at the script.
    fn apply(&self, cmd: &mut Command, old: &str, new: &str) {
        cmd.env("SSH_ASKPASS", self.dir.join("askpass"))
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into()))
            .env("KEYSMITH_OLD", old)
            .env("KEYSMITH_NEW", new)
            .env("KEYSMITH_MARK", self.dir.join("asked"))
            .stdin(Stdio::null());
    }
}

impl Drop for Askpass {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn run(mut cmd: Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("Couldn't run {}: {e}", cmd.get_program().to_string_lossy()))?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
    if out.status.success() {
        Ok(text(&out.stdout))
    } else {
        let err = text(&out.stderr);
        Err(if err.is_empty() { text(&out.stdout) } else { err })
    }
}

/// Loads a key into the agent. On macOS `keychain` also stores its passphrase in the login
/// keychain, so the agent can load it again after a restart without asking.
pub fn agent_add(paths: &Paths, key: &Path, passphrase: &str, keychain: bool) -> Result<(), String> {
    let askpass = Askpass::new().map_err(|e| e.to_string())?;
    let mut cmd = ssh_add(paths);
    askpass.apply(&mut cmd, passphrase, "");
    if keychain && cfg!(target_os = "macos") {
        cmd.arg("--apple-use-keychain");
    }
    cmd.arg(key);
    run(cmd).map(|_| ()).map_err(|e| if askpass.dir.join("asked.bad").exists() { "Wrong passphrase".into() } else { friendly(&e) })
}

/// Unloads a key from the agent, by its public half.
pub fn agent_remove(paths: &Paths, key: &Key) -> Result<(), String> {
    // ssh-add -d reads the public key; give it one even when only the private file holds it.
    let scratch = Askpass::new().map_err(|e| e.to_string())?;
    let file = match (&key.public, &key.public_text) {
        (Some(p), _) => p.clone(),
        (None, Some(text)) => {
            let f = scratch.dir.join("key.pub");
            fs::write(&f, format!("{text}\n")).map_err(|e| e.to_string())?;
            f
        }
        (None, None) => key.path().to_path_buf(),
    };
    let mut cmd = ssh_add(paths);
    cmd.arg("-d").arg(file);
    run(cmd).map(|_| ()).map_err(|e| friendly(&e))
}

pub fn agent_clear(paths: &Paths) -> Result<(), String> {
    let mut cmd = ssh_add(paths);
    cmd.arg("-D");
    run(cmd).map(|_| ())
}

fn friendly(e: &str) -> String {
    if e.contains("incorrect passphrase") || e.contains("Bad passphrase") || e.contains("bad passphrase") {
        "Wrong passphrase".into()
    } else if e.contains("Could not open a connection") || e.contains("Error connecting to agent") {
        "No ssh-agent is running".into()
    } else {
        e.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyType {
    Ed25519,
    Rsa,
    Ecdsa,
}

impl KeyType {
    pub const ALL: [KeyType; 3] = [KeyType::Ed25519, KeyType::Rsa, KeyType::Ecdsa];

    pub fn label(self) -> &'static str {
        match self {
            KeyType::Ed25519 => "Ed25519",
            KeyType::Rsa => "RSA 4096",
            KeyType::Ecdsa => "ECDSA P-521",
        }
    }

    pub fn file(self) -> &'static str {
        match self {
            KeyType::Ed25519 => "id_ed25519",
            KeyType::Rsa => "id_rsa",
            KeyType::Ecdsa => "id_ecdsa",
        }
    }

    fn args(self) -> [&'static str; 4] {
        match self {
            KeyType::Ed25519 => ["-t", "ed25519", "-a", "100"],
            KeyType::Rsa => ["-t", "rsa", "-b", "4096"],
            KeyType::Ecdsa => ["-t", "ecdsa", "-b", "521"],
        }
    }
}

/// A file name for a key: no folders, nothing hidden, not a file ssh keeps there itself.
pub fn check_name(paths: &Paths, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the key a file name".into());
    }
    if name.contains('/') || name.starts_with('.') || name.ends_with(".pub") || skipped(name) {
        return Err("Use a plain file name, like id_ed25519_work".into());
    }
    let path = paths.ssh.join(name);
    if path.exists() || path.with_file_name(format!("{name}.pub")).exists() {
        return Err(format!("{} already exists", paths.tilde(&path)));
    }
    Ok(path)
}

/// Makes a key pair in `~/.ssh`. Returns the private key's path.
pub fn generate(paths: &Paths, ty: KeyType, name: &str, comment: &str, passphrase: &str) -> Result<PathBuf, String> {
    let path = check_name(paths, name)?;
    if !paths.ssh.exists() {
        fs::create_dir_all(&paths.ssh).map_err(|e| e.to_string())?;
        fsops::set_mode(&paths.ssh, 0o700).map_err(|e| e.to_string())?;
    }
    let askpass = Askpass::new().map_err(|e| e.to_string())?;
    let mut cmd = Command::new("ssh-keygen");
    cmd.args(ty.args()).arg("-q").arg("-C").arg(comment.trim()).arg("-f").arg(&path);
    if passphrase.is_empty() {
        cmd.args(["-N", ""]);
    }
    askpass.apply(&mut cmd, "", passphrase);
    run(cmd)?;
    if !path.is_file() {
        return Err("ssh-keygen made no key".into());
    }
    Ok(path)
}

/// Changes a key's passphrase; an empty `new` removes it.
pub fn change_passphrase(key: &Path, old: &str, new: &str) -> Result<(), String> {
    let askpass = Askpass::new().map_err(|e| e.to_string())?;
    let mut cmd = Command::new("ssh-keygen");
    cmd.arg("-p").arg("-f").arg(key);
    if new.is_empty() {
        cmd.args(["-N", ""]);
    }
    askpass.apply(&mut cmd, old, new);
    run(cmd).map(|_| ()).map_err(|e| friendly(&e))
}

/// Changes the comment in a key (and rewrites its `.pub`).
pub fn change_comment(key: &Path, passphrase: &str, comment: &str) -> Result<(), String> {
    let askpass = Askpass::new().map_err(|e| e.to_string())?;
    let mut cmd = Command::new("ssh-keygen");
    cmd.arg("-c").arg("-C").arg(comment.trim()).arg("-f").arg(key);
    askpass.apply(&mut cmd, passphrase, "");
    run(cmd).map(|_| ()).map_err(|e| friendly(&e))
}

/// Writes the missing `.pub` beside a private key.
pub fn write_public(key: &Key) -> Result<PathBuf, String> {
    let (Some(private), Some(text)) = (&key.private, &key.public_text) else { return Err("The public half isn't readable without the passphrase".into()) };
    let path = private.with_file_name(format!("{}.pub", key.name));
    fs::OpenOptions::new().write(true).create_new(true).mode(0o644).open(&path).and_then(|mut f| f.write_all(format!("{text}\n").as_bytes())).map_err(|e| e.to_string())?;
    Ok(path)
}

/// ssh's own rules: the private key 0600, the folder 0700.
pub fn fix_permissions(paths: &Paths, key: &Key) -> Result<(), String> {
    if let Some(p) = &key.private {
        fsops::set_mode(p, 0o600).map_err(|e| e.to_string())?;
    }
    if let Some(p) = &key.public {
        fsops::set_mode(p, 0o644).map_err(|e| e.to_string())?;
    }
    fsops::set_mode(&paths.ssh, 0o700).map_err(|e| e.to_string())
}

/// Copies a key from elsewhere into `~/.ssh` (with its `.pub` when it has one), private.
pub fn import(paths: &Paths, from: &Path) -> Result<PathBuf, String> {
    let text = fs::read_to_string(from).map_err(|e| e.to_string())?;
    if private_pem(&text).is_none() {
        return Err("That file isn't a private key".into());
    }
    let name = from.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let to = check_name(paths, &name)?;
    if !paths.ssh.exists() {
        fs::create_dir_all(&paths.ssh).map_err(|e| e.to_string())?;
        fsops::set_mode(&paths.ssh, 0o700).map_err(|e| e.to_string())?;
    }
    fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&to).and_then(|mut f| f.write_all(text.as_bytes())).map_err(|e| e.to_string())?;
    let pub_from = from.with_file_name(format!("{name}.pub"));
    if let Ok(p) = fs::read_to_string(&pub_from) {
        let _ = fs::OpenOptions::new().write(true).create_new(true).mode(0o644).open(to.with_file_name(format!("{name}.pub"))).and_then(|mut f| f.write_all(p.as_bytes()));
    }
    Ok(to)
}

/// Moves both halves to the trash. Returns (where each went, where it was).
pub fn trash(paths: &Paths, key: &Key) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut moved = Vec::new();
    for p in [&key.private, &key.public].into_iter().flatten() {
        let to = fsops::trash(p, paths).map_err(|e| format!("Couldn't move {} to the Trash: {e}", paths.tilde(p)))?;
        moved.push((to, p.clone()));
    }
    Ok(moved)
}

#[cfg(test)]
#[path = "keys_tests.rs"]
pub(crate) mod tests;
