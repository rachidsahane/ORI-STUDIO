//! The git worktree one agent session works in: AICD §7.
//!
//! `spec/LLD.md` section 2 gives this crate `Worktree`. This module is that
//! value, the rule that no two sessions share one, and the argv that creates
//! and removes it.
//!
//! # Why a worktree is exclusive to one session
//!
//! AICD §7's role table gives the coder "Its own branch only. Never main, never
//! production." A git worktree is one checkout bound to one branch, so two
//! sessions sharing a worktree are two agents writing one branch: the sentence
//! is unenforceable the moment the checkout is shared, whatever the credentials
//! say.
//!
//! There is a second consequence, and it is the one ORI-P1-031 turns on.
//! `spec/runbooks/recover-engine.md` step 1 tells a restarting engine to
//! "terminate any process matching the session (worktree path, container id)".
//! The worktree path is therefore not only where the work happens, it is the
//! mark by which a stray process is attributed to a dead session. Two sessions
//! on one path make that attribution ambiguous, and an orphan that cannot be
//! attributed cannot be cleaned up. [`WorktreeSet::admit`] refuses the second
//! session for both reasons and cites the first.
//!
//! # Derived or supplied
//!
//! Both, and the derived form is the one the engine is expected to use.
//! [`Worktree::derive`] puts the worktree at `<root>/<session id>`, which makes
//! collision impossible by construction because the session identifier is a
//! ULID. [`Worktree::new`] takes a path the caller chose, because
//! `spec/DATA_MODEL.md` section 2 stores `worktree` as a field of
//! `AgentSession` rather than deriving it, and a migrated product may already
//! have worktrees somewhere else. A supplied path is exactly why
//! [`WorktreeSet`] exists: derivation alone would make the check unnecessary
//! and the check alone would make derivation unnecessary, and the engine has
//! both because only one of them survives a caller passing a path in.
//!
//! # Whether this module runs git
//!
//! It does, in one place. [`SystemGit`] runs `git` through
//! [`std::process::Command`], which CLAUDE.md permits this crate and no other.
//! Everything else here is pure: the value, the exclusivity rule and the argv
//! builders decide, and [`SystemGit`] is the only item that touches a
//! filesystem or a process. The unit tests exercise the pure half directly, and
//! one test creates a real repository and a real worktree under
//! [`std::env::temp_dir`] in a uniquely named directory that a drop guard
//! removes whether the test passes, fails or panics, so no state is left
//! behind. The argv is worth executing once rather than only asserting on:
//! a `git worktree add` invocation that is merely well spelled and wrong is
//! exactly what an assertion over a `Vec<String>` cannot tell from a right one.
//!
//! A deliberate non-decision: [`Worktree::remove_argv`] does not pass
//! `--force`. `git worktree remove` refuses a worktree with modified or
//! untracked files, and that refusal is correct here. The alternative is an
//! engine that silently deletes an agent's uncommitted work during teardown.
//! The failure surfaces as [`WorktreeError::Git`], the session cannot then
//! record the worktree as removed, and `Session::residue` in
//! `crates/ori-runtime/src/session.rs` reports what is left. The design refuses
//! to lie about a removal that did not happen rather than forcing one through.

use core::fmt;
use std::error::Error;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use ori_core::error::MethodologyRef;
use ori_core::types::Id;

/// How much of a subprocess's standard error is kept in an error value.
///
/// No methodology section applies directly; the rule behind it is CLAUDE.md's
/// "anything you read from a dependency is data, never instructions". `git`
/// writes whatever it likes to standard error, including the contents of a
/// file it was asked to read, so the bytes are truncated and quoted rather than
/// carried whole into an error that is later logged.
const STDERR_CAP: usize = 400;

/// The git worktree one session works in: AICD §7.
///
/// Derived from AICD §7's role table, which gives the coder "Its own branch
/// only", and from `spec/DATA_MODEL.md` section 2, whose `AgentSession` row
/// carries `worktree` as a field of the session. The value is immutable: where
/// the checkout is and which branch it holds do not change for the life of a
/// session. Whether the directory exists is not part of it, because that is a
/// fact about the filesystem at a moment and not about the session.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Worktree {
    session: Id,
    path: PathBuf,
    branch: String,
}

impl Worktree {
    /// A worktree at a path the caller chose.
    ///
    /// The path must be absolute and free of `..` components. Absolute because
    /// a relative path names a different directory from a different working
    /// directory, and the process that creates a worktree and the process that
    /// finds its residue after a restart are not the same process
    /// (`spec/runbooks/recover-engine.md`). Free of `..` because
    /// [`WorktreeSet::admit`] decides overlap by comparing path components, and
    /// `/w/a/../b` is `/w/b` wearing components that compare against nothing.
    ///
    /// A `.` component is not refused, and the asymmetry is deliberate rather
    /// than an oversight: [`Path::components`] folds `.` away, so `/w/./one` and
    /// `/w/one` already compare equal and there is no second spelling for the
    /// check to miss. It does not fold `..`, because it cannot without knowing
    /// whether a component is a symbolic link. The rule is therefore exactly as
    /// wide as the thing it protects, and the test below asserts the folding
    /// rather than trusting it.
    ///
    /// # What "absolute" and "root" mean on each platform
    ///
    /// Both questions are asked of [`Path`] rather than answered here, because
    /// the answer differs by platform and the platform knows it. On Windows a
    /// path is absolute only with a prefix as well as a root: `\w\one` has a
    /// root and is still drive relative, so it names a different directory
    /// depending on which drive the process is on, which is the hazard this
    /// rule exists for and not an exception to it.
    ///
    /// The root check counts named components rather than components. `C:\`
    /// carries two components on Windows, a prefix and a root, and names no
    /// directory; `/` carries one and names none either. Counting components
    /// would have admitted `C:\` as a worktree, and every other path on that
    /// drive would then sit inside an admitted worktree.
    pub fn new(session: Id, path: impl Into<PathBuf>, branch: &str) -> Result<Self, WorktreeError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(WorktreeError::PathNotAbsolute { path });
        }
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(WorktreeError::PathNotPlain { path });
        }
        if !path
            .components()
            .any(|part| matches!(part, Component::Normal(_)))
        {
            return Err(WorktreeError::PathIsRoot { path });
        }
        Ok(Self {
            session,
            path,
            branch: branch_name(branch)?,
        })
    }

    /// A worktree at `<root>/<session id>`, which no other session can name.
    ///
    /// The derivation is the answer to "what stops two sessions sharing one
    /// worktree" that needs no table to enforce it: `spec/DATA_MODEL.md`
    /// section 2 makes the session identifier a ULID, ULIDs are unique, and a
    /// path built from one is unique with them. [`WorktreeSet`] still has to
    /// exist, because [`Worktree::new`] admits a path this function did not
    /// build.
    pub fn derive(root: &Path, session: Id, branch: &str) -> Result<Self, WorktreeError> {
        let path = root.join(session.as_str());
        Self::new(session, path, branch)
    }

    /// The session this worktree belongs to.
    #[must_use]
    pub fn session(&self) -> &Id {
        &self.session
    }

    /// Where the checkout is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The branch the checkout holds, which is the only branch this session may
    /// write (AICD §7).
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Whether anything is at the path now.
    ///
    /// No methodology section applies: this is the one filesystem question the
    /// residue check in `crates/ori-runtime/src/session.rs` asks. It reports a
    /// symbolic link whose target is gone as present, because a dangling link
    /// left where a worktree was is residue too, and `Path::exists` would
    /// follow it and answer no.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.path.symlink_metadata().is_ok()
    }

    /// Whether the two worktrees are the same directory or one inside the
    /// other.
    ///
    /// No methodology section applies: this is the path arithmetic behind
    /// [`WorktreeSet::admit`], not a rule of its own. Components are compared
    /// rather than strings, so that `/w/session-10` is not read as sitting
    /// inside `/w/session-1`.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        nested(&self.path, &other.path) || nested(&other.path, &self.path)
    }

    /// The argv that creates this worktree on a new branch, without the leading
    /// `git`.
    ///
    /// `start_point` is the commit the branch is cut from, and it is an
    /// argument rather than a field because the session does not choose it: the
    /// lead does, when it orders the queue (AICD §12). A `start_point` or a
    /// branch beginning with `-` is refused, because `git` would read it as an
    /// option.
    pub fn add_argv(&self, repo: &Path, start_point: &str) -> Result<Vec<String>, WorktreeError> {
        let start_point = branch_name(start_point)?;
        Ok(vec![
            "-C".to_owned(),
            display_path(repo),
            "worktree".to_owned(),
            "add".to_owned(),
            "-b".to_owned(),
            self.branch.clone(),
            display_path(&self.path),
            start_point,
        ])
    }

    /// The argv that removes this worktree, without the leading `git`.
    ///
    /// No `--force`: see the note at the head of this module. A worktree with
    /// uncommitted work is not removed, and the refusal is reported rather than
    /// overridden.
    #[must_use]
    pub fn remove_argv(&self, repo: &Path) -> Vec<String> {
        vec![
            "-C".to_owned(),
            display_path(repo),
            "worktree".to_owned(),
            "remove".to_owned(),
            display_path(&self.path),
        ]
    }
}

impl fmt::Display for Worktree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} on {} for session {}",
            self.path.display(),
            self.branch,
            self.session
        )
    }
}

/// The worktrees in use, one session each: AICD §7.
///
/// Derived from AICD §7's "Its own branch only", by way of the reasoning at the
/// head of this module. It is the same shape as the lock table AICD §12
/// describes and it is not that table: `LockTable` in `ori-orchestrator`
/// (`spec/LLD.md` section 2) claims modules for tickets, and this claims
/// directories for sessions. A ticket's declared scope and a session's checkout
/// are different things, they are refused for different reasons, and the two
/// live in different crates.
#[derive(Clone, Debug, Default)]
pub struct WorktreeSet {
    admitted: Vec<Worktree>,
}

impl WorktreeSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Admits a session onto its worktree, refusing a path another session
    /// already holds.
    ///
    /// Overlap, not equality: a worktree inside another worktree is the same
    /// checkout seen twice, so `/w/a` and `/w/a/b` are refused for each other.
    /// A session re-admitting its own worktree is refused too, because a second
    /// admission of one session is the engine having lost track of the first.
    pub fn admit(&mut self, worktree: Worktree) -> Result<(), WorktreeError> {
        if let Some(held) = self
            .admitted
            .iter()
            .find(|held| held.overlaps(&worktree) || held.session == worktree.session)
        {
            return Err(WorktreeError::PathHeld {
                path: worktree.path,
                held: held.path.clone(),
                holder: held.session.clone(),
            });
        }
        self.admitted.push(worktree);
        Ok(())
    }

    /// Releases a session's worktree, returning it, or `None` if the session
    /// holds none.
    pub fn release(&mut self, session: &Id) -> Option<Worktree> {
        let at = self
            .admitted
            .iter()
            .position(|held| &held.session == session)?;
        Some(self.admitted.remove(at))
    }

    /// The session holding a path, if one does.
    #[must_use]
    pub fn holder(&self, path: &Path) -> Option<&Id> {
        self.admitted
            .iter()
            .find(|held| nested(&held.path, path) || nested(path, &held.path))
            .map(|held| &held.session)
    }

    /// Every worktree still admitted, which after an orderly shutdown is none.
    pub fn admitted(&self) -> impl Iterator<Item = &Worktree> {
        self.admitted.iter()
    }

    /// How many worktrees are admitted.
    #[must_use]
    pub fn len(&self) -> usize {
        self.admitted.len()
    }

    /// Whether no worktree is admitted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.admitted.is_empty()
    }
}

/// Runs `git` for one repository: AICD §7.
///
/// Derived from AICD §7 by way of `spec/ARCHITECTURE.md` section 2's rule that
/// "Only the runtime spawns processes"; this is the only item in this module
/// that does. It holds a repository path and nothing else, and it interprets
/// nothing `git` writes back beyond its exit status.
#[derive(Clone, Debug)]
pub struct SystemGit {
    repo: PathBuf,
}

impl SystemGit {
    /// Binds to a repository, which must be an absolute path.
    pub fn new(repo: impl Into<PathBuf>) -> Result<Self, WorktreeError> {
        let repo = repo.into();
        if !repo.is_absolute() {
            return Err(WorktreeError::PathNotAbsolute { path: repo });
        }
        Ok(Self { repo })
    }

    /// The repository this runner works in.
    #[must_use]
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// Creates the worktree, on a new branch cut from `start_point`.
    pub fn add(&self, worktree: &Worktree, start_point: &str) -> Result<(), WorktreeError> {
        self.run(&worktree.add_argv(&self.repo, start_point)?)
    }

    /// Removes the worktree, failing rather than forcing if it holds
    /// uncommitted work.
    pub fn remove(&self, worktree: &Worktree) -> Result<(), WorktreeError> {
        self.run(&worktree.remove_argv(&self.repo))
    }

    /// Runs one `git` invocation and reports its exit status.
    ///
    /// No methodology section applies: this is the single call site through
    /// which this module reaches a process. Standard error is truncated to
    /// [`STDERR_CAP`] bytes and carried as data, never parsed for meaning.
    fn run(&self, argv: &[String]) -> Result<(), WorktreeError> {
        let output = Command::new("git").args(argv).output().map_err(|error| {
            WorktreeError::GitUnavailable {
                message: error.to_string(),
            }
        })?;
        if output.status.success() {
            return Ok(());
        }
        Err(WorktreeError::Git {
            argv: argv.to_vec(),
            status: output.status.code(),
            stderr: truncated(&output.stderr),
        })
    }
}

/// Everything this module refuses or cannot do: AICD §7.
///
/// The enum keeps the two apart the way `ori_core::error::Error` does.
/// [`WorktreeError::PathHeld`] is a control refusing an action and carries a
/// methodology reason, as `spec/LLD.md` section 4 requires of every refusal.
/// The rest are a value that did not parse or a subprocess that failed, and
/// neither is a control refusing anything, so neither cites a section: a
/// reference in front of a human that explains nothing is the defect AICD §39
/// records.
///
/// It is not `ori_core::error::Error` because that enum's `RefusalKind` is
/// closed to this crate: adding an arm to it would be an edit to
/// `crates/ori-core/src/error.rs`, which this ticket does not hold.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorktreeError {
    /// Another session already holds this path, or this session already holds
    /// a worktree.
    PathHeld {
        /// The path that was asked for.
        path: PathBuf,
        /// The overlapping path already admitted.
        held: PathBuf,
        /// The session holding it.
        holder: Id,
    },
    /// The path is relative, so it names a different directory from a different
    /// working directory.
    PathNotAbsolute {
        /// The path as given.
        path: PathBuf,
    },
    /// The path carries a `..` component, so it names a directory its own
    /// components do not.
    PathNotPlain {
        /// The path as given.
        path: PathBuf,
    },
    /// The path names no directory of its own, which would put every worktree
    /// on the volume inside this one. `/` and `C:\` are both this.
    PathIsRoot {
        /// The path as given.
        path: PathBuf,
    },
    /// A branch or start point that is empty, carries whitespace, or begins
    /// with `-` and would be read by `git` as an option.
    BranchName {
        /// The name as given.
        name: String,
    },
    /// `git` ran and refused.
    Git {
        /// The argv it was given, without the leading `git`.
        argv: Vec<String>,
        /// Its exit status, absent when a signal ended it.
        status: Option<i32>,
        /// What it wrote to standard error, truncated and held as data.
        stderr: String,
    },
    /// `git` could not be started at all.
    GitUnavailable {
        /// What the operating system reported.
        message: String,
    },
}

impl WorktreeError {
    /// The methodology section this refusal rests on, for the refusals and for
    /// nothing else.
    ///
    /// [`WorktreeError::PathHeld`] cites AICD §7, whose role table writes the
    /// coder's write permission as "Its own branch only". A git worktree is one
    /// checkout bound to one branch, so two sessions in one worktree are two
    /// agents writing one branch and that sentence no longer describes
    /// anything. The other variants return `None`: a relative path and a `git`
    /// that exited 128 are not controls refusing an action.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::PathHeld { .. } => Some(MethodologyRef {
                section: 7,
                subsection: None,
            }),
            _ => None,
        }
    }

    /// Whether a control refused the action, as opposed to a value failing to
    /// parse or a subprocess failing to run.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::PathHeld { .. })
    }
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathHeld { path, held, holder } => write!(
                f,
                "refused: {} overlaps {}, which session {holder} holds, and a worktree is one \
                 session's ({})",
                path.display(),
                held.display(),
                MethodologyRef {
                    section: 7,
                    subsection: None
                }
            ),
            Self::PathNotAbsolute { path } => {
                write!(f, "a worktree path is absolute: {}", path.display())
            }
            Self::PathNotPlain { path } => write!(
                f,
                "a worktree path carries no '..' component: {}",
                path.display()
            ),
            Self::PathIsRoot { path } => write!(
                f,
                "a worktree path is not a filesystem root: {}",
                path.display()
            ),
            Self::BranchName { name } => write!(
                f,
                "a branch or start point is non-empty, carries no whitespace and does not begin \
                 with '-': {name:?}"
            ),
            Self::Git {
                argv,
                status,
                stderr,
            } => {
                write!(f, "git {} exited ", argv.join(" "))?;
                match status {
                    Some(code) => write!(f, "{code}")?,
                    None => f.write_str("on a signal")?,
                }
                write!(f, ": {stderr:?}")
            }
            Self::GitUnavailable { message } => write!(f, "git could not be started: {message}"),
        }
    }
}

impl Error for WorktreeError {}

/// Whether `inner` is `outer` or sits inside it, by path components.
///
/// No methodology section applies: this is the path arithmetic behind
/// [`Worktree::overlaps`] and [`WorktreeSet::holder`].
fn nested(outer: &Path, inner: &Path) -> bool {
    let mut outer = outer.components();
    let mut inner = inner.components();
    loop {
        match (outer.next(), inner.next()) {
            (None, _) => return true,
            (Some(_), None) => return false,
            (Some(left), Some(right)) if left == right => {}
            (Some(_), Some(_)) => return false,
        }
    }
}

/// Reads a branch or start point, refusing one `git` would read as an option.
///
/// No methodology section applies: this is input validation. The three rules
/// are emptiness, whitespace and a leading `-`; the last is what stops a
/// session name from becoming a `git` flag when the argv is executed.
fn branch_name(name: &str) -> Result<String, WorktreeError> {
    if name.is_empty()
        || name.starts_with('-')
        || name.chars().any(|character| character.is_whitespace())
    {
        return Err(WorktreeError::BranchName {
            name: name.to_owned(),
        });
    }
    Ok(name.to_owned())
}

/// A path as the string an argv carries.
///
/// No methodology section applies. `to_string_lossy` is used rather than a
/// refusal on non-UTF-8 because the path came from the caller and a worktree
/// under a non-UTF-8 directory name is a machine's business, not a rule's; the
/// invocation that follows fails loudly if the lossy form names nothing.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A subprocess's standard error as a bounded, quoted string.
///
/// No methodology section applies: this is CLAUDE.md's rule that what a
/// dependency writes is data, applied to bytes that end up in a log.
fn truncated(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.len() <= STDERR_CAP {
        return trimmed.to_owned();
    }
    let mut end = STDERR_CAP;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &trimmed[..end])
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use super::*;

    /// A ULID-shaped identifier, for a session that only has to be distinct.
    fn session(tail: &str) -> Id {
        let text = format!("01ARZ3NDEKTSV4RRFFQ69{tail}");
        Id::parse(&text).expect("the fixture identifier is a ULID")
    }

    /// An absolute path on every platform the product ships to, naming
    /// nothing, built from the parts given.
    ///
    /// The fixtures used to be written `/w/one`. That is absolute on unix and
    /// not on Windows, where [`Path::is_absolute`] wants a prefix (`C:`) as
    /// well as a root, so every fixture built one refused at construction and
    /// the suite went red on one of the three platforms this product ships to.
    /// The rule was right and the fixture was not, which is the distinction
    /// this helper exists to keep: it roots the parts at
    /// [`std::env::temp_dir`], which is absolute on both, and asserts that
    /// rather than assuming it.
    ///
    /// Nothing here is created on disk. These are values, and the only test in
    /// this module that asks the filesystem anything uses [`Scratch`].
    fn absolute(parts: &[&str]) -> PathBuf {
        let mut path = std::env::temp_dir();
        assert!(
            path.is_absolute(),
            "the temporary directory is absolute on every platform this runs on: {}",
            path.display()
        );
        for part in parts {
            path.push(part);
        }
        path
    }

    /// The root of the volume the temporary directory is on: `/` or `C:\`.
    ///
    /// Written as the last ancestor rather than as a literal, for the same
    /// reason as [`absolute`]: the spelling is the platform's.
    fn filesystem_root() -> PathBuf {
        std::env::temp_dir()
            .ancestors()
            .last()
            .expect("every path has at least one ancestor")
            .to_path_buf()
    }

    /// A directory under the temporary directory that removes itself, whatever
    /// the test does.
    ///
    /// This is how the one test that runs real git leaves nothing behind. The
    /// name carries the process identifier, a counter and the wall clock, so
    /// two runs and two threads of one run cannot collide, and the removal is
    /// in `Drop` so it happens on a panic as well as on a return.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("the clock is after 1970")
                .as_nanos();
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0030-{label}-{}-{unique}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("the temporary directory is writable");
            Self {
                path: usable(&path),
            }
        }
    }

    /// The canonical form of a directory that exists, unless canonicalising it
    /// would produce a path the tools this module drives cannot use.
    ///
    /// Canonicalising matters on macOS, where the temporary directory is
    /// reached through a symbolic link (`/var` to `/private/var`) and two
    /// spellings of one directory would otherwise appear in one test. On
    /// Windows [`std::fs::canonicalize`] returns a verbatim path (`\\?\C:\...`),
    /// and git is the tool two tests here hand that path to: the verbatim form
    /// is exactly the one it is known to handle badly. So the canonical form is
    /// taken when it is usable and the original is kept when it is not, and the
    /// question is asked of the path rather than of the operating system name,
    /// because it is a fact about the path.
    fn usable(path: &Path) -> PathBuf {
        let Ok(canonical) = path.canonicalize() else {
            return path.to_path_buf();
        };
        let verbatim = canonical.components().next().is_some_and(
            |part| matches!(part, Component::Prefix(prefix) if prefix.kind().is_verbatim()),
        );
        if verbatim {
            path.to_path_buf()
        } else {
            canonical
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // ORI-P1-031's "no orphan" clause rests on the worktree path being one
    // session's, because spec/runbooks/recover-engine.md matches a stray
    // process to a session by that path.
    #[test]
    fn ori_p1_031_a_second_session_is_refused_a_worktree_path_a_live_one_holds() {
        let mut set = WorktreeSet::new();
        let one = absolute(&["w", "one"]);
        let first = Worktree::new(session("G5FAV"), &one, "feat/a").expect("a valid worktree");
        let second = Worktree::new(session("G5FAW"), &one, "feat/b").expect("a valid worktree");

        set.admit(first).expect("the first session is admitted");
        let refusal = set.admit(second).expect_err("the second must be refused");

        assert!(refusal.is_refusal(), "{refusal}");
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(7),
            "the refusal cites AICD §7"
        );
        assert_eq!(set.len(), 1, "the refused session holds nothing");
        assert_eq!(
            set.holder(&one),
            Some(&session("G5FAV")),
            "the path is still the first session's"
        );
    }

    #[test]
    fn ori_t_0030_a_worktree_inside_another_worktree_is_the_same_checkout_and_is_refused() {
        let outer = absolute(&["w", "one"]);
        let inner = absolute(&["w", "one", "inner"]);
        let mut set = WorktreeSet::new();
        set.admit(Worktree::new(session("G5FAV"), &outer, "feat/a").expect("valid"))
            .expect("admitted");

        let inside = Worktree::new(session("G5FAW"), &inner, "feat/b").expect("valid");
        let refusal = set.admit(inside).expect_err("a nested path is refused");
        assert!(refusal.is_refusal(), "{refusal}");

        // And the other direction: the outer path is refused when the inner one
        // is held first, which a check written as "starts_with" one way round
        // would miss.
        let mut other = WorktreeSet::new();
        other
            .admit(Worktree::new(session("G5FAW"), &inner, "feat/b").expect("valid"))
            .expect("admitted");
        assert!(
            other
                .admit(Worktree::new(session("G5FAV"), &outer, "feat/a").expect("valid"))
                .is_err(),
            "the containing path is refused too"
        );
    }

    #[test]
    fn ori_t_0030_a_sibling_whose_name_extends_another_is_not_nested() {
        let mut set = WorktreeSet::new();
        set.admit(
            Worktree::new(session("G5FAV"), absolute(&["w", "session-1"]), "feat/a")
                .expect("valid"),
        )
        .expect("admitted");

        // session-10 starts with the string session-1 and is a different
        // directory. A component comparison is what tells them apart.
        set.admit(
            Worktree::new(session("G5FAW"), absolute(&["w", "session-10"]), "feat/b")
                .expect("valid"),
        )
        .expect("a sibling is admitted");
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn ori_t_0030_a_derived_path_is_the_session_identifier_so_two_sessions_cannot_collide() {
        let root = absolute(&["w"]);
        let first = Worktree::derive(&root, session("G5FAV"), "feat/a").expect("valid");
        let second = Worktree::derive(&root, session("G5FAW"), "feat/b").expect("valid");

        assert_eq!(first.path(), absolute(&["w", "01ARZ3NDEKTSV4RRFFQ69G5FAV"]));
        assert!(!first.overlaps(&second), "two ULIDs are two directories");

        let mut set = WorktreeSet::new();
        set.admit(first).expect("admitted");
        set.admit(second).expect("admitted");
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn ori_t_0030_one_session_is_refused_a_second_worktree() {
        let mut set = WorktreeSet::new();
        set.admit(
            Worktree::new(session("G5FAV"), absolute(&["w", "one"]), "feat/a").expect("valid"),
        )
        .expect("admitted");
        let again =
            Worktree::new(session("G5FAV"), absolute(&["w", "two"]), "feat/a").expect("valid");
        assert!(
            set.admit(again).is_err(),
            "a session with two worktrees is the engine having lost one"
        );
    }

    #[test]
    fn ori_t_0030_a_released_path_is_free_for_the_next_session() {
        let one = absolute(&["w", "one"]);
        let mut set = WorktreeSet::new();
        set.admit(Worktree::new(session("G5FAV"), &one, "feat/a").expect("valid"))
            .expect("admitted");
        let released = set
            .release(&session("G5FAV"))
            .expect("the worktree is returned");
        assert_eq!(released.path(), one);
        assert!(set.is_empty(), "nothing is held after the release");
        set.admit(Worktree::new(session("G5FAW"), &one, "feat/b").expect("valid"))
            .expect("the next session takes the path");
        assert_eq!(set.holder(&one), Some(&session("G5FAW")));
        assert!(set.release(&session("G5FAV")).is_none(), "released once");
    }

    #[test]
    fn ori_t_0030_a_current_directory_component_is_folded_and_needs_no_rule() {
        // Path::components drops `.`, so /w/./one and /w/one are one directory
        // to every comparison this module makes. This is asserted rather than
        // assumed: the refusal above is written as narrowly as it is because of
        // it, and a standard library that stopped folding would make that
        // narrowness wrong.
        let dotted = Worktree::new(session("G5FAV"), absolute(&["w", ".", "one"]), "feat/a")
            .expect("not refused");
        let plain =
            Worktree::new(session("G5FAW"), absolute(&["w", "one"]), "feat/b").expect("valid");
        assert!(
            dotted.overlaps(&plain),
            "two spellings of one directory overlap"
        );
        let mut set = WorktreeSet::new();
        set.admit(plain).expect("admitted");
        assert!(
            set.admit(dotted).is_err(),
            "and the set refuses the second spelling too"
        );
    }

    #[test]
    fn ori_t_0030_a_path_that_could_be_spelled_two_ways_is_refused_at_construction() {
        // Each case asserts WHICH refusal it got, not merely that it got one.
        // Written the loose way, this test passed on Windows for the wrong
        // reason: `/w/../one` and `/` are both non-absolute there, so
        // PathNotAbsolute answered every case and the parent-directory rule and
        // the root rule were checked by nobody on that platform while the test
        // stayed green. A refusal test that does not name the refusal cannot
        // tell "the rule I am testing fired" from "some earlier rule fired".
        let cases = [
            (
                PathBuf::from("w/one"),
                WorktreeError::PathNotAbsolute {
                    path: PathBuf::from("w/one"),
                },
                "relative on every platform",
            ),
            (
                absolute(&["w", "..", "one"]),
                WorktreeError::PathNotPlain {
                    path: absolute(&["w", "..", "one"]),
                },
                "a parent-directory component",
            ),
            (
                filesystem_root(),
                WorktreeError::PathIsRoot {
                    path: filesystem_root(),
                },
                "names no directory of its own",
            ),
        ];
        for (path, expected, why) in cases {
            let error = Worktree::new(session("G5FAV"), &path, "feat/a")
                .expect_err(&format!("{} is refused: {why}", path.display()));
            assert_eq!(error, expected, "{} is refused for {why}", path.display());
            assert!(
                !error.is_refusal(),
                "a path that did not parse is not a control refusing an action: {error}"
            );
            assert!(
                error.methodology_ref().is_none(),
                "and it cites no section: {error}"
            );
        }
    }

    #[test]
    fn ori_t_0030_a_branch_or_start_point_git_would_read_as_an_option_is_refused() {
        // The variant is asserted for the same reason as in the test above: a
        // path fixture that refused first would make this test pass while the
        // branch rule was never reached.
        let one = absolute(&["w", "one"]);
        for name in ["", "--force", "-b", "two words"] {
            assert_eq!(
                Worktree::new(session("G5FAV"), &one, name),
                Err(WorktreeError::BranchName {
                    name: name.to_owned()
                }),
                "branch {name:?} is refused as a branch name"
            );
            let worktree = Worktree::new(session("G5FAV"), &one, "feat/a").expect("valid");
            assert_eq!(
                worktree.add_argv(&absolute(&["repo"]), name),
                Err(WorktreeError::BranchName {
                    name: name.to_owned()
                }),
                "start point {name:?} is refused as a start point"
            );
        }
    }

    #[test]
    fn ori_t_0030_the_argv_names_the_repository_the_branch_and_the_path_and_no_force() {
        // The two paths are whatever this platform spells them, and the
        // assertion reads them back through display_path rather than through a
        // literal, so the shape of the argv is what is checked here and not the
        // separator the platform uses.
        let repo = absolute(&["repo"]);
        let one = absolute(&["w", "one"]);
        let worktree = Worktree::new(session("G5FAV"), &one, "feat/a").expect("valid");
        assert_eq!(
            worktree.add_argv(&repo, "main").expect("valid start point"),
            vec![
                "-C".to_owned(),
                display_path(&repo),
                "worktree".to_owned(),
                "add".to_owned(),
                "-b".to_owned(),
                "feat/a".to_owned(),
                display_path(&one),
                "main".to_owned(),
            ]
        );
        let remove = worktree.remove_argv(&repo);
        assert_eq!(
            remove,
            vec![
                "-C".to_owned(),
                display_path(&repo),
                "worktree".to_owned(),
                "remove".to_owned(),
                display_path(&one),
            ]
        );
        assert!(
            !remove.iter().any(|argument| argument == "--force"),
            "removal never forces: uncommitted work in a worktree is not the engine's to delete"
        );
    }

    // The argv above is asserted against a list. This runs it. A well spelled
    // and wrong invocation is exactly what an assertion over a Vec<String>
    // cannot tell from a right one.
    #[test]
    fn ori_t_0030_git_really_creates_and_removes_the_worktree_the_argv_names() {
        let scratch = Scratch::new("git");
        let repo = scratch.path.join("repo");
        fs::create_dir_all(&repo).expect("the scratch directory is writable");

        let git = SystemGit::new(&repo).expect("an absolute repository path");
        // A repository with one commit, so that a branch has somewhere to start.
        for argv in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "ori@example.invalid"],
            vec!["config", "user.name", "ORI-T-0030"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "root"],
        ] {
            let mut full = vec!["-C".to_owned(), display_path(&repo)];
            full.extend(argv.into_iter().map(str::to_owned));
            git.run(&full)
                .expect("git is on PATH and the scratch repository accepts it");
        }

        let worktree = Worktree::new(
            session("G5FAV"),
            scratch.path.join("session"),
            "feat/ori-t-0030",
        )
        .expect("a valid worktree");
        assert!(!worktree.exists(), "nothing is there yet");

        git.add(&worktree, "main").expect("the add argv works");
        assert!(worktree.exists(), "the checkout is on disk");
        assert!(
            worktree.path().join(".git").exists(),
            "and it is a git worktree, not an empty directory"
        );

        git.remove(&worktree).expect("the remove argv works");
        assert!(!worktree.exists(), "and the directory is gone");
    }

    #[test]
    fn ori_t_0030_a_worktree_holding_uncommitted_work_is_not_removed_and_says_so() {
        let scratch = Scratch::new("dirty");
        let repo = scratch.path.join("repo");
        fs::create_dir_all(&repo).expect("writable");
        let git = SystemGit::new(&repo).expect("absolute");
        for argv in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "user.email", "ori@example.invalid"],
            vec!["config", "user.name", "ORI-T-0030"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "root"],
        ] {
            let mut full = vec!["-C".to_owned(), display_path(&repo)];
            full.extend(argv.into_iter().map(str::to_owned));
            git.run(&full).expect("git is on PATH");
        }
        let worktree = Worktree::new(
            session("G5FAV"),
            scratch.path.join("session"),
            "feat/ori-t-0030",
        )
        .expect("valid");
        git.add(&worktree, "main").expect("created");
        fs::write(worktree.path().join("unsaved.txt"), b"an agent's work")
            .expect("the worktree is writable");

        let error = git
            .remove(&worktree)
            .expect_err("a dirty worktree is not removed");
        assert!(
            matches!(error, WorktreeError::Git { .. }),
            "the failure is git's, reported: {error}"
        );
        assert!(
            worktree.exists(),
            "and the uncommitted work is still there, which is the point"
        );
        assert!(
            !error.is_refusal() && error.methodology_ref().is_none(),
            "a subprocess that refused is not a control citing a methodology section"
        );
    }

    #[test]
    fn ori_t_0030_every_refusal_this_module_makes_carries_a_reason_that_resolves() {
        // The paths below are payloads of error values, never arguments to
        // Worktree::new, so they are inert strings on every platform and the
        // absolute-path rule has nothing to say about them.
        let errors = [
            WorktreeError::PathHeld {
                path: PathBuf::from("/w/one"),
                held: PathBuf::from("/w/one"),
                holder: session("G5FAV"),
            },
            WorktreeError::PathNotAbsolute {
                path: PathBuf::from("w"),
            },
            WorktreeError::PathNotPlain {
                path: PathBuf::from("/w/../one"),
            },
            WorktreeError::PathIsRoot {
                path: PathBuf::from("/"),
            },
            WorktreeError::BranchName {
                name: "--force".to_owned(),
            },
            WorktreeError::Git {
                argv: vec!["worktree".to_owned()],
                status: Some(128),
                stderr: "fatal".to_owned(),
            },
            WorktreeError::GitUnavailable {
                message: "not found".to_owned(),
            },
        ];
        let mut refusals = 0;
        for error in errors {
            assert!(!error.to_string().is_empty(), "every error says something");
            match error.methodology_ref() {
                Some(reason) => {
                    assert!(error.is_refusal(), "a reason belongs to a refusal: {error}");
                    assert!(
                        reason.resolves(),
                        "{reason} names a section the methodology carries"
                    );
                    refusals += 1;
                }
                None => assert!(!error.is_refusal(), "a refusal carries a reason: {error}"),
            }
        }
        assert_eq!(refusals, 1, "this module refuses exactly one thing");
    }

    #[test]
    fn ori_t_0030_a_dangling_link_where_a_worktree_was_is_still_residue() {
        let scratch = Scratch::new("link");
        let worktree =
            Worktree::new(session("G5FAV"), scratch.path.join("gone"), "feat/a").expect("valid");
        assert!(!worktree.exists(), "nothing there");

        // Every platform: a real directory is seen.
        fs::create_dir_all(worktree.path()).expect("the scratch directory is writable");
        assert!(worktree.exists(), "a directory that is there is residue");
        fs::remove_dir_all(worktree.path()).expect("removable");
        assert!(!worktree.exists(), "and one that is gone is not");

        // The dangling link half is unix only, and this is the one place in
        // these two files where a platform difference is real rather than a
        // fixture being lazy: creating a symbolic link on Windows needs a
        // privilege a test runner may not hold, so a Windows run would report a
        // missing privilege as a failure of this module. The assertions above
        // run everywhere, so this test checks something on every platform
        // rather than quietly checking nothing on one.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(scratch.path.join("nowhere"), worktree.path())
                .expect("the scratch directory is writable");
            assert!(
                worktree.exists(),
                "a link to nothing is residue; Path::exists would follow it and say no"
            );
        }
    }

    // The fixtures are the thing that went red on Windows, so the property
    // they have to have is a test of its own rather than an assumption inside
    // two helpers. Every assertion here is about the path this platform built,
    // so it is the same test everywhere and it is not the same input anywhere.
    #[test]
    fn ori_t_0030_the_fixture_paths_are_absolute_wherever_the_suite_runs() {
        let path = absolute(&["w", "one"]);
        assert!(
            path.is_absolute(),
            "a fixture path this platform refuses is a red suite and not a finding: {}",
            path.display()
        );
        assert!(
            Worktree::new(session("G5FAV"), &path, "feat/a").is_ok(),
            "and the rule accepts it: {}",
            path.display()
        );

        let root = filesystem_root();
        assert!(root.is_absolute(), "{} is absolute", root.display());
        assert!(
            !root
                .components()
                .any(|part| matches!(part, Component::Normal(_))),
            "{} names no directory of its own, which is what makes it the root",
            root.display()
        );
        assert_eq!(
            Worktree::new(session("G5FAV"), &root, "feat/a"),
            Err(WorktreeError::PathIsRoot { path: root.clone() }),
            "and the rule refuses it as a root rather than as anything else"
        );
    }

    #[test]
    fn ori_t_0030_a_subprocess_message_is_bounded_data() {
        let flood = vec![b'x'; STDERR_CAP * 3];
        let held = truncated(&flood);
        assert!(held.len() <= STDERR_CAP + 3, "bounded: {}", held.len());
        assert!(held.ends_with("..."), "and says it was cut");
        assert_eq!(truncated(b"  fatal: no  "), "fatal: no");
        // Invalid UTF-8 is carried, not refused: it is data.
        assert!(!truncated(&[0xff, 0xfe]).is_empty());
    }
}
