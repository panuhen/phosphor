use anyhow::{Context, Result};
use git2::{Repository, StatusOptions};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub is_clean: bool,
    pub ahead: usize,
    pub behind: usize,
    pub modified: usize,
    pub staged: usize,
    pub untracked: usize,
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub time: i64,
    pub repo_name: String,
}

pub struct GitTracker {
    repos: Vec<PathBuf>,
}

impl GitTracker {
    pub fn new(repo_paths: &[String]) -> Self {
        let repos = repo_paths
            .iter()
            .map(|p| {
                let expanded = shellexpand::tilde(p);
                PathBuf::from(expanded.as_ref())
            })
            .collect();

        Self { repos }
    }

    pub fn get_status(&self) -> Result<Vec<RepoStatus>> {
        let mut statuses = Vec::new();

        for path in &self.repos {
            if let Ok(status) = self.get_repo_status(path) {
                statuses.push(status);
            }
        }

        Ok(statuses)
    }

    fn get_repo_status(&self, path: &PathBuf) -> Result<RepoStatus> {
        let repo = Repository::open(path)
            .with_context(|| format!("Failed to open repository: {}", path.display()))?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let head = repo.head()?;
        let branch = head
            .shorthand()
            .unwrap_or("HEAD")
            .to_string();

        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses_list = repo.statuses(Some(&mut opts))?;

        let mut modified = 0;
        let mut staged = 0;
        let mut untracked = 0;

        for entry in statuses_list.iter() {
            let status = entry.status();
            if status.is_wt_modified() || status.is_wt_deleted() || status.is_wt_renamed() {
                modified += 1;
            }
            if status.is_index_new() || status.is_index_modified() || status.is_index_deleted() {
                staged += 1;
            }
            if status.is_wt_new() {
                untracked += 1;
            }
        }

        let is_clean = modified == 0 && staged == 0 && untracked == 0;

        let (ahead, behind) = self.get_ahead_behind(&repo)?;

        Ok(RepoStatus {
            name,
            path: path.clone(),
            branch,
            is_clean,
            ahead,
            behind,
            modified,
            staged,
            untracked,
        })
    }

    fn get_ahead_behind(&self, repo: &Repository) -> Result<(usize, usize)> {
        let head = match repo.head() {
            Ok(h) => h,
            Err(_) => return Ok((0, 0)),
        };

        let local_oid = match head.target() {
            Some(oid) => oid,
            None => return Ok((0, 0)),
        };

        let branch_name = match head.shorthand() {
            Some(name) => name,
            None => return Ok((0, 0)),
        };

        let upstream_name = format!("refs/remotes/origin/{}", branch_name);
        let upstream = match repo.find_reference(&upstream_name) {
            Ok(r) => r,
            Err(_) => return Ok((0, 0)),
        };

        let upstream_oid = match upstream.target() {
            Some(oid) => oid,
            None => return Ok((0, 0)),
        };

        let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
        Ok((ahead, behind))
    }

    pub fn get_recent_commits(&self, max_commits: usize) -> Result<Vec<CommitInfo>> {
        let mut all_commits = Vec::new();

        for path in &self.repos {
            if let Ok(commits) = self.get_repo_commits(path, max_commits) {
                all_commits.extend(commits);
            }
        }

        // Sort by time descending
        all_commits.sort_by(|a, b| b.time.cmp(&a.time));
        all_commits.truncate(max_commits);

        Ok(all_commits)
    }

    fn get_repo_commits(&self, path: &PathBuf, max: usize) -> Result<Vec<CommitInfo>> {
        let repo = Repository::open(path)?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;

        let repo_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let commits: Vec<CommitInfo> = revwalk
            .take(max)
            .filter_map(|oid| oid.ok())
            .filter_map(|oid| repo.find_commit(oid).ok())
            .map(|commit| {
                let message = commit
                    .message()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();

                CommitInfo {
                    hash: commit.id().to_string(),
                    message,
                    author: commit.author().name().unwrap_or("Unknown").to_string(),
                    time: commit.time().seconds(),
                    repo_name: repo_name.clone(),
                }
            })
            .collect();

        Ok(commits)
    }
}
