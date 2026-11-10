//! Writing to disk safely: configs are replaced atomically (through a symlink, so a dotfiles
//! repo keeps its link), keys never lose their permissions, and nothing is deleted outright.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::paths::Paths;

/// The file a write lands in: a symlink's target, else the path.
pub fn real_target(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Replaces `path` with `text`: a temp file beside the target, synced, then renamed over it.
/// The file keeps its mode (0600 when new). `~/.ssh` is made 0700 if it isn't there.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let target = real_target(path);
    let dir = target.parent().unwrap_or(Path::new("/"));
    if !dir.exists() {
        fs::create_dir_all(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    let mode = fs::metadata(&target).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o600);
    let name = target.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let tmp = dir.join(format!(".{name}.keysmith-{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(mode).open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
        fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Writes `text` to `path` and keeps what was there in `<name>.keysmith.bak` beside it.
pub fn write_with_backup(path: &Path, text: &str) -> io::Result<()> {
    let target = real_target(path);
    if let Ok(old) = fs::read_to_string(&target) {
        let name = target.file_name().unwrap_or_default().to_string_lossy().into_owned();
        write_atomic(&target.with_file_name(format!("{name}.keysmith.bak")), &old)?;
    }
    write_atomic(path, text)
}

/// Moves `path` to the trash and returns where it went. On Linux the freedesktop trash gets an
/// info file, so a file manager can put it back.
pub fn trash(path: &Path, paths: &Paths) -> io::Result<PathBuf> {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let files = if cfg!(target_os = "macos") { paths.trash.clone() } else { paths.trash.join("files") };
    fs::create_dir_all(&files)?;
    let mut dst = files.join(&name);
    let mut n = 0;
    while fs::symlink_metadata(&dst).is_ok() {
        n += 1;
        dst = files.join(format!("{name} {n}"));
    }
    if !cfg!(target_os = "macos") {
        let info = paths.trash.join("info");
        fs::create_dir_all(&info)?;
        let body = format!("[Trash Info]\nPath={}\n", std::path::absolute(path)?.display());
        fs::write(info.join(format!("{}.trashinfo", dst.file_name().unwrap_or_default().to_string_lossy())), body)?;
    }
    match fs::rename(path, &dst) {
        Ok(()) => Ok(dst),
        // Another volume: copy, then remove.
        Err(_) => {
            fs::copy(path, &dst)?;
            fs::remove_file(path)?;
            Ok(dst)
        }
    }
}

/// The permission bits of `path`, if it is there.
pub fn mode(path: &Path) -> Option<u32> {
    fs::metadata(path).ok().map(|m| m.permissions().mode() & 0o777)
}

pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_keeps_mode_and_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("dotfiles/ssh_config");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "old\n").unwrap();
        set_mode(&real, 0o644).unwrap();
        let link = tmp.path().join(".ssh/config");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_with_backup(&link, "new\n").unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link stays a link");
        assert_eq!(fs::read_to_string(&real).unwrap(), "new\n");
        assert_eq!(mode(&real), Some(0o644));
        assert_eq!(fs::read_to_string(real.with_file_name("ssh_config.keysmith.bak")).unwrap(), "old\n");
    }

    #[test]
    fn new_files_are_private_and_trash_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::isolated(tmp.path());
        write_atomic(&paths.config, "Host a\n").unwrap();
        assert_eq!(mode(&paths.config), Some(0o600));
        assert_eq!(mode(&paths.ssh), Some(0o700));
        let a = trash(&paths.config, &paths).unwrap();
        write_atomic(&paths.config, "Host b\n").unwrap();
        let b = trash(&paths.config, &paths).unwrap();
        assert_ne!(a, b);
        assert_eq!(fs::read_to_string(b).unwrap(), "Host b\n");
        assert!(!paths.config.exists());
    }
}
