//! `ssh_config` files, read and edited in place: a change rewrites only the lines it has to,
//! so comments, blank lines, indentation, key case and the order of everything else survive.
//!
//! A file is lines. Options before the first `Host` or `Match` apply everywhere; each `Host` or
//! `Match` line starts a section that runs to the next one. Comment lines right above a
//! section's header (no blank line between) are its description.

use std::path::{Path, PathBuf};

use crate::paths::{Paths, unquote};

#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    /// As written: `HostName`, `hostname`…
    pub key: String,
    /// As written, quotes kept.
    pub value: String,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Host,
    Match,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub kind: Kind,
    /// What follows `Host` (patterns) or `Match` (criteria), as written.
    pub patterns: String,
    pub header: usize,
    /// The first line of the description above the header (`header` when there is none).
    pub doc_start: usize,
    /// The description, without the `#`.
    pub doc: Vec<String>,
    /// One past the section's last line: the next section's `doc_start`, or the end.
    pub end: usize,
    pub options: Vec<Opt>,
}

impl Section {
    /// The first value of `key` (any case), unquoted.
    pub fn get(&self, key: &str) -> Option<String> {
        self.options.iter().find(|o| o.key.eq_ignore_ascii_case(key)).map(|o| unquote(&o.value))
    }

    pub fn all(&self, key: &str) -> Vec<String> {
        self.options.iter().filter(|o| o.key.eq_ignore_ascii_case(key)).map(|o| unquote(&o.value)).collect()
    }

    /// A pattern with a wildcard or a negation matches many hosts: it sets defaults, nobody
    /// connects to it by name.
    pub fn is_pattern(&self) -> bool {
        self.kind == Kind::Match || self.patterns.split_whitespace().all(|p| p.contains(['*', '?', '!']))
    }

    /// The name to type after `ssh`: the first plain pattern.
    pub fn alias(&self) -> Option<&str> {
        if self.kind == Kind::Match {
            return None;
        }
        self.patterns.split_whitespace().find(|p| !p.contains(['*', '?', '!']))
    }

    /// One past the last option (or the header): where new options go.
    fn body_end(&self) -> usize {
        self.options.last().map_or(self.header + 1, |o| o.line + 1)
    }
}

/// A host as the form edits it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostSpec {
    pub kind: Option<Kind>,
    pub patterns: String,
    pub doc: Vec<String>,
    /// In order; a key may repeat (`IdentityFile`, `LocalForward`).
    pub options: Vec<(String, String)>,
}

#[cfg(test)]
impl HostSpec {
    pub fn of(s: &Section) -> HostSpec {
        HostSpec { kind: Some(s.kind), patterns: s.patterns.clone(), doc: s.doc.clone(), options: s.options.iter().map(|o| (o.key.clone(), o.value.clone())).collect() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub path: PathBuf,
    pub lines: Vec<String>,
    /// Options before the first section.
    pub global: Vec<Opt>,
    pub sections: Vec<Section>,
    /// The file didn't end in a newline.
    no_final_newline: bool,
}

/// A line as `key value`, `key=value` or `key = value`; `None` for blanks and comments.
pub fn split_option(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let end = t.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(t.len());
    let key = t[..end].to_string();
    let rest = t[end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
    Some((key, rest.to_string()))
}

fn comment_text(line: &str) -> Option<String> {
    let t = line.trim();
    t.strip_prefix('#').map(|c| c.strip_prefix(' ').unwrap_or(c).trim_end().to_string())
}

fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// A key is letters and digits; ssh rejects anything else.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Every ssh_config keyword as the manual spells it (`ssh -G` prints them in lower case).
const KEYWORDS: &[&str] = &[
    "Host", "Match", "Include", "AddKeysToAgent", "AddressFamily", "BatchMode", "BindAddress", "BindInterface", "CanonicalDomains", "CanonicalizeFallbackLocal",
    "CanonicalizeHostname", "CanonicalizeMaxDots", "CanonicalizePermittedCNAMEs", "CASignatureAlgorithms", "CertificateFile", "ChannelTimeout", "CheckHostIP",
    "Ciphers", "ClearAllForwardings", "Compression", "ConnectionAttempts", "ConnectTimeout", "ControlMaster", "ControlPath", "ControlPersist", "DynamicForward",
    "EnableEscapeCommandline", "EnableSSHKeysign", "EscapeChar", "ExitOnForwardFailure", "FingerprintHash", "ForkAfterAuthentication", "ForwardAgent",
    "ForwardX11", "ForwardX11Timeout", "ForwardX11Trusted", "GatewayPorts", "GlobalKnownHostsFile", "GSSAPIAuthentication", "GSSAPIDelegateCredentials",
    "HashKnownHosts", "HostbasedAcceptedAlgorithms", "HostbasedAuthentication", "HostKeyAlgorithms", "HostKeyAlias", "HostName", "IdentitiesOnly",
    "IdentityAgent", "IdentityFile", "IgnoreUnknown", "IPQoS", "KbdInteractiveAuthentication", "KbdInteractiveDevices", "KexAlgorithms", "KnownHostsCommand",
    "LocalCommand", "LocalForward", "LogLevel", "LogVerbose", "MACs", "NoHostAuthenticationForLocalhost", "NoHostAuthenticationForProxyCommand",
    "NumberOfPasswordPrompts", "ObscureKeystrokeTiming", "PasswordAuthentication", "PermitLocalCommand", "PermitRemoteOpen", "PKCS11Provider", "Port",
    "PreferredAuthentications", "ProxyCommand", "ProxyJump", "ProxyUseFdpass", "PubkeyAcceptedAlgorithms", "PubkeyAuthentication", "RefuseConnection",
    "RekeyLimit", "RemoteCommand", "RemoteForward", "RequestTTY", "RequiredRSASize", "RevokedHostKeys", "SecurityKeyProvider", "SendEnv", "ServerAliveCountMax",
    "ServerAliveInterval", "SessionType", "SetEnv", "StdinNull", "StreamLocalBindMask", "StreamLocalBindUnlink", "StrictHostKeyChecking", "SyslogFacility",
    "TCPKeepAlive", "Tunnel", "TunnelDevice", "UpdateHostKeys", "UseKeychain", "User", "UserKnownHostsFile", "VerifyHostKeyDNS", "VersionAddendum",
    "VisualHostKey", "WarnWeakCrypto", "XAuthLocation",
];

/// A keyword in the manual's spelling, whatever case it came in; unknown ones stay as they are.
pub fn canonical(key: &str) -> &str {
    KEYWORDS.iter().find(|k| k.eq_ignore_ascii_case(key)).copied().unwrap_or(key)
}

impl Document {
    pub fn empty(path: PathBuf) -> Document {
        Document { path, lines: Vec::new(), global: Vec::new(), sections: Vec::new(), no_final_newline: false }
    }

    pub fn parse(path: PathBuf, text: &str) -> Document {
        let mut lines: Vec<String> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect();
        let no_final_newline = !text.is_empty() && !text.ends_with('\n');
        if !no_final_newline {
            lines.pop();
        }
        let mut d = Document { path, lines, global: Vec::new(), sections: Vec::new(), no_final_newline };
        d.reindex();
        d
    }

    pub fn read(path: &Path) -> std::io::Result<Document> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Document::parse(path.to_path_buf(), &text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::empty(path.to_path_buf())),
            Err(e) => Err(e),
        }
    }

    fn reindex(&mut self) {
        self.global.clear();
        self.sections.clear();
        for (i, line) in self.lines.iter().enumerate() {
            let Some((key, value)) = split_option(line) else { continue };
            let kind = if key.eq_ignore_ascii_case("host") {
                Some(Kind::Host)
            } else if key.eq_ignore_ascii_case("match") {
                Some(Kind::Match)
            } else {
                None
            };
            match kind {
                Some(kind) => {
                    let mut doc_start = i;
                    while doc_start > 0 && comment_text(&self.lines[doc_start - 1]).is_some() {
                        doc_start -= 1;
                    }
                    // A comment directly above belongs to this section, not the one before.
                    let floor = self.sections.last().map_or(0, |s: &Section| s.body_end());
                    let doc_start = doc_start.max(floor);
                    if let Some(prev) = self.sections.last_mut() {
                        prev.end = doc_start;
                    }
                    let doc = self.lines[doc_start..i].iter().filter_map(|l| comment_text(l)).collect();
                    self.sections.push(Section { kind, patterns: value, header: i, doc_start, doc, end: self.lines.len(), options: Vec::new() });
                }
                None => {
                    let opt = Opt { key, value, line: i };
                    match self.sections.last_mut() {
                        Some(s) => s.options.push(opt),
                        None => self.global.push(opt),
                    }
                }
            }
        }
        if let Some(last) = self.sections.last_mut() {
            last.end = self.lines.len();
        }
    }

    pub fn text(&self) -> String {
        let mut t = self.lines.join("\n");
        if !self.no_final_newline && !self.lines.is_empty() {
            t.push('\n');
        }
        t
    }

    /// Every `Include` in the file, global or in a section, each pattern on its own.
    pub fn includes(&self) -> Vec<String> {
        self.global.iter().chain(self.sections.iter().flat_map(|s| &s.options)).filter(|o| o.key.eq_ignore_ascii_case("include")).flat_map(|o| o.value.split_whitespace().map(str::to_string).collect::<Vec<_>>()).collect()
    }

    fn header_line(kind: Kind, patterns: &str) -> String {
        format!("{} {}", if kind == Kind::Match { "Match" } else { "Host" }, patterns.trim())
    }

    /// The lines a new section is written as.
    fn render(spec: &HostSpec, indent: &str) -> Vec<String> {
        let mut out: Vec<String> = spec.doc.iter().map(|d| if d.is_empty() { "#".to_string() } else { format!("# {d}") }).collect();
        out.push(Document::header_line(spec.kind.unwrap_or(Kind::Host), &spec.patterns));
        out.extend(spec.options.iter().map(|(k, v)| format!("{indent}{k} {}", v.trim())));
        out
    }

    /// Adds a section: before a closing `Host *` (it sets defaults, and ssh takes the first
    /// value it finds, so hosts go above it), else at the end. Returns its index.
    pub fn add(&mut self, spec: &HostSpec) -> usize {
        let indent = self.indent();
        let last_star = self.sections.last().filter(|s| s.kind == Kind::Host && s.patterns.trim() == "*").map(|s| s.doc_start);
        let at = last_star.unwrap_or(self.lines.len());
        let mut block = Document::render(spec, &indent);
        if at > 0 && !self.lines[at - 1].trim().is_empty() {
            block.insert(0, String::new());
        }
        if at < self.lines.len() && !self.lines[at].trim().is_empty() {
            block.push(String::new());
        }
        self.lines.splice(at..at, block);
        self.reindex();
        self.sections.iter().position(|s| s.header >= at).unwrap_or(self.sections.len() - 1)
    }

    /// The indentation the file uses for options, four spaces when it has none yet.
    fn indent(&self) -> String {
        self.sections.iter().flat_map(|s| &s.options).map(|o| indent_of(&self.lines[o.line]).to_string()).find(|i| !i.is_empty()).unwrap_or_else(|| "    ".into())
    }

    /// Rewrites section `i` as `spec`. Lines whose option keeps its value stay as they are;
    /// a changed value keeps the line's indentation and key; comments and blanks stay put; a
    /// new option goes after the last one. The `n`-th `IdentityFile` in `spec` replaces the
    /// `n`-th one in the file, and so on for every key.
    pub fn replace(&mut self, i: usize, spec: &HostSpec) {
        let s = self.sections[i].clone();
        let indent = s.options.first().map_or_else(|| self.indent(), |o| indent_of(&self.lines[o.line]).to_string());
        let mut used = vec![false; spec.options.len()];
        let mut seen: Vec<(String, usize)> = Vec::new();
        let mut body: Vec<String> = Vec::new();
        for line in &self.lines[s.header + 1..s.body_end()] {
            let Some((key, value)) = split_option(line) else {
                body.push(line.clone());
                continue;
            };
            let lower = key.to_ascii_lowercase();
            let n = match seen.iter_mut().find(|(k, _)| *k == lower) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    seen.push((lower.clone(), 0));
                    0
                }
            };
            let pick = spec.options.iter().enumerate().filter(|(_, (k, _))| k.eq_ignore_ascii_case(&key)).nth(n).map(|(j, (_, v))| (j, v.trim()));
            // An option the spec doesn't have any more is dropped.
            if let Some((j, v)) = pick {
                used[j] = true;
                body.push(if v == value { line.clone() } else { format!("{}{key} {v}", indent_of(line)) });
            }
        }
        // New options go after the last option line, ahead of trailing comments.
        let last_opt = body.iter().rposition(|l| split_option(l).is_some()).map_or(0, |p| p + 1);
        let added: Vec<String> = spec.options.iter().zip(&used).filter(|(_, u)| !**u).map(|((k, v), _)| format!("{indent}{k} {}", v.trim())).collect();
        body.splice(last_opt..last_opt, added);

        let header = if spec.patterns.trim() == s.patterns.trim() {
            self.lines[s.header].clone()
        } else {
            let line = &self.lines[s.header];
            let (key, _) = split_option(line).expect("a header is an option line");
            format!("{}{key} {}", indent_of(line), spec.patterns.trim())
        };
        let doc: Vec<String> = if spec.doc == s.doc {
            self.lines[s.doc_start..s.header].to_vec()
        } else {
            spec.doc.iter().map(|d| if d.is_empty() { "#".to_string() } else { format!("# {d}") }).collect()
        };
        let mut block = doc;
        block.push(header);
        block.extend(body);
        self.lines.splice(s.doc_start..s.body_end(), block);
        self.reindex();
    }

    /// Removes section `i` with its description. A blank line it leaves doubled is dropped.
    pub fn remove(&mut self, i: usize) {
        let s = self.sections[i].clone();
        let mut end = s.body_end();
        // Its blank lines go too, unless they separate the two around it.
        while end < s.end && self.lines[end].trim().is_empty() {
            end += 1;
        }
        let keep_gap = s.doc_start > 0 && !self.lines[s.doc_start - 1].trim().is_empty() && end < self.lines.len();
        let mut tail = end;
        if keep_gap && tail > s.body_end() {
            tail -= 1;
        }
        self.lines.drain(s.doc_start..tail);
        self.reindex();
    }

    /// Swaps two sections' content (description, header, options). What lies between and
    /// around them (blank lines, loose comments) stays where it is.
    pub fn swap(&mut self, i: usize, j: usize) {
        let (a, b) = (i.min(j), i.max(j));
        if b >= self.sections.len() || a == b {
            return;
        }
        let (sa, sb) = (self.sections[a].clone(), self.sections[b].clone());
        let mut block: Vec<String> = self.lines[sb.doc_start..sb.body_end()].to_vec();
        block.extend_from_slice(&self.lines[sa.body_end()..sb.doc_start]);
        block.extend_from_slice(&self.lines[sa.doc_start..sa.body_end()]);
        self.lines.splice(sa.doc_start..sb.body_end(), block);
        self.reindex();
    }
}

/// The config and every file it includes, each once, includes in the order ssh reads them.
pub fn load(paths: &Paths) -> (Vec<Document>, Vec<(PathBuf, String)>) {
    let mut docs = Vec::new();
    let mut errors = Vec::new();
    let mut queue = vec![(paths.config.clone(), 0usize)];
    while let Some((path, depth)) = queue.pop() {
        if docs.iter().any(|d: &Document| d.path == path) {
            continue;
        }
        match Document::read(&path) {
            Ok(d) => {
                if depth < 8 {
                    let mut found = Vec::new();
                    for inc in d.includes() {
                        found.extend(resolve_include(&inc, paths));
                    }
                    // A stack: push in reverse so the first include is read next.
                    queue.extend(found.into_iter().rev().map(|p| (p, depth + 1)));
                }
                docs.push(d);
            }
            Err(e) => errors.push((path, e.to_string())),
        }
    }
    (docs, errors)
}

/// The files an `Include` names: `~` and relative paths as ssh reads them, wildcards in the
/// file name only, sorted like a shell glob.
pub fn resolve_include(pattern: &str, paths: &Paths) -> Vec<PathBuf> {
    let full = paths.expand(pattern);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !name.contains(['*', '?']) {
        return if full.is_file() { vec![full] } else { Vec::new() };
    }
    let Some(dir) = full.parent() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = rd.flatten().filter(|e| e.path().is_file()).filter(|e| glob(&name, &e.file_name().to_string_lossy())).map(|e| e.path()).collect();
    out.sort();
    out
}

/// `*` and `?` matching, as ssh_config patterns use them.
pub fn glob(pattern: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (pattern.chars().collect(), s.chars().collect());
    let (mut pi, mut si, mut star, mut mark) = (0, 0, None, 0);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
#[path = "sshconfig_tests.rs"]
mod tests;
