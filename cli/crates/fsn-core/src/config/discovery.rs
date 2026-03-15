// Config file discovery — find project and host TOML files by scanning
// the FSN directory layout.
//
// These helpers are used by multiple CLI commands (deploy, config, init, sync).
// They are pure filesystem operations: no I/O beyond directory iteration.

use std::path::{Path, PathBuf};

use crate::config::HostConfig;

/// Find the project config file.
///
/// If `explicit` is provided it is returned as-is.
/// Otherwise scans `{root}/projects/**/*.project.toml` and returns the
/// first match.
pub fn find_project(root: &Path, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    scan_project_files(root)
        .find(|p| p.to_string_lossy().ends_with(".project.toml"))
}

/// Find the host config file.
///
/// Search order:
///   1. `{root}/projects/**/*.host.toml`   (TUI layout)
///   2. `{root}/hosts/*.host.toml`          (legacy layout)
///
/// `example.host.toml` files are always ignored.
pub fn find_host(root: &Path) -> Option<PathBuf> {
    find_host_by(root, |name| is_real_host_toml(name))
}

/// Find a host config file whose `[host].name` field (or filename prefix)
/// matches `host_name`.
pub fn find_host_by_name(root: &Path, host_name: &str) -> Option<PathBuf> {
    // 1. Projects tree
    let projects_dir = root.join("projects");
    for proj_dir in read_subdirs(&projects_dir) {
        for path in read_dir_files(&proj_dir) {
            let fname = file_name(&path);
            if !is_real_host_toml(fname) {
                continue;
            }
            if fname.starts_with(&format!("{host_name}.")) {
                return Some(path);
            }
            // Also match against the [host].name field inside the file
            if let Ok(h) = HostConfig::load(&path) {
                if h.host.name() == host_name {
                    return Some(path);
                }
            }
        }
    }

    // 2. Legacy hosts/ directory
    let hosts_dir = root.join("hosts");
    for path in read_dir_files(&hosts_dir) {
        let fname = file_name(&path);
        if is_real_host_toml(fname) && fname.starts_with(&format!("{host_name}.")) {
            return Some(path);
        }
    }

    None
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Return `true` for `*.host.toml` files that are not the example template.
fn is_real_host_toml(fname: &str) -> bool {
    fname.ends_with(".host.toml") && fname != "example.host.toml"
}

/// Walk `{root}/projects/**/` and return the first path that matches `pred`.
fn find_host_by<F>(root: &Path, pred: F) -> Option<PathBuf>
where
    F: Fn(&str) -> bool,
{
    // 1. Projects tree
    let projects_dir = root.join("projects");
    for proj_dir in read_subdirs(&projects_dir) {
        if let Some(path) = read_dir_files(&proj_dir).find(|p| pred(file_name(p))) {
            return Some(path);
        }
    }

    // 2. Legacy hosts/ directory
    let hosts_dir = root.join("hosts");
    read_dir_files(&hosts_dir).find(|p| pred(file_name(p)))
}

/// Iterate over immediate sub-directories of `dir`.
fn read_subdirs(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
}

/// Iterate over files directly inside `dir` (non-recursive).
fn read_dir_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
}

/// Walk `{root}/projects/**` and yield all regular files.
fn scan_project_files(root: &Path) -> impl Iterator<Item = PathBuf> {
    read_subdirs(&root.join("projects"))
        .flat_map(|d| read_dir_files(&d).collect::<Vec<_>>())
}

/// Extract the file name as `&str` (empty string on failure).
fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}
