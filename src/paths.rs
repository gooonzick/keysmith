//! Where things live: home, `~/.ssh`, its config, the trash and the agent's socket. Tests
//! point all of them into a throwaway folder with [`Paths::isolated`].

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub home: PathBuf,
    /// `~/.ssh`.
    pub ssh: PathBuf,
    /// `~/.ssh/config`.
    pub config: PathBuf,
    /// Where removed keys go.
    pub trash: PathBuf,
    /// The ssh-agent socket to talk to; `None` uses `SSH_AUTH_SOCK` as the app got it.
    pub agent_sock: Option<PathBuf>,
    /// The host name new keys are commented with: `user@host`.
    pub user_at_host: String,
}

impl Paths {
    pub fn from_env() -> Paths {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let ssh = home.join(".ssh");
        let trash = if cfg!(target_os = "macos") { home.join(".Trash") } else { dirs::data_dir().unwrap_or_else(|| home.join(".local/share")).join("Trash") };
        Paths { config: ssh.join("config"), ssh, trash, home, agent_sock: None, user_at_host: user_at_host() }
    }

    /// Everything under `root`, with an agent socket there too (nothing listens unless a test
    /// starts an agent on it), so a test never touches the real `~/.ssh` or agent.
    pub fn isolated(root: &Path) -> Paths {
        let ssh = root.join(".ssh");
        Paths {
            home: root.to_path_buf(),
            config: ssh.join("config"),
            ssh,
            trash: root.join(".Trash"),
            agent_sock: Some(root.join("agent.sock")),
            user_at_host: "me@test".into(),
        }
    }

    /// `path` with the home folder written as `~`.
    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// A path as ssh_config writes one: `~` and `%d` are home, relative paths are in `~/.ssh`.
    pub fn expand(&self, value: &str) -> PathBuf {
        let v = unquote(value);
        let v = v.replace("%d", &self.home.to_string_lossy()).replace("%%", "%");
        if v == "~" {
            return self.home.clone();
        }
        if let Some(rest) = v.strip_prefix("~/") {
            return self.home.join(rest);
        }
        let p = PathBuf::from(&v);
        if p.is_absolute() { p } else { self.ssh.join(p) }
    }

    /// How a config refers to a file: `~/…` under home, the full path elsewhere.
    pub fn config_path(&self, path: &Path) -> String {
        let s = self.tilde(path);
        if s.contains(char::is_whitespace) { format!("\"{s}\"") } else { s }
    }
}

/// A value without the double quotes around it.
pub fn unquote(value: &str) -> String {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') { v[1..v.len() - 1].to_string() } else { v.to_string() }
}

fn user_at_host() -> String {
    let user = std::env::var("USER").unwrap_or_else(|_| "me".into());
    let host = std::process::Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".into());
    format!("{user}@{host}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_follows_ssh_config_rules() {
        let p = Paths::isolated(Path::new("/h"));
        assert_eq!(p.expand("~/.ssh/id_ed25519"), PathBuf::from("/h/.ssh/id_ed25519"));
        assert_eq!(p.expand("\"~/My Keys/k\""), PathBuf::from("/h/My Keys/k"));
        assert_eq!(p.expand("%d/.ssh/k"), PathBuf::from("/h/.ssh/k"));
        assert_eq!(p.expand("id_rsa"), PathBuf::from("/h/.ssh/id_rsa"));
        assert_eq!(p.expand("/etc/k"), PathBuf::from("/etc/k"));
        assert_eq!(p.tilde(Path::new("/h/.ssh/k")), "~/.ssh/k");
        assert_eq!(p.config_path(Path::new("/h/a b/k")), "\"~/a b/k\"");
    }
}
