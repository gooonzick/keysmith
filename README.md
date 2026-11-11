<div align="center">

<img src="assets/icon.svg" width="128" alt="Keysmith icon">

# Keysmith

**Your SSH config and SSH keys in one native window.**

A desktop app for macOS and Linux, written in Rust on [Rune](https://github.com/anomalyco/rune) (the same stack as Prism).

</div>

## What it does

**Hosts (`~/.ssh/config`)**

- Every `Host` and `Match` section, from the config and every file it `Include`s, with a filter.
- Add, edit, duplicate, remove and reorder hosts. The form has fields for the usual options (HostName, User, Port, IdentityFile, ProxyJump) and rows for any other option.
- **Edits happen in place.** Only the lines that change are rewritten: comments, blank lines, indentation, key case and every other section stay as you wrote them. Comment lines right above a `Host` are its description.
- New hosts go above a closing `Host *`, because ssh takes the first value it finds.
- Each write is atomic, follows symlinks (a dotfiles repo keeps its link), keeps the file's mode and saves the old text to `config.keysmith.bak`. ⌘Z undoes the last change.
- For each host the inspector shows its keys (missing, loaded in the agent), what `ssh -G` resolves it to, and buttons for Copy command, Open in Terminal and a no-prompt **Test connection**.

**Keys (`~/.ssh`)**

- Every key pair, with type, size, fingerprint, comment, whether it has a passphrase, whether ssh-agent holds it, and which hosts use it. A private key that others can read is flagged and can be fixed (0600).
- **New key** (Ed25519, RSA 4096, ECDSA P-521) through `ssh-keygen`, with a passphrase and an option to load it into the agent (and the macOS Keychain).
- Add to and remove from ssh-agent, remove all, change or remove a passphrase, edit the comment, copy the public key, write a missing `.pub`, import a key, show it in Finder, move it to the Trash (with Undo).
- **Passphrases never go on a command line**, where `ps` would show them: `ssh-keygen` and `ssh-add` ask for them through a private `SSH_ASKPASS` script that reads them from its environment and gives up after one wrong answer.

Keysmith watches `~/.ssh` and the included files, so changes from a terminal or an editor show up on their own.

## Install

macOS (Apple silicon), through Homebrew:

```bash
brew install --cask gooonzick/tap/keysmith
```

The app isn't notarized; the cask clears the download flag so it opens without a Gatekeeper prompt. Without Homebrew, download the zip from [Releases](https://github.com/gooonzick/keysmith/releases), move `Keysmith.app` to Applications and run `xattr -dr com.apple.quarantine /Applications/Keysmith.app` once.

## Run from source

```bash
cargo run --release
```

Build the macOS app (and copy it to `~/Applications` with `--install`):

```bash
scripts/bundle-macos.sh --install
```

Try it on sample data without touching your own `~/.ssh`:

```bash
scripts/demo-home.sh /tmp/ks-demo && KEYSMITH_HOME=/tmp/ks-demo cargo run
```

## Shortcuts

| Key | Does |
| --- | --- |
| ⌘N | Add a host / new key |
| ⌘F | Filter |
| ⌘R | Reload |
| ⌘Z | Undo the last change |
| ⌘1, ⌘2 | Hosts, Keys |
| ↑ ↓, Enter | Move in the list, edit the host |

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Tests run against a throwaway home and their own `ssh-agent`, never yours. `KEYSMITH_HOME=dir cargo run -- --snap out.png --scene keys` renders a scene headless (`hosts`, `host-form`, `add-host`, `keys`, `new-key`, `passphrase`, `remove`, and `ds`: every design-system block). Read [DESIGN.md](DESIGN.md) before any UI change.

| Module | Owns |
| --- | --- |
| `sshconfig` | Reading and editing ssh_config files line by line, includes |
| `keys` | Scanning keys, `ssh-keygen`, `ssh-add`, the askpass script |
| `fsops` | Atomic writes, backups, the trash |
| `app` | State and actions; `ui` only reads it |
| `ds` | The design system: tokens, type and colour roles, blocks; depends only on Rune and rune-kit |
| `ui` | The window, pages and dialogs, built from `ds` |
