//! What a project path is: one repo, a directory of repos, or a repo in one.
//!
//! A *meta* directory (for example `~/workspaces/ai`) holds many repos and is
//! still one project to the user — one Bridge, many subdirectories. The rules
//! here only classify; nothing is cached yet.

use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Repo,
    Meta,
    RepoInMeta,
}

/// The rule table, pure so it is testable without a filesystem.
///
/// A `.git` at the top always wins: a submodule or vendored repo inside your
/// project does not turn it into a fleet. Anything unreadable is one project.
pub fn classify(is_git_root: bool, child_repos: usize, parent_is_meta: bool) -> ProjectKind {
    if is_git_root && parent_is_meta {
        ProjectKind::RepoInMeta
    } else if is_git_root {
        ProjectKind::Repo
    } else if child_repos > 0 {
        ProjectKind::Meta
    } else {
        ProjectKind::Repo
    }
}

/// A git root is a directory where `.git` is a directory **or a file**:
/// worktrees and submodules use a file.
fn is_git_root(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Immediate child directories that are git roots. Errors count as none.
fn child_repos(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir() && is_git_root(&e.path()))
        .count()
}

/// Classify a path as one repo, a meta directory of repos, or a repo in one.
///
/// **Windows**: cards there carry WSL paths the Windows process cannot read,
/// so the filesystem answers "nothing" and the rule above makes that `Repo`,
/// the common case, instead of mislabelling every single-repo card `Meta`.
/// Correctly classifying unreadable paths is an open item.
pub fn project_kind(path: &Path) -> ProjectKind {
    let is_root = is_git_root(path);
    let children = child_repos(path);
    let parent_is_meta = path.parent().is_some_and(|p| !is_git_root(p) && child_repos(p) >= 2);
    classify(is_root, children, parent_is_meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_follows_the_rules() {
        assert_eq!(classify(true, 0, false), ProjectKind::Repo);
        assert_eq!(classify(true, 0, true), ProjectKind::RepoInMeta);
        // A repo with vendored repos inside is still one project.
        assert_eq!(classify(true, 3, false), ProjectKind::Repo);
        assert_eq!(classify(false, 3, false), ProjectKind::Meta);
        // A plain or unreadable directory is one project, not a fleet.
        assert_eq!(classify(false, 0, false), ProjectKind::Repo);
    }

    #[test]
    fn a_fleet_of_repos_is_a_meta_project() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["svc-a", "svc-b"] {
            std::fs::create_dir_all(dir.path().join(name).join(".git")).unwrap();
        }
        assert_eq!(project_kind(dir.path()), ProjectKind::Meta);
        assert_eq!(project_kind(&dir.path().join("svc-a")), ProjectKind::RepoInMeta);
        // Totality: missing path and a file are treated as a single project.
        assert_eq!(project_kind(&dir.path().join("missing")), ProjectKind::Repo);
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(project_kind(&file), ProjectKind::Repo);
    }
}
