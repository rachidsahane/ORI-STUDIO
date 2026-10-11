//! The git hooks installer: AICD §13, AICD §23.
//!
//! `spec/PRD.md` Z-01 requires unattributed-change detection to cover "disk
//! edits, commits and pushes", and `spec/ARCHITECTURE.md` section 5.2 names the
//! channel: "git events (hooks installed in the repository, plus polling of
//! refs)". [`GitHooks`] installs four hooks, [`GitHook::PostCommit`],
//! [`GitHook::PostMerge`], [`GitHook::PostRewrite`] and [`GitHook::PrePush`],
//! each of which appends one line to a *spool* directory the engine owns;
//! [`HookSpool`] is how the engine reads those lines back. The ref poller in
//! `crate::watcher` is the detector that does not depend on a hook (a commit
//! made with `--no-verify` still moves a ref).
//!
//! ```mermaid
//! sequenceDiagram
//!   participant G as git
//!   participant H as hook script
//!   participant S as spool: hook-events.log
//!   participant E as engine: HookSpool::drain
//!   G->>H: post-commit, post-merge, post-rewrite, pre-push
//!   H->>S: append one record, or print an error on stderr
//!   H->>H: run the hook that was here before, if any
//!   E->>S: read new complete lines from its offset
//! ```
//!
//! # Is `.git/hooks/` "the tree"? No: the reading, and its source
//!
//! `spec/LLD.md` section 2 says `ori-watch` "Must not: modify the tree", and
//! `spec/ENV_SETUP.md` (the `git` row) says "Hooks installed by the watcher".
//! Both are binding, and they are consistent only if the hooks directory is
//! not "the tree". It is not, on three readings that agree:
//!
//! 1. Git's own definition (gitglossary(7)): the *working tree* is "the tree
//!    of actual checked out files"; the *repository* is the collection of refs
//!    and objects kept in `$GIT_DIR`. The hooks directory is "`$GIT_DIR/hooks`"
//!    (githooks(5)): repository configuration, untracked, never part of a
//!    commit, and invisible to `git status`.
//! 2. This crate's own watcher ignores `.git` entirely, because it is not an
//!    edit to the tree (`crate::watcher`, "What is ignored"). The installer
//!    could not write there, and the watcher not report it, if `.git` were the
//!    tree.
//! 3. The consequence the rule exists for: AICD §23's golden rule forbids a
//!    manual edit of the tracked tree, because it puts the system outside what
//!    the agents know. Writing a hook changes nothing a commit, a diff or a
//!    session can see.
//!
//! The reading is tested, not just stated: `tests::running::ori_t_0054_installing_hooks_leaves_the_working_tree_untouched`
//! installs into a real repository and compares every file outside `.git`.
//! The installer writes nothing else: not the working tree, not the git
//! configuration, not the index. The spool is outside the tree and is refused
//! if it is inside it ([`HooksError::SpoolInsideTree`]), since a hook that
//! wrote there would be an unattributed edit of its own making.
//!
//! # What is installed, and what happens to a hook already there
//!
//! A hook this module wrote carries a marker on its second line. A hook
//! without the marker belongs to someone else and is never overwritten:
//! it is renamed to `<hook>.ori-original` and the installed script runs it
//! after recording (same arguments, same standard input, and its exit status
//! is the hook's, so a `pre-push` that refuses a push still refuses it). The
//! report says so ([`InstallOutcome::InstalledPreserving`]). If
//! `<hook>.ori-original` already exists the installer refuses
//! ([`HooksError::PreservedNameTaken`]) rather than choose which of two hooks
//! to lose. Every refusal is decided for all four hooks before any file is
//! written, and each file is written to a temporary name and renamed into
//! place. Installing again changes nothing ([`InstallOutcome::Unchanged`]); a
//! marked hook whose text differs, a changed spool for example, is rewritten
//! ([`InstallOutcome::Updated`]).
//!
//! # A hook that cannot reach the engine says so (AICD §14)
//!
//! A hook that fails silently is the defect AICD §14 names, "present but
//! reporting nothing". If the spool directory is missing or cannot be
//! appended to, the hook prints an `ori-studio: ERROR` block on standard
//! error, which git shows to whoever ran the command. It then exits as the
//! preserved hook did, or zero if there was none: a hook cannot stop a commit
//! (git ignores a `post-*` hook's status), and blocking a *push* because the
//! engine's spool is unwritable would turn an observability fault into an
//! availability fault when the ref poller still sees the push as a moved
//! `refs/remotes/*` ref. [`GitHooks::status`] is the liveness check: it
//! reports which hooks are installed, stale, missing, replaced or not
//! executable.
//!
//! # Nothing from the repository is interpolated unquoted
//!
//! The generated script is a constant template with exactly two values
//! substituted: the hook's own name (one of four constants) and the spool
//! path, which the engine gives, never the repository. The spool path is
//! written inside single quotes with every embedded single quote closed,
//! escaped and reopened (`shell_quote`), the one quoting form that has no
//! other special character. Everything a hook *reads* at run time, branch
//! names, ref names, object ids and arguments, is data: each is passed to a
//! function as a quoted argument and reduced to a safe alphabet and 200 bytes
//! (`ori_clean`) before it is written, so a branch named to contain a tab, a
//! newline or `$(...)` cannot forge a second record or run anything. The
//! script never records a remote URL, only a remote's name, because a URL can
//! carry a credential (`CLAUDE.md` rule 4): a remote spelled as a URL is
//! recorded as `url-redacted`.
//!
//! # Blocking calls
//!
//! [`GitHooks::install`], [`GitHooks::status`] and [`HookSpool::drain`] do
//! blocking file I/O. The engine runs them with `spawn_blocking`
//! (`spec/CONVENTIONS.md`, "Rust").
//!
//! # What is not covered
//!
//! - `core.hooksPath` set in the repository's own `.git/config` makes git
//!   ignore `.git/hooks`; the installer refuses
//!   ([`HooksError::HooksPathConfigured`]) rather than install hooks that
//!   would never run. The same setting in a global or system configuration,
//!   in `GIT_CONFIG_*` variables or through an `include`, cannot be seen from
//!   here; the ref poller is the backstop, and [`GitHooks::status`] reports
//!   installed hooks, not that git will run them.
//! - A linked worktree's `.git` is a file; the installer refuses it
//!   ([`HooksError::GitDirNotADirectory`]). Install into the repository that
//!   owns the worktree: hooks live in the common directory and run for every
//!   worktree.
//! - The scripts are POSIX `sh`. Git for Windows runs them with its bundled
//!   shell; the executing tests below run on Unix only.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ori_core::error::MethodologyRef;
use ori_core::types::Timestamp;

/// The file in the spool directory the hooks append to.
pub const SPOOL_LOG_NAME: &str = "hook-events.log";

/// The marker on the second line of every script this module writes.
const MARKER: &str = "# ori-studio-hook v1";

/// The suffix a preserved hook gets.
const PRESERVED_SUFFIX: &str = ".ori-original";

/// The most of an existing hook or of `.git/config` that is read.
const READ_CAP: u64 = 1024 * 1024;

/// The most one [`HookSpool::drain`] reads, so a huge log is taken in turns.
pub const DRAIN_BYTES_CAP: usize = 1024 * 1024;

/// The most fields one record keeps.
const FIELD_CAP: usize = 8;

/// A git hook the engine installs: AICD §13.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GitHook {
    /// After `git commit` creates a commit.
    PostCommit,
    /// After `git merge` completes, which makes a commit `post-commit` does
    /// not see.
    PostMerge,
    /// After `git commit --amend` or `git rebase` rewrote commits.
    PostRewrite,
    /// Before `git push` sends anything: the only hook point a push has.
    PrePush,
}

impl GitHook {
    /// The four hooks, in the order they are installed.
    pub const ALL: [Self; 4] = [
        Self::PostCommit,
        Self::PostMerge,
        Self::PostRewrite,
        Self::PrePush,
    ];

    /// The file name git looks for: AICD §13.
    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::PostCommit => "post-commit",
            Self::PostMerge => "post-merge",
            Self::PostRewrite => "post-rewrite",
            Self::PrePush => "pre-push",
        }
    }

    fn from_file_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|hook| hook.file_name() == name)
    }

    /// Whether git feeds this hook lines on standard input.
    const fn reads_stdin(self) -> bool {
        matches!(self, Self::PostRewrite | Self::PrePush)
    }
}

/// Everything the installer and the spool reader refuse or cannot do:
/// AICD §14.
///
/// Mirrors `WatchError`: a control refusing an action carries a methodology
/// reason ([`HooksError::methodology_ref`]), an I/O failure does not.
#[derive(Debug)]
#[non_exhaustive]
pub enum HooksError {
    /// The repository root could not be resolved or is not a directory.
    Root {
        /// The path as given.
        path: PathBuf,
        /// What the operating system said.
        source: io::Error,
    },
    /// `.git` is a file or a link, so nothing is written through it.
    GitDirNotADirectory {
        /// The path of `.git`.
        path: PathBuf,
    },
    /// The spool path is relative, carries `..`, or is not UTF-8, so it names
    /// a different directory from a different working directory or cannot be
    /// written into a script faithfully.
    SpoolNotPlain {
        /// The path as given.
        path: PathBuf,
    },
    /// The spool directory is inside the working tree, so a hook's own record
    /// would be an edit of the tree.
    SpoolInsideTree {
        /// The spool, resolved.
        spool: PathBuf,
        /// The repository root, resolved.
        root: PathBuf,
    },
    /// `core.hooksPath` is set in the repository's configuration, so git
    /// would never run a hook installed in `.git/hooks`.
    HooksPathConfigured {
        /// The configured value, as data.
        value: String,
    },
    /// `.git/hooks` is a link or a file.
    HooksDirNotADirectory {
        /// The path.
        path: PathBuf,
    },
    /// A directory sits where a hook file should.
    HookNotAFile {
        /// The path.
        path: PathBuf,
    },
    /// A hook of someone else's is installed and its preserved name is
    /// already taken, so one of the two would be lost.
    PreservedNameTaken {
        /// The path of the existing preserved hook.
        path: PathBuf,
    },
    /// The spool log is a link or not a regular file, so it is not read.
    SpoolLogNotAFile {
        /// The path.
        path: PathBuf,
    },
    /// A file-system operation failed.
    Io {
        /// The path being operated on.
        path: PathBuf,
        /// What the operating system said.
        source: io::Error,
    },
}

impl HooksError {
    /// The methodology section a refusal rests on, for the refusals only.
    ///
    /// - [`HooksError::SpoolInsideTree`] cites AICD §23: a write into the
    ///   tree that no session made is the manual edit its golden rule forbids,
    ///   and the engine's own hook would be doing it.
    /// - [`HooksError::HooksPathConfigured`] cites AICD §14: hooks that git
    ///   will never run are installed and reporting nothing.
    /// - The rest cite AICD §13: git is the audit trail, and the installer
    ///   will not write through a link, over another actor's hook or into a
    ///   repository other than the one it was pointed at.
    ///
    /// I/O failures and a root that does not resolve are not controls
    /// refusing anything and cite nothing.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        let section = match self {
            Self::SpoolInsideTree { .. } => 23,
            Self::HooksPathConfigured { .. } => 14,
            Self::GitDirNotADirectory { .. }
            | Self::SpoolNotPlain { .. }
            | Self::HooksDirNotADirectory { .. }
            | Self::HookNotAFile { .. }
            | Self::PreservedNameTaken { .. }
            | Self::SpoolLogNotAFile { .. } => 13,
            Self::Root { .. } | Self::Io { .. } => return None,
        };
        Some(MethodologyRef {
            section,
            subsection: None,
        })
    }
}

impl fmt::Display for HooksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cite = |f: &mut fmt::Formatter<'_>, error: &Self| -> fmt::Result {
            match error.methodology_ref() {
                Some(reference) => write!(f, " ({reference})"),
                None => Ok(()),
            }
        };
        match self {
            Self::Root { path, source } => {
                write!(
                    f,
                    "cannot use {} as a repository root: {source}",
                    path.display()
                )
            }
            Self::GitDirNotADirectory { path } => write!(
                f,
                "refused: {} is a file or a link, not the repository's own directory; install \
                 into the repository that owns this worktree",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::SpoolNotPlain { path } => write!(
                f,
                "refused: the spool path must be absolute, UTF-8 and free of '..': {}",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::SpoolInsideTree { spool, root } => write!(
                f,
                "refused: the spool {} is inside the working tree {}, so a hook's own record \
                 would be an unattributed edit",
                spool.display(),
                root.display()
            )
            .and_then(|()| cite(f, self)),
            Self::HooksPathConfigured { value } => write!(
                f,
                "refused: core.hooksPath is set to {value:?} in the repository's configuration, \
                 so git would never run a hook installed in .git/hooks"
            )
            .and_then(|()| cite(f, self)),
            Self::HooksDirNotADirectory { path } => write!(
                f,
                "refused: {} is a link or not a directory; nothing is written through it",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::HookNotAFile { path } => write!(
                f,
                "refused: {} is a directory where a hook file belongs",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::PreservedNameTaken { path } => write!(
                f,
                "refused: another actor's hook is installed and {} already holds a preserved \
                 one; neither is overwritten",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::SpoolLogNotAFile { path } => write!(
                f,
                "refused: the spool log {} is a link or not a regular file and is not read",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl Error for HooksError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Root { source, .. } | Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// What happened to one hook during [`GitHooks::install`]: AICD §13.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallOutcome {
    /// Nothing was there; the hook was written.
    Installed,
    /// Another actor's hook was there. It was moved to `preserved` and the
    /// installed script runs it after recording.
    InstalledPreserving {
        /// Where the hook that was there before now lives.
        preserved: PathBuf,
    },
    /// The engine's own hook was there but differed (a changed spool, an
    /// older script, a lost executable bit) and was rewritten.
    Updated,
    /// The engine's own hook was already exactly this; nothing was written.
    Unchanged,
}

/// The result of [`GitHooks::install`], one outcome per hook: AICD §13.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallReport {
    /// The hooks in [`GitHook::ALL`] order.
    pub outcomes: Vec<(GitHook, InstallOutcome)>,
}

impl InstallReport {
    /// Whether the installation wrote nothing.
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.outcomes
            .iter()
            .all(|(_, outcome)| *outcome == InstallOutcome::Unchanged)
    }
}

/// How one hook stands: AICD §14.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookState {
    /// The engine's hook, exactly as it would be written now, and executable.
    Installed,
    /// The engine's hook, but its text differs from what would be written now.
    Stale,
    /// No hook of that name.
    Missing,
    /// Someone else's hook is there and the engine's is not.
    Foreign,
    /// The engine's hook, current, but git will not run it: no executable bit.
    NotExecutable,
}

/// The standing of one hook, from [`GitHooks::status`]: AICD §14.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookStatus {
    /// Which hook.
    pub hook: GitHook,
    /// How it stands.
    pub state: HookState,
    /// Whether a preserved hook from before the engine sits beside it and is
    /// run after recording.
    pub chains_to_original: bool,
}

/// The installer for one repository: AICD §13, AICD §23.
///
/// See the module doc for what is installed, what happens to a hook already
/// there, and why writing here is not modifying the tree.
#[derive(Clone, Debug)]
pub struct GitHooks {
    root: PathBuf,
    git_dir: PathBuf,
    spool: PathBuf,
}

impl GitHooks {
    /// Prepares an installer for the repository at `repo_root` whose hooks
    /// record into `spool_dir`: AICD §23.
    ///
    /// Nothing is written. The spool must be absolute, UTF-8, free of `..`,
    /// and not inside the repository's working tree; it need not exist yet.
    ///
    /// # Errors
    ///
    /// [`HooksError::Root`], [`HooksError::GitDirNotADirectory`],
    /// [`HooksError::SpoolNotPlain`], [`HooksError::SpoolInsideTree`] or
    /// [`HooksError::Io`].
    pub fn new(repo_root: &Path, spool_dir: &Path) -> Result<Self, HooksError> {
        let root = fs::canonicalize(repo_root).map_err(|source| HooksError::Root {
            path: repo_root.to_path_buf(),
            source,
        })?;
        if !root.is_dir() {
            return Err(HooksError::Root {
                path: root,
                source: io::Error::new(io::ErrorKind::NotADirectory, "not a directory"),
            });
        }
        let git_dir = root.join(".git");
        let metadata = fs::symlink_metadata(&git_dir).map_err(|source| HooksError::Io {
            path: git_dir.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(HooksError::GitDirNotADirectory { path: git_dir });
        }
        let plain = spool_dir.is_absolute()
            && spool_dir.to_str().is_some_and(|text| !text.contains('\0'))
            && spool_dir
                .components()
                .all(|c| !matches!(c, Component::ParentDir | Component::CurDir));
        if !plain {
            return Err(HooksError::SpoolNotPlain {
                path: spool_dir.to_path_buf(),
            });
        }
        let spool = resolve_leniently(spool_dir).map_err(|source| HooksError::Io {
            path: spool_dir.to_path_buf(),
            source,
        })?;
        if spool.starts_with(&root) {
            return Err(HooksError::SpoolInsideTree { spool, root });
        }
        Ok(Self {
            root,
            git_dir,
            spool,
        })
    }

    /// The canonical repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The resolved spool directory the hooks record into.
    #[must_use]
    pub fn spool(&self) -> &Path {
        &self.spool
    }

    fn hooks_dir(&self) -> PathBuf {
        self.git_dir.join("hooks")
    }

    /// The text of the script that would be installed for `hook`: AICD §13.
    #[must_use]
    pub fn script_for(&self, hook: GitHook) -> String {
        render_script(hook, self.spool_text())
    }

    fn spool_text(&self) -> &str {
        // `new` refused a spool that is not UTF-8, and canonicalising keeps it
        // so on every platform this builds for; the fallback is never written
        // because an empty spool would make every hook report loudly.
        self.spool.to_str().unwrap_or("")
    }

    /// Installs the four hooks, preserving any hook already there:
    /// AICD §13, AICD §23.
    ///
    /// Idempotent. Every refusal is decided before any file is written. Writes
    /// nothing in the working tree (module doc, "Is `.git/hooks/` the tree?").
    ///
    /// # Errors
    ///
    /// [`HooksError::HooksPathConfigured`],
    /// [`HooksError::HooksDirNotADirectory`], [`HooksError::HookNotAFile`],
    /// [`HooksError::PreservedNameTaken`] or [`HooksError::Io`].
    pub fn install(&self) -> Result<InstallReport, HooksError> {
        self.refuse_if_hooks_path_is_configured()?;
        let hooks_dir = self.hooks_dir();
        match fs::symlink_metadata(&hooks_dir) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(HooksError::HooksDirNotADirectory { path: hooks_dir });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(HooksError::Io {
                    path: hooks_dir,
                    source,
                });
            }
        }
        // Decide every hook before writing any.
        let mut plans = Vec::new();
        for hook in GitHook::ALL {
            plans.push((hook, self.plan(hook)?));
        }
        fs::create_dir_all(&self.spool).map_err(|source| HooksError::Io {
            path: self.spool.clone(),
            source,
        })?;
        fs::create_dir_all(&hooks_dir).map_err(|source| HooksError::Io {
            path: hooks_dir.clone(),
            source,
        })?;
        let mut outcomes = Vec::new();
        for (hook, plan) in plans {
            let path = hooks_dir.join(hook.file_name());
            let outcome = match plan {
                Plan::Keep => InstallOutcome::Unchanged,
                Plan::Write => {
                    self.write_script(hook, &path)?;
                    InstallOutcome::Installed
                }
                Plan::Rewrite => {
                    self.write_script(hook, &path)?;
                    InstallOutcome::Updated
                }
                Plan::Preserve => {
                    let preserved = preserved_path(&path);
                    fs::rename(&path, &preserved).map_err(|source| HooksError::Io {
                        path: path.clone(),
                        source,
                    })?;
                    self.write_script(hook, &path)?;
                    InstallOutcome::InstalledPreserving { preserved }
                }
            };
            outcomes.push((hook, outcome));
        }
        Ok(InstallReport { outcomes })
    }

    /// How each hook stands now: AICD §14.
    ///
    /// This is the liveness check for the hooks: it reads, never writes. It
    /// reports that the files are in place, not that git will run them (see
    /// the module doc's "What is not covered").
    ///
    /// # Errors
    ///
    /// [`HooksError::HookNotAFile`] when a directory sits where a hook
    /// belongs, [`HooksError::Io`] when a hook file cannot be examined.
    pub fn status(&self) -> Result<Vec<HookStatus>, HooksError> {
        let hooks_dir = self.hooks_dir();
        let mut statuses = Vec::new();
        for hook in GitHook::ALL {
            let path = hooks_dir.join(hook.file_name());
            let state = match self.examine(hook, &path)? {
                Examined::Absent => HookState::Missing,
                Examined::Ours {
                    current,
                    executable,
                } => match (current, executable) {
                    (true, true) => HookState::Installed,
                    (true, false) => HookState::NotExecutable,
                    (false, _) => HookState::Stale,
                },
                Examined::Foreign => HookState::Foreign,
                Examined::Directory => {
                    return Err(HooksError::HookNotAFile { path });
                }
            };
            let chains_to_original = fs::symlink_metadata(preserved_path(&path)).is_ok();
            statuses.push(HookStatus {
                hook,
                state,
                chains_to_original,
            });
        }
        Ok(statuses)
    }

    fn plan(&self, hook: GitHook) -> Result<Plan, HooksError> {
        let path = self.hooks_dir().join(hook.file_name());
        match self.examine(hook, &path)? {
            Examined::Absent => Ok(Plan::Write),
            Examined::Ours {
                current,
                executable,
            } => {
                if current && executable {
                    Ok(Plan::Keep)
                } else {
                    Ok(Plan::Rewrite)
                }
            }
            Examined::Foreign => {
                let preserved = preserved_path(&path);
                if fs::symlink_metadata(&preserved).is_ok() {
                    Err(HooksError::PreservedNameTaken { path: preserved })
                } else {
                    Ok(Plan::Preserve)
                }
            }
            Examined::Directory => Err(HooksError::HookNotAFile { path }),
        }
    }

    fn examine(&self, hook: GitHook, path: &Path) -> Result<Examined, HooksError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Examined::Absent),
            Err(source) => {
                return Err(HooksError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        if metadata.is_dir() {
            return Ok(Examined::Directory);
        }
        // A link is someone else's hook: it is moved, never written through.
        if !metadata.is_file() {
            return Ok(Examined::Foreign);
        }
        let (text, truncated) = read_text(path, READ_CAP).map_err(|source| HooksError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if truncated || !is_ours(&text) {
            return Ok(Examined::Foreign);
        }
        Ok(Examined::Ours {
            current: text == self.script_for(hook),
            executable: is_executable(&metadata),
        })
    }

    fn write_script(&self, hook: GitHook, path: &Path) -> Result<(), HooksError> {
        let temporary = self.hooks_dir().join(format!(
            ".{}.ori-tmp-{}",
            hook.file_name(),
            std::process::id()
        ));
        let script = self.script_for(hook);
        write_atomically(&temporary, path, script.as_bytes()).map_err(|source| {
            let _ = fs::remove_file(&temporary);
            HooksError::Io {
                path: path.to_path_buf(),
                source,
            }
        })
    }

    fn refuse_if_hooks_path_is_configured(&self) -> Result<(), HooksError> {
        let config = self.git_dir.join("config");
        let text = match read_text(&config, READ_CAP) {
            Ok((text, _)) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(HooksError::Io {
                    path: config,
                    source,
                });
            }
        };
        match configured_hooks_path(&text) {
            Some(value) => Err(HooksError::HooksPathConfigured { value }),
            None => Ok(()),
        }
    }
}

/// Writes `bytes` to `temporary` (which must not exist), makes it executable
/// and renames it over `path`, so a reader never sees half a script.
fn write_atomically(temporary: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    make_executable(temporary)?;
    fs::rename(temporary, path)
}

enum Plan {
    Keep,
    Write,
    Rewrite,
    Preserve,
}

enum Examined {
    Absent,
    Directory,
    Foreign,
    Ours { current: bool, executable: bool },
}

fn preserved_path(hook_path: &Path) -> PathBuf {
    let mut name = hook_path.as_os_str().to_os_string();
    name.push(PRESERVED_SUFFIX);
    PathBuf::from(name)
}

/// Whether `text` is a script this module wrote: AICD §13.
fn is_ours(text: &str) -> bool {
    text.lines()
        .nth(1)
        .is_some_and(|line| line.starts_with(MARKER))
}

/// The value of `core.hooksPath` in a git configuration file, if set.
///
/// A deliberately small reading of git's configuration syntax: section headers,
/// `key = value` lines, comments. It exists to refuse an installation that
/// could never run, so it errs towards refusing: any non-empty value counts.
fn configured_hooks_path(text: &str) -> Option<String> {
    let mut in_core = false;
    for raw in text.lines() {
        let mut line = raw.trim();
        if let Some(rest) = line.strip_prefix('[') {
            let Some((header, after)) = rest.split_once(']') else {
                in_core = false;
                continue;
            };
            in_core = header.trim().eq_ignore_ascii_case("core");
            line = after.trim();
        }
        if !in_core || line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("hookspath")
        {
            let value = value.trim().trim_matches('"').trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

/// Resolves `path` as far as it exists and appends the rest unchanged, so a
/// spool that is not yet created can still be judged against the tree.
fn resolve_leniently(path: &Path) -> io::Result<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match fs::canonicalize(&existing) {
            Ok(base) => {
                return Ok(tail
                    .iter()
                    .rev()
                    .fold(base, |resolved, part| resolved.join(part)));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name().map(std::ffi::OsStr::to_os_string) else {
                    return Err(error);
                };
                tail.push(name);
                if !existing.pop() {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn read_text(path: &Path, limit: u64) -> io::Result<(String, bool)> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    let truncated = u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit;
    if truncated {
        bytes.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

// Windows has no executable bit; git for Windows decides by the shebang.
#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Quotes `text` for a POSIX shell as one word: AICD §23.
///
/// Single quotes around the whole, and each embedded single quote closes the
/// quotes, writes an escaped quote and reopens them. Inside single quotes
/// nothing else is special, so no byte of `text` can end the word, expand or
/// run.
fn shell_quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for character in text.chars() {
        if character == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(character);
        }
    }
    quoted.push('\'');
    quoted
}

/// The constant template, with `@HOOK@` and `@SPOOL@` the only holes.
const HEADER: &str = "#!/bin/sh
# ori-studio-hook v1: written by Ori Studio (ori-watch). The engine rewrites this file.
# Reports this git event to the engine by appending one line to the spool below,
# then runs the hook that was here before Ori Studio (\"<this file>.ori-original\"), if any.
# If the spool cannot be written this prints an error on standard error.
ori_hook=@HOOK@
ori_spool=@SPOOL@
ori_log=\"$ori_spool/hook-events.log\"
ori_original=\"$0.ori-original\"

ori_warn() {
    printf 'ori-studio: ERROR: the %s hook could not report to the Ori Studio engine: %s\\n' \"$ori_hook\" \"$1\" >&2
    printf 'ori-studio: spool directory: %s\\n' \"$ori_spool\" >&2
    printf 'ori-studio: the engine did not see this event through the hook; only ref polling can still notice it.\\n' >&2
}

ori_clean() {
    printf '%s' \"$1\" | LC_ALL=C tr -c 'A-Za-z0-9._/:@+=,%~-' '?' | cut -c1-200
}

ori_emit() {
    ori_tab=$(printf '\\t')
    ori_now=$(date +%s 2>/dev/null) || ori_now=0
    ori_line=\"$ori_now$ori_tab$ori_hook\"
    for ori_field in \"$@\"; do
        ori_line=\"$ori_line$ori_tab$(ori_clean \"$ori_field\")\"
    done
    if [ ! -d \"$ori_spool\" ]; then
        ori_warn \"the spool directory does not exist\"
        return 1
    fi
    if ! printf '%s\\n' \"$ori_line\" 2>/dev/null >> \"$ori_log\"; then
        ori_warn \"could not append to $ori_log\"
        return 1
    fi
    return 0
}

ori_head=$(git rev-parse HEAD 2>/dev/null) || ori_head='?'
";

/// Renders the script for `hook` with the already-resolved `spool` text.
fn render_script(hook: GitHook, spool: &str) -> String {
    let mut script = HEADER
        .replace("@HOOK@", &shell_quote(hook.file_name()))
        .replace("@SPOOL@", &shell_quote(spool));
    script.push_str(body_for(hook));
    script.push_str(tail_for(hook));
    script
}

/// What each hook records. Constant text; nothing is interpolated.
fn body_for(hook: GitHook) -> &'static str {
    match hook {
        GitHook::PostCommit => {
            "ori_ref=$(git symbolic-ref -q HEAD 2>/dev/null) || ori_ref='?'
ori_emit \"$ori_head\" \"$ori_ref\"
"
        }
        GitHook::PostMerge => {
            "ori_emit \"${1:-?}\" \"$ori_head\"
"
        }
        GitHook::PostRewrite => {
            "ori_stdin=$(cat)
ori_emit \"${1:-?}\" \"$ori_head\"
"
        }
        GitHook::PrePush => {
            "ori_stdin=$(cat)
case \"$1\" in
    *://*|*@*|*:*) ori_remote='url-redacted' ;;
    *) ori_remote=\"${1:-?}\" ;;
esac
if [ -n \"$ori_stdin\" ]; then
    printf '%s\\n' \"$ori_stdin\" | while read -r ori_lref ori_lsha ori_rref ori_rsha; do
        ori_emit \"$ori_remote\" \"$ori_lref\" \"$ori_lsha\" \"$ori_rref\" \"$ori_rsha\"
    done
fi
"
        }
    }
}

/// How the script hands over to the preserved hook, if there is one.
fn tail_for(hook: GitHook) -> &'static str {
    if hook.reads_stdin() {
        "if [ -x \"$ori_original\" ]; then
    if [ -n \"$ori_stdin\" ]; then
        printf '%s\\n' \"$ori_stdin\" | \"$ori_original\" \"$@\"
    else
        \"$ori_original\" \"$@\" </dev/null
    fi
    exit $?
fi
exit 0
"
    } else {
        "if [ -x \"$ori_original\" ]; then
    exec \"$ori_original\" \"$@\"
fi
exit 0
"
    }
}

/// One record a hook wrote: AICD §13.
///
/// Every field is text from the repository, reduced to a safe alphabet by the
/// hook and treated by the reader as data, never as an instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEvent {
    /// Which hook wrote it.
    pub hook: GitHook,
    /// The time the hook says it ran, to the second. Untrusted: it is the
    /// clock of whatever process ran the hook.
    pub recorded_at: Timestamp,
    /// When [`HookSpool::drain`] read it.
    pub observed_at: Timestamp,
    /// What the hook recorded, at most eight fields. `post-commit`: the new
    /// `HEAD` and its ref name. `post-merge` and `post-rewrite`: git's
    /// argument and `HEAD`. `pre-push`: the remote's name (`url-redacted` for
    /// a URL), the local ref, the local object id, the remote ref and the
    /// remote object id.
    pub fields: Vec<String>,
}

/// What one [`HookSpool::drain`] found: AICD §14.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SpoolDrain {
    /// The complete records read, oldest first.
    pub events: Vec<HookEvent>,
    /// Lines that were not a record, counted so they are not silence.
    pub malformed: usize,
    /// Set when the log was shorter than the reader's offset and so was
    /// started over: it was truncated or replaced, and records may be lost.
    pub restarted: bool,
}

/// The engine's reader of the hooks' spool: AICD §13.
///
/// Reads `hook-events.log` from an offset and returns only complete lines, so
/// a hook halfway through an append is read whole on the next drain. The
/// engine persists [`HookSpool::offset`] and reopens with
/// [`HookSpool::open_at`]; a reader opened at zero reads the whole log, which
/// is what an engine that was down while a commit was made wants.
#[derive(Debug)]
pub struct HookSpool {
    log: PathBuf,
    offset: u64,
}

impl HookSpool {
    /// Opens the spool at `spool_dir`, reading from the start: AICD §13.
    #[must_use]
    pub fn open(spool_dir: &Path) -> Self {
        Self::open_at(spool_dir, 0)
    }

    /// Opens the spool at `spool_dir`, reading from `offset`: AICD §13.
    #[must_use]
    pub fn open_at(spool_dir: &Path, offset: u64) -> Self {
        Self {
            log: spool_dir.join(SPOOL_LOG_NAME),
            offset,
        }
    }

    /// How many bytes of the log have been consumed.
    #[must_use]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Reads the complete records written since the last drain: AICD §13.
    ///
    /// Reads at most [`DRAIN_BYTES_CAP`] bytes; call again for more. A log
    /// that does not exist yet is an empty drain.
    ///
    /// # Errors
    ///
    /// [`HooksError::SpoolLogNotAFile`] when the log is a link or not a
    /// regular file, [`HooksError::Io`] on a read failure.
    pub fn drain(&mut self) -> Result<SpoolDrain, HooksError> {
        let io_error = |source| HooksError::Io {
            path: self.log.clone(),
            source,
        };
        let metadata = match fs::symlink_metadata(&self.log) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SpoolDrain::default());
            }
            Err(error) => return Err(io_error(error)),
        };
        if !metadata.is_file() {
            return Err(HooksError::SpoolLogNotAFile {
                path: self.log.clone(),
            });
        }
        let mut drain = SpoolDrain::default();
        if metadata.len() < self.offset {
            self.offset = 0;
            drain.restarted = true;
        }
        let mut file = fs::File::open(&self.log).map_err(io_error)?;
        file.seek(SeekFrom::Start(self.offset)).map_err(io_error)?;
        let mut chunk = Vec::new();
        file.take(DRAIN_BYTES_CAP as u64)
            .read_to_end(&mut chunk)
            .map_err(io_error)?;
        let observed_at = now();
        let consumed = match chunk.iter().rposition(|byte| *byte == b'\n') {
            Some(last) => last + 1,
            None if chunk.len() >= DRAIN_BYTES_CAP => {
                // One line longer than the cap cannot have come from a hook.
                drain.malformed += 1;
                chunk.len()
            }
            None => 0,
        };
        for line in chunk[..consumed].split(|byte| *byte == b'\n') {
            if line.is_empty() {
                continue;
            }
            match parse_record(&String::from_utf8_lossy(line), observed_at) {
                Some(event) => drain.events.push(event),
                None => drain.malformed += 1,
            }
        }
        self.offset += consumed as u64;
        Ok(drain)
    }
}

fn parse_record(line: &str, observed_at: Timestamp) -> Option<HookEvent> {
    let mut parts = line.split('\t');
    let seconds: i64 = parts.next()?.parse().ok()?;
    let hook = GitHook::from_file_name(parts.next()?)?;
    let fields: Vec<String> = parts.take(FIELD_CAP).map(str::to_owned).collect();
    Some(HookEvent {
        hook,
        recorded_at: Timestamp::from_millis(seconds.saturating_mul(1000)),
        observed_at,
        fields,
    })
}

fn now() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_millis(millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A scratch directory under the system temporary directory, removed when
    /// it drops.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0054-hooks-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a fresh scratch directory can be created");
            let path = fs::canonicalize(&path).expect("the scratch directory resolves");
            Self { path }
        }

        fn dir(&self, name: &str) -> PathBuf {
            let path = self.path.join(name);
            fs::create_dir_all(&path).expect("a scratch subdirectory can be created");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A repository skeleton with a `.git` directory and nothing else.
    fn skeleton(scratch: &Scratch) -> (PathBuf, PathBuf) {
        let repo = scratch.dir("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let spool = scratch.path.join("spool");
        (repo, spool)
    }

    fn hook_path(repo: &Path, hook: GitHook) -> PathBuf {
        repo.join(".git").join("hooks").join(hook.file_name())
    }

    #[test]
    fn ori_t_0054_install_writes_every_hook_and_installing_again_changes_nothing() {
        let scratch = Scratch::new("idempotent");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();

        let first = hooks.install().unwrap();
        assert_eq!(first.outcomes.len(), 4);
        assert!(
            first
                .outcomes
                .iter()
                .all(|(_, outcome)| *outcome == InstallOutcome::Installed)
        );
        assert!(spool.is_dir(), "the spool is created outside the tree");
        let before: Vec<String> = GitHook::ALL
            .iter()
            .map(|hook| fs::read_to_string(hook_path(&repo, *hook)).unwrap())
            .collect();
        for (hook, text) in GitHook::ALL.iter().zip(&before) {
            assert_eq!(text, &hooks.script_for(*hook));
            assert!(text.starts_with("#!/bin/sh\n"));
            assert!(is_ours(text));
        }

        let second = hooks.install().unwrap();
        assert!(second.is_unchanged(), "{second:?}");
        let after: Vec<String> = GitHook::ALL
            .iter()
            .map(|hook| fs::read_to_string(hook_path(&repo, *hook)).unwrap())
            .collect();
        assert_eq!(before, after);
        // No stray temporary or preserved file was left behind.
        let names: Vec<String> = fs::read_dir(repo.join(".git").join("hooks"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 4, "{names:?}");

        for status in hooks.status().unwrap() {
            assert_eq!(status.state, HookState::Installed, "{status:?}");
            assert!(!status.chains_to_original);
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_installed_hooks_carry_the_executable_bit_git_needs() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("mode");
        let (repo, spool) = skeleton(&scratch);
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        for hook in GitHook::ALL {
            let mode = fs::metadata(hook_path(&repo, hook))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "{hook:?} is not executable: {mode:o}");
        }
    }

    #[test]
    fn ori_t_0054_an_existing_hook_is_preserved_chained_and_reported() {
        let scratch = Scratch::new("preserve");
        let (repo, spool) = skeleton(&scratch);
        let existing = hook_path(&repo, GitHook::PostCommit);
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        let theirs = "#!/bin/sh\necho theirs\n";
        fs::write(&existing, theirs).unwrap();
        make_executable(&existing).unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();

        // Before: someone else's hook is reported as such.
        let status = hooks.status().unwrap();
        assert_eq!(status[0].hook, GitHook::PostCommit);
        assert_eq!(status[0].state, HookState::Foreign);

        let report = hooks.install().unwrap();
        let preserved = preserved_path(&existing);
        assert_eq!(
            report.outcomes[0],
            (
                GitHook::PostCommit,
                InstallOutcome::InstalledPreserving {
                    preserved: preserved.clone()
                }
            )
        );
        assert_eq!(fs::read_to_string(&preserved).unwrap(), theirs);
        assert!(is_ours(&fs::read_to_string(&existing).unwrap()));
        assert!(hooks.status().unwrap()[0].chains_to_original);

        // Installing again neither re-preserves nor touches the original.
        let again = hooks.install().unwrap();
        assert!(again.is_unchanged(), "{again:?}");
        assert_eq!(fs::read_to_string(&preserved).unwrap(), theirs);
    }

    #[test]
    fn ori_t_0054_two_hooks_are_never_collapsed_into_one() {
        let scratch = Scratch::new("taken");
        let (repo, spool) = skeleton(&scratch);
        let existing = hook_path(&repo, GitHook::PrePush);
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, "#!/bin/sh\necho theirs\n").unwrap();
        fs::write(preserved_path(&existing), "#!/bin/sh\necho older\n").unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        let error = hooks.install().unwrap_err();
        assert!(
            matches!(error, HooksError::PreservedNameTaken { .. }),
            "{error}"
        );
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        // Nothing was written for any hook, including the ones that were free.
        assert!(!hook_path(&repo, GitHook::PostCommit).exists());
        assert_eq!(
            fs::read_to_string(&existing).unwrap(),
            "#!/bin/sh\necho theirs\n"
        );
    }

    #[test]
    fn ori_t_0054_a_changed_spool_rewrites_the_engines_own_hook_and_only_that() {
        let scratch = Scratch::new("respool");
        let (repo, spool) = skeleton(&scratch);
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        let other = scratch.path.join("other-spool");
        let hooks = GitHooks::new(&repo, &other).unwrap();
        for status in hooks.status().unwrap() {
            assert_eq!(status.state, HookState::Stale);
        }
        let report = hooks.install().unwrap();
        assert!(
            report
                .outcomes
                .iter()
                .all(|(_, o)| *o == InstallOutcome::Updated)
        );
        assert!(
            fs::read_to_string(hook_path(&repo, GitHook::PostCommit))
                .unwrap()
                .contains(&shell_quote(other.to_str().unwrap()))
        );
        assert!(!preserved_path(&hook_path(&repo, GitHook::PostCommit)).exists());
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_hook_that_lost_its_executable_bit_is_reported_and_repaired() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("nox");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let path = hook_path(&repo, GitHook::PostMerge);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let states: Vec<HookState> = hooks.status().unwrap().iter().map(|s| s.state).collect();
        assert_eq!(states[1], HookState::NotExecutable);
        let report = hooks.install().unwrap();
        assert_eq!(report.outcomes[1].1, InstallOutcome::Updated);
        assert_eq!(hooks.status().unwrap()[1].state, HookState::Installed);
    }

    #[test]
    fn ori_t_0054_status_reports_a_missing_hook() {
        let scratch = Scratch::new("missing");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        for status in hooks.status().unwrap() {
            assert_eq!(status.state, HookState::Missing);
        }
    }

    #[test]
    fn ori_t_0054_an_installation_that_git_would_never_run_is_refused() {
        let scratch = Scratch::new("hookspath");
        let (repo, spool) = skeleton(&scratch);
        fs::write(
            repo.join(".git").join("config"),
            "[core]\n\trepositoryformatversion = 0\n\thooksPath = .githooks\n[user]\n\tname = t\n",
        )
        .unwrap();
        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::HooksPathConfigured { value } => assert_eq!(value, ".githooks"),
            other => panic!("expected HooksPathConfigured, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 14);
        assert!(
            !repo.join(".git").join("hooks").exists(),
            "nothing was written"
        );
        assert!(!spool.exists());
    }

    #[test]
    fn ori_t_0054_a_hooks_path_in_another_section_or_empty_is_not_mistaken_for_core() {
        assert_eq!(
            configured_hooks_path("[core]\n\thooksPath = x\n"),
            Some("x".into())
        );
        assert_eq!(
            configured_hooks_path("[Core] hookspath = \"y z\"\n"),
            Some("y z".into())
        );
        assert_eq!(configured_hooks_path("[alias]\n\thooksPath = x\n"), None);
        assert_eq!(configured_hooks_path("[core]\n\thooksPath =\n"), None);
        assert_eq!(configured_hooks_path("[core]\n\t# hooksPath = x\n"), None);
        assert_eq!(
            configured_hooks_path("[core]\n\tbare = false\n[alias]\n\tx = hooksPath = y\n"),
            None
        );
    }

    #[test]
    fn ori_t_0054_a_git_file_is_refused_so_nothing_is_written_through_a_linked_worktree() {
        let scratch = Scratch::new("gitfile");
        let repo = scratch.dir("worktree");
        fs::write(repo.join(".git"), "gitdir: /elsewhere/.git/worktrees/w\n").unwrap();
        let error = GitHooks::new(&repo, &scratch.path.join("spool")).unwrap_err();
        assert!(
            matches!(error, HooksError::GitDirNotADirectory { .. }),
            "{error}"
        );
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_symlinked_hooks_directory_is_refused_and_nothing_lands_beyond_it() {
        let scratch = Scratch::new("hookslink");
        let (repo, spool) = skeleton(&scratch);
        let elsewhere = scratch.dir("elsewhere");
        std::os::unix::fs::symlink(&elsewhere, repo.join(".git").join("hooks")).unwrap();
        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        assert!(
            matches!(error, HooksError::HooksDirNotADirectory { .. }),
            "{error}"
        );
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);

        // A symlinked `.git` is refused at construction.
        let linked = scratch.dir("linked");
        std::os::unix::fs::symlink(&elsewhere, linked.join(".git")).unwrap();
        assert!(matches!(
            GitHooks::new(&linked, &spool),
            Err(HooksError::GitDirNotADirectory { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_hook_that_is_a_symlink_is_someone_elses_and_is_moved_not_written_through() {
        let scratch = Scratch::new("hooklink");
        let (repo, spool) = skeleton(&scratch);
        let target = scratch.path.join("shared-hook.sh");
        fs::write(&target, "#!/bin/sh\necho shared\n").unwrap();
        let path = hook_path(&repo, GitHook::PostCommit);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "#!/bin/sh\necho shared\n",
            "the link's target was written through"
        );
        let preserved = preserved_path(&path);
        assert!(
            fs::symlink_metadata(&preserved)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(is_ours(&fs::read_to_string(&path).unwrap()));
    }

    #[test]
    fn ori_t_0054_a_directory_where_a_hook_belongs_is_refused() {
        let scratch = Scratch::new("hookdir");
        let (repo, spool) = skeleton(&scratch);
        fs::create_dir_all(hook_path(&repo, GitHook::PrePush)).unwrap();
        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        assert!(matches!(error, HooksError::HookNotAFile { .. }), "{error}");
    }

    #[test]
    fn ori_t_0054_a_spool_inside_the_working_tree_is_refused() {
        let scratch = Scratch::new("spoolinside");
        let (repo, _) = skeleton(&scratch);
        for inside in [repo.join("spool"), repo.join("a").join("b"), repo.clone()] {
            let error = GitHooks::new(&repo, &inside).unwrap_err();
            assert!(
                matches!(error, HooksError::SpoolInsideTree { .. }),
                "{inside:?}: {error}"
            );
            assert_eq!(error.methodology_ref().unwrap().section, 23);
        }
        // Relative, `..` and `.` spools name a different directory from a
        // different working directory.
        for plain in [
            PathBuf::from("spool"),
            scratch.path.join("x").join("..").join("spool"),
        ] {
            let error = GitHooks::new(&repo, &plain).unwrap_err();
            assert!(
                matches!(error, HooksError::SpoolNotPlain { .. }),
                "{plain:?}: {error}"
            );
        }
        // A sibling whose name extends the repository's is outside it.
        let sibling = PathBuf::from(format!("{}-spool", repo.display()));
        assert!(GitHooks::new(&repo, &sibling).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_spool_reached_through_a_link_into_the_tree_is_still_inside_it() {
        let scratch = Scratch::new("spoollink");
        let (repo, _) = skeleton(&scratch);
        fs::create_dir_all(repo.join("real")).unwrap();
        let alias = scratch.path.join("alias");
        std::os::unix::fs::symlink(repo.join("real"), &alias).unwrap();
        let error = GitHooks::new(&repo, &alias.join("spool")).unwrap_err();
        assert!(
            matches!(error, HooksError::SpoolInsideTree { .. }),
            "{error}"
        );
    }

    #[test]
    fn ori_t_0054_the_spool_path_is_the_only_value_in_the_script_and_it_is_quoted() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
        let nasty = "/tmp/sp ool/$(touch pwned)/`id`/a'b\"c;d&e|f\\g\nh\ti";
        let script = render_script(GitHook::PrePush, nasty);
        let quoted = shell_quote(nasty);
        // The spool appears exactly once, inside its quotes, on the one line
        // that assigns it; none of its special characters appears raw anywhere
        // else in the script.
        assert_eq!(script.matches(&quoted).count(), 1);
        assert!(script.contains(&format!("\nori_spool={quoted}\n")));
        let without = script.replacen(&quoted, "", 1);
        assert!(!without.contains("pwned"));
        assert!(!without.contains("`id`"));
        // The hook's own name is the only other substituted value.
        assert!(script.contains("\nori_hook='pre-push'\n"));
        for hook in GitHook::ALL {
            let text = render_script(hook, "/s");
            assert!(!text.contains("@HOOK@") && !text.contains("@SPOOL@"));
            assert!(is_ours(&text));
        }
    }

    #[test]
    fn ori_t_0054_the_spool_reader_reads_whole_lines_only_and_counts_what_it_cannot_parse() {
        let scratch = Scratch::new("spoolread");
        let spool = scratch.dir("spool");
        let log = spool.join(SPOOL_LOG_NAME);
        let mut reader = HookSpool::open(&spool);
        assert_eq!(reader.drain().unwrap(), SpoolDrain::default());

        fs::write(
            &log,
            "1700000000\tpost-commit\tabc123\trefs/heads/main\n\
             not a record\n\
             1700000001\tunknown-hook\tx\n\
             1700000002\tpre-push\torigin\trefs/heads/m\t",
        )
        .unwrap();
        let drain = reader.drain().unwrap();
        assert_eq!(drain.events.len(), 1);
        assert_eq!(drain.events[0].hook, GitHook::PostCommit);
        assert_eq!(drain.events[0].fields, vec!["abc123", "refs/heads/main"]);
        assert_eq!(
            drain.events[0].recorded_at,
            Timestamp::from_millis(1_700_000_000_000)
        );
        assert_eq!(drain.malformed, 2);
        let consumed = reader.offset();

        // The unfinished last line is left for the next drain, whole.
        assert!(reader.drain().unwrap().events.is_empty());
        assert_eq!(reader.offset(), consumed);
        let mut file = fs::OpenOptions::new().append(true).open(&log).unwrap();
        file.write_all(b"aaa\tbbb\tccc\n").unwrap();
        drop(file);
        let rest = reader.drain().unwrap();
        assert_eq!(rest.events.len(), 1);
        assert_eq!(rest.events[0].hook, GitHook::PrePush);
        assert_eq!(
            rest.events[0].fields,
            vec!["origin", "refs/heads/m", "aaa", "bbb", "ccc"]
        );

        // A log shorter than the offset was truncated or replaced: say so.
        fs::write(&log, "1700000009\tpost-merge\t0\tdef\n").unwrap();
        let restarted = reader.drain().unwrap();
        assert!(restarted.restarted);
        assert_eq!(restarted.events.len(), 1);

        // An engine that was down reopens at its saved offset.
        let mut resumed = HookSpool::open_at(&spool, reader.offset());
        assert!(resumed.drain().unwrap().events.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_spool_log_that_is_a_symlink_is_not_read() {
        let scratch = Scratch::new("spoollog");
        let spool = scratch.dir("spool");
        let secret = scratch.path.join("secret");
        fs::write(&secret, "1\tpost-commit\tleaked\n").unwrap();
        std::os::unix::fs::symlink(&secret, spool.join(SPOOL_LOG_NAME)).unwrap();
        let error = HookSpool::open(&spool).drain().unwrap_err();
        assert!(
            matches!(error, HooksError::SpoolLogNotAFile { .. }),
            "{error}"
        );
    }

    // ---- the scripts, run by a real shell and by real git ----

    #[cfg(unix)]
    mod running {
        use super::*;
        use std::process::{Command, Output, Stdio};

        /// Runs `git` in `dir` with the user's and the system's configuration
        /// out of the way.
        fn git(scratch: &Scratch, dir: &Path, args: &[&str]) -> Output {
            Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args([
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "init.defaultBranch=main",
                ])
                .args(args)
                .current_dir(dir)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("HOME", &scratch.path)
                .env("XDG_CONFIG_HOME", &scratch.path)
                .output()
                .expect("git can be run")
        }

        fn git_ok(scratch: &Scratch, dir: &Path, args: &[&str]) -> String {
            let output = git(scratch, dir, args);
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }

        fn real_repo(scratch: &Scratch) -> PathBuf {
            let repo = scratch.dir("repo");
            git_ok(scratch, &repo, &["init", "-q"]);
            fs::write(repo.join("a.txt"), "a").unwrap();
            git_ok(scratch, &repo, &["add", "a.txt"]);
            repo
        }

        /// Every file under `dir` outside `.git`, with its bytes.
        fn tree_snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
            let mut files = Vec::new();
            let mut stack = vec![dir.to_path_buf()];
            while let Some(current) = stack.pop() {
                for entry in fs::read_dir(&current).unwrap() {
                    let entry = entry.unwrap();
                    if entry.file_name() == ".git" {
                        continue;
                    }
                    let path = entry.path();
                    if entry.file_type().unwrap().is_dir() {
                        stack.push(path);
                    } else {
                        let bytes = fs::read(&path).unwrap();
                        files.push((path, bytes));
                    }
                }
            }
            files.sort();
            files
        }

        fn log_of(spool: &Path) -> String {
            fs::read_to_string(spool.join(SPOOL_LOG_NAME)).unwrap_or_default()
        }

        #[test]
        fn ori_t_0054_installing_hooks_leaves_the_working_tree_untouched() {
            let scratch = Scratch::new("tree");
            let repo = real_repo(&scratch);
            fs::create_dir_all(repo.join("src")).unwrap();
            fs::write(repo.join("src").join("lib.rs"), "fn main() {}").unwrap();
            let spool = scratch.path.join("spool");
            let before = tree_snapshot(&repo);
            let status_before = git_ok(&scratch, &repo, &["status", "--porcelain"]);

            GitHooks::new(&repo, &spool).unwrap().install().unwrap();

            assert_eq!(tree_snapshot(&repo), before, "a file outside .git changed");
            assert_eq!(
                git_ok(&scratch, &repo, &["status", "--porcelain"]),
                status_before,
                "git sees a difference in the tree"
            );
        }

        #[test]
        fn ori_t_0054_a_real_commit_amend_merge_and_push_each_reach_the_spool() {
            let scratch = Scratch::new("e2e");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            let mut reader = HookSpool::open(&spool);

            git_ok(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            let head = git_ok(&scratch, &repo, &["rev-parse", "HEAD"]);
            let drain = reader.drain().unwrap();
            assert_eq!(drain.malformed, 0);
            assert_eq!(drain.events.len(), 1, "{:?}", log_of(&spool));
            assert_eq!(drain.events[0].hook, GitHook::PostCommit);
            assert_eq!(
                drain.events[0].fields,
                vec![head.clone(), "refs/heads/main".to_owned()]
            );

            git_ok(
                &scratch,
                &repo,
                &["commit", "-q", "--amend", "-m", "first, reworded"],
            );
            let amended = reader.drain().unwrap();
            let hooks: Vec<GitHook> = amended.events.iter().map(|e| e.hook).collect();
            assert!(hooks.contains(&GitHook::PostCommit), "{hooks:?}");
            assert!(hooks.contains(&GitHook::PostRewrite), "{hooks:?}");

            // A merge commit fires post-merge and not post-commit.
            git_ok(&scratch, &repo, &["checkout", "-q", "-b", "topic"]);
            fs::write(repo.join("b.txt"), "b").unwrap();
            git_ok(&scratch, &repo, &["add", "b.txt"]);
            git_ok(&scratch, &repo, &["commit", "-q", "-m", "topic work"]);
            git_ok(&scratch, &repo, &["checkout", "-q", "main"]);
            fs::write(repo.join("c.txt"), "c").unwrap();
            git_ok(&scratch, &repo, &["add", "c.txt"]);
            git_ok(&scratch, &repo, &["commit", "-q", "-m", "main work"]);
            let _ = reader.drain().unwrap();
            git_ok(
                &scratch,
                &repo,
                &["merge", "-q", "--no-ff", "-m", "merge topic", "topic"],
            );
            let merged = reader.drain().unwrap();
            assert!(
                merged.events.iter().any(|e| e.hook == GitHook::PostMerge),
                "{:?}",
                log_of(&spool)
            );

            // A push to a bare remote, named by its remote name.
            let bare = scratch.dir("bare.git");
            git_ok(&scratch, &bare, &["init", "-q", "--bare"]);
            git_ok(
                &scratch,
                &repo,
                &["remote", "add", "origin", bare.to_str().unwrap()],
            );
            git_ok(&scratch, &repo, &["push", "-q", "origin", "main"]);
            let pushed = reader.drain().unwrap();
            let push = pushed
                .events
                .iter()
                .find(|e| e.hook == GitHook::PrePush)
                .unwrap_or_else(|| panic!("no pre-push record: {:?}", log_of(&spool)));
            assert_eq!(push.fields[0], "origin");
            assert_eq!(push.fields[1], "refs/heads/main");
            assert_eq!(push.fields[3], "refs/heads/main");
        }

        #[test]
        fn ori_p1_015_a_commit_in_a_linked_worktree_reaches_the_main_repositorys_hooks_and_refs() {
            use crate::watcher::{RefClass, RefPoller};
            let scratch = Scratch::new("linked");
            let repo = real_repo(&scratch);
            git_ok(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            let spool = scratch.path.join("spool");
            // Hooks live in the common directory: installed once in the main
            // repository, they run for every worktree.
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            let mut poller = RefPoller::open(&repo).unwrap();
            let worktree = scratch.path.join("session-worktree");
            git_ok(
                &scratch,
                &repo,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    "feat/T-1",
                    worktree.to_str().unwrap(),
                ],
            );
            let _ = poller.poll().unwrap();

            fs::write(worktree.join("session.txt"), "from the session").unwrap();
            git_ok(&scratch, &worktree, &["add", "session.txt"]);
            git_ok(&scratch, &worktree, &["commit", "-q", "-m", "session work"]);
            let head = git_ok(&scratch, &worktree, &["rev-parse", "HEAD"]);

            let drain = HookSpool::open(&spool).drain().unwrap();
            let commit = drain
                .events
                .iter()
                .find(|event| event.hook == GitHook::PostCommit)
                .unwrap_or_else(|| panic!("no post-commit record: {:?}", log_of(&spool)));
            assert_eq!(
                commit.fields,
                vec![head.clone(), "refs/heads/feat/T-1".to_owned()]
            );
            // The ref poller on the main repository sees the branch move, with
            // the worktree's branch name, which is how a commit is attributed.
            let poll = poller.poll().unwrap();
            let moved = poll
                .changes
                .iter()
                .find(|change| change.name == "refs/heads/feat/T-1")
                .unwrap_or_else(|| panic!("no ref change: {:?}", poll.changes));
            assert_eq!(moved.class(), RefClass::Branch);
            assert_eq!(moved.after.as_deref(), Some(head.as_str()));
        }

        #[test]
        fn ori_t_0054_a_hook_that_cannot_reach_the_engine_says_so_on_stderr_and_does_not_hide() {
            let scratch = Scratch::new("loud");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();

            // The spool disappears.
            fs::remove_dir_all(&spool).unwrap();
            let output = git(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            assert!(
                output.status.success(),
                "a spool fault must not fail the commit: {stderr}"
            );
            assert!(
                stderr.contains("ori-studio: ERROR"),
                "silent failure: {stderr:?}"
            );
            assert!(stderr.contains("post-commit"), "{stderr:?}");
            assert!(
                stderr.contains("spool directory does not exist"),
                "{stderr:?}"
            );

            // The spool exists but its log cannot be appended to: a directory
            // sits where the file belongs.
            fs::create_dir_all(spool.join(SPOOL_LOG_NAME)).unwrap();
            fs::write(repo.join("a.txt"), "again").unwrap();
            let output = git(&scratch, &repo, &["commit", "-q", "-a", "-m", "second"]);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            assert!(output.status.success(), "{stderr}");
            assert!(
                stderr.contains("could not append"),
                "silent failure: {stderr:?}"
            );
        }

        #[test]
        fn ori_t_0054_the_preserved_hook_still_runs_receives_its_input_and_keeps_its_veto() {
            let scratch = Scratch::new("chain");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            let marker = scratch.path.join("theirs-ran");
            let hooks_dir = repo.join(".git").join("hooks");
            fs::create_dir_all(&hooks_dir).unwrap();
            let post = hooks_dir.join("post-commit");
            fs::write(
                &post,
                format!("#!/bin/sh\necho \"$#\" > '{}'\n", marker.display()),
            )
            .unwrap();
            make_executable(&post).unwrap();
            let pre = hooks_dir.join("pre-push");
            let stdin_copy = scratch.path.join("their-stdin");
            fs::write(
                &pre,
                format!(
                    "#!/bin/sh\ncat > '{}'\necho \"their veto\" >&2\nexit 1\n",
                    stdin_copy.display()
                ),
            )
            .unwrap();
            make_executable(&pre).unwrap();
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();

            git_ok(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            assert_eq!(
                fs::read_to_string(&marker).unwrap().trim(),
                "0",
                "their post-commit did not run"
            );
            assert!(
                log_of(&spool).contains("post-commit"),
                "ours did not record"
            );

            let bare = scratch.dir("bare.git");
            git_ok(&scratch, &bare, &["init", "-q", "--bare"]);
            git_ok(
                &scratch,
                &repo,
                &["remote", "add", "origin", bare.to_str().unwrap()],
            );
            let push = git(&scratch, &repo, &["push", "-q", "origin", "main"]);
            assert!(!push.status.success(), "their pre-push veto was lost");
            assert!(String::from_utf8_lossy(&push.stderr).contains("their veto"));
            let theirs = fs::read_to_string(&stdin_copy).unwrap();
            assert!(
                theirs.starts_with("refs/heads/main "),
                "their stdin was {theirs:?}"
            );
            // Ours recorded the attempted push before handing over.
            assert!(log_of(&spool).contains("pre-push"));
        }

        #[test]
        fn ori_t_0054_a_spool_path_full_of_shell_metacharacters_is_a_directory_name_and_nothing_else()
         {
            let scratch = Scratch::new("nasty");
            let repo = real_repo(&scratch);
            let canary = scratch.path.join("pwned");
            let spool = scratch.path.join(format!(
                "sp ool'; touch '{}'; echo '$(touch {})`touch {}`\nline",
                canary.display(),
                canary.display(),
                canary.display()
            ));
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            git_ok(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            assert!(!canary.exists(), "the spool path was executed");
            let log = fs::read_to_string(spool.join(SPOOL_LOG_NAME)).unwrap();
            assert!(log.contains("\tpost-commit\t"), "{log:?}");
        }

        #[test]
        fn ori_t_0054_branch_and_ref_names_cannot_forge_a_record_or_run_anything() {
            let scratch = Scratch::new("forge");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            let canary = scratch.path.join("pwned");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            git_ok(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            // A branch name git accepts that is hostile to a naive script.
            let evil = format!(
                "x$(touch${{IFS}}{})`touch${{IFS}}{}`;&|'\"y",
                canary.display(),
                canary.display()
            );
            git_ok(&scratch, &repo, &["checkout", "-q", "-b", &evil]);
            fs::write(repo.join("b.txt"), "b").unwrap();
            git_ok(&scratch, &repo, &["add", "b.txt"]);
            git_ok(
                &scratch,
                &repo,
                &["commit", "-q", "-m", "on the evil branch"],
            );
            assert!(!canary.exists(), "a branch name was executed");
            let drain = HookSpool::open(&spool).drain().unwrap();
            assert_eq!(drain.malformed, 0);
            assert_eq!(
                drain.events.len(),
                2,
                "exactly one record per commit: {:?}",
                log_of(&spool)
            );
            for event in &drain.events {
                assert!(
                    event.fields.iter().all(
                        |f| f.len() <= 200 && !f.contains(['$', '`', '\'', '"', ';', '|', '&'])
                    )
                );
            }
        }

        #[test]
        fn ori_t_0054_a_remote_url_and_the_credential_in_it_are_never_recorded() {
            let scratch = Scratch::new("secret");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            let hooks = GitHooks::new(&repo, &spool).unwrap();
            hooks.install().unwrap();
            let script = hook_path(&repo, GitHook::PrePush);
            for url in [
                "https://user:s3cr3t-token@example.invalid/org/repo.git",
                "git@example.invalid:org/repo.git",
                "ssh://deploy@example.invalid/x",
            ] {
                // Git passes the URL as both arguments when pushing to one.
                let mut child = Command::new("sh")
                    .arg(&script)
                    .args([url, url])
                    .current_dir(&repo)
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(b"refs/heads/main aaa refs/heads/main bbb\n")
                    .unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let log = log_of(&spool);
            assert!(log.contains("url-redacted"), "{log:?}");
            for leaked in ["s3cr3t", "token", "example.invalid", "deploy"] {
                assert!(!log.contains(leaked), "{leaked} reached the spool: {log:?}");
            }
        }

        #[test]
        fn ori_t_0054_a_script_that_is_syntactically_valid_for_every_hook() {
            let scratch = Scratch::new("syntax");
            let (repo, spool) = skeleton(&scratch);
            let hooks = GitHooks::new(&repo, &spool).unwrap();
            for hook in GitHook::ALL {
                let path = scratch.path.join(format!("{}.sh", hook.file_name()));
                fs::write(&path, hooks.script_for(hook)).unwrap();
                let output = Command::new("sh").arg("-n").arg(&path).output().unwrap();
                assert!(
                    output.status.success(),
                    "{hook:?} does not parse: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}
