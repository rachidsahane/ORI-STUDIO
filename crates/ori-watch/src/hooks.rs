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
//! without the marker belongs to someone else and is never overwritten. It is
//! kept at `hooks/ori-original/<hook>`, under **its own file name**, and the
//! installed script runs it after recording (same arguments, same standard
//! input, and its exit status is the hook's, so a `pre-push` that refuses a
//! push still refuses it). The file name matters: a hook that decides what to
//! do from the name it was run under (`basename "$0"`, as hook managers do)
//! would find a different name and run nothing, and exit zero, which silently
//! drops a veto. What changes for such a hook is the directory it runs from:
//! one level below `hooks/`, so a hook that finds its files by `$0`-relative
//! path (`$(dirname "$0")/..`) finds a different place. A hook that is a
//! relative symbolic link is kept as a link whose target is adjusted for that
//! one level, so it still reaches what it stood for. The report says so
//! ([`InstallOutcome::InstalledPreserving`]). If the preserved name is taken
//! the installer refuses ([`HooksError::PreservedNameTaken`]) rather than
//! choose which of two hooks to lose. `ori-original/` is the installer's, so an
//! `ori-original` that is a link, or not a directory, refuses the install
//! ([`HooksError::HooksDirNotADirectory`]) whatever the hooks are. A link in
//! its place would make the scripts run whatever it leads to, themselves
//! included. The entries inside it that the scripts run, those named for a hook
//! this installer replaces, are judged like an entry of the hooks directory
//! (below) when they are links; a link kept by a first install passes again,
//! since it was judged where it stood before.
//!
//! **Two arrangements are refused, and each is looked for in every entry of the
//! hooks directory, not only the four hooks**, and in the links of
//! `ori-original/` named for a hook the installer replaces: a link that reaches
//! a hook the installer replaces, and a link whose way enters a directory where
//! a path names something different for each process, whatever it would reach.
//!
//! The first: a link that, followed to its
//! end, reaches the place of a hook the installer replaces cannot be kept
//! safely. `post-merge -> post-commit` once kept would resolve to the engine's
//! own script, so the hook it stood for, a `pre-push` veto included, would
//! stop running without a word. The same holds through a chain
//! (`pre-push -> relay -> post-commit`), and it holds for a link that is not a
//! managed hook at all: `post-checkout -> post-merge` would run the engine's
//! `post-merge` script at every checkout in place of the hook the link stood
//! for: the script's own check of the name it was run under keeps it from
//! recording (the section on a script not run by git for its own event,
//! below), but the hook the link stood for no longer runs. Each hop is
//! resolved against its own directory with the operating system's own reading
//! of `..` and of links to directories. A way that runs through
//! `ori-original/`, which the installer creates, or through an entry of it
//! named for a hook it replaces, before that is there is refused, whatever it
//! would reach (the installer is not modelled, and
//! `follow_chain_to_managed_hook` says why); any other directory that does not
//! exist yet is read as git's process will find it if it is later made as a
//! real one: the way is read physically, with the links before the missing
//! directory followed where they stand and a `..` that climbs back to a
//! directory that is there going on physically, and its text is read as a
//! second look, refused if that reading lands on a hook's place at that hop
//! (`hooks/L -> ../refs` with `L/remotes/../../hooks/post-merge` reaches
//! nothing until the first fetch makes `refs/remotes`, and the engine's
//! `post-merge` from then on). A link to a same-named hook in some other
//! directory (a team's shared hooks) is not this. A link that cannot be read,
//! or an entry that cannot be looked at for a reason other than not being
//! there, refuses the installation as an IO error rather than passing it
//! ([`HooksError::Io`]).
//! On macOS and Windows, whose file systems are case-insensitive by
//! default, names are compared without regard to ASCII case, so a link to
//! `Post-Commit` counts; a case-sensitive volume there is refused where it
//! need not be, which is the safe side of the choice
//! (`CASE_INSENSITIVE_NAMES`). That installation fails
//! ([`HooksError::HookLinksToManagedHook`]) and writes nothing.
//!
//! The second: a link whose way enters `/proc` or `/dev/fd`, where a path names
//! something different for each process, is refused
//! ([`HooksError::HookLinkDependsOnProcess`]) whatever it reaches, because the
//! installer's process cannot judge what git's will run: git runs a hook with
//! its working directory at the worktree's root. That installation fails and
//! writes nothing. Git also makes directories in
//! `.git` after the install and before it runs a hook (`logs/` at the first
//! commit, `rebase-merge/` during a rebase), so past a component that is not
//! there the way is followed by its text, until a `..` climbs back to a
//! directory that is there, and then physically again. It assumes that a
//! directory made later is a real directory. The working tree is judged as it
//! stands at the install, and a later checkout can make a link or a file where
//! a way passes, so that it reaches a hook it did not reach then: the script's
//! own check of the name it was run under keeps such a way from recording (the
//! section on a script not run by git for its own event, below), and the next
//! `install` or `status` refuses it.
//!
//! The installation is built so that a hook of someone else's is not lost
//! to a failure or to a second installer running at the same time:
//!
//! - Every refusal is decided for all four hooks before any file is written.
//! - The script is written first, to a uniquely named temporary file in the
//!   hooks directory (`create_new`, so no two writers share one).
//! - The existing hook is then given its second name by `link(2)`, which fails
//!   with "exists" rather than replace what is there, and only then is the
//!   temporary renamed over the hook's name. There is no moment with no hook,
//!   and a failure before the rename leaves the original where it was (the
//!   second name made for it is removed).
//! - A hook that was absent at the decision is put in place by `link(2)` too,
//!   and a hook of the engine's that is rewritten is looked at again just
//!   before the rename; a hook that appeared or changed meanwhile is refused
//!   ([`HooksError::HookAppeared`]), not replaced. Two windows remain, and
//!   they are not closed: the instruction between that last look and the
//!   rename of a hook the engine itself wrote; and, for a hook of someone
//!   else's, the interval between the `link(2)` that gives it its second name
//!   and the rename that replaces its first. A third tool that *replaces* the
//!   hook in that interval (writes a new file and renames it into place, as
//!   an installer does) has its write lost to our rename; a tool that writes
//!   the hook in place is not lost, because it writes the file both names
//!   share.
//! - The installer needs hard links: a file system without them (some network
//!   and removable ones) makes installation fail loudly instead of falling
//!   back to a rename that could replace.
//!
//! Installing again changes nothing ([`InstallOutcome::Unchanged`]); a marked
//! hook whose text differs, a changed spool for example, is rewritten
//! ([`InstallOutcome::Updated`]).
//!
//! Everything read from the repository here is read as a plain file: a hook,
//! `.git/config`, the spool log. None is read through a link, none can make a
//! read wait on a pipe, and the handle that was opened is the one checked
//! (`crate::watcher`'s `open_plain_file`). A `.git/config` that is a link, not
//! a regular file or too long to read in full is refused
//! ([`HooksError::ConfigNotReadable`]): whether `core.hooksPath` is set could
//! not be established.
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
//! executable, and it judges the links as an install does.
//!
//! # A script not run by git for its own event records nothing (AICD §13)
//!
//! Git runs a hook as `<hooks directory>/<its event's name>`. A script that
//! finds it was run under another name, or from `ori-original/`, was reached
//! through a link or run by another script: git did not run it for its own
//! event. It prints one line on standard error, which gives the name it was
//! run as and then the directory it was run in, each through `ori_clean` (so
//! that a long path cannot hide the name), reads its standard input unless that
//! is a terminal, records nothing, runs nothing and exits 0. It
//! reads its input first so that a writer that feeds this event is never cut
//! off by a closed pipe, and it exits 0 because git takes a non-zero status
//! from `post-checkout` as the checkout's own status, and from a vetoing event
//! as a veto: a script reached by mistake must not block git, which is the
//! principle of the section above for an unwritable spool.
//!
//! This closes, at run time, the false records that a link in the hooks
//! directory can make: `post-checkout -> post-merge` would otherwise record a
//! `post-merge` event at every checkout, and a link in `ori-original/` to an
//! engine script would run it a second time. The install-time checks of the
//! links ([`GitHooks::install`], [`GitHooks::status`]) judge the hooks
//! directory as it stands, and cannot judge what a later checkout puts on a
//! way that passes through the working tree. The script's check does not
//! depend on the tree: whatever a checkout puts on the way of a hook that git
//! runs, the name it was run under is the name of the event or it is not. The
//! install-time checks stay, because they keep a veto from being silently
//! replaced, which a run-time check cannot restore.
//!
//! What the check does not stop, each of which needs code that already runs
//! as the user, a change to the hooks directory itself (which
//! [`GitHooks::status`] and the next [`GitHooks::install`] judge), or, for the
//! last item, an arrangement made in another repository:
//!
//! - a script that runs the engine's hook under its own name passes the check
//!   and records. That covers running it by its path
//!   (`"$(dirname "$0")/post-merge"`), through `git hook run post-merge`, or
//!   through a hook set in git's configuration (`hook.<name>.command`, in the
//!   versions of git that have it);
//! - a preserved hook that runs `"$(dirname "$0")/../post-commit"` passes it
//!   too, and records again for each round it makes: this is the `$0`-relative
//!   change the section "What is installed" already names;
//! - a process of the same user that writes the spool directly records
//!   whatever it likes;
//! - a later checkout that changes what a kept link's working-tree script does:
//!   that script runs under the hook's own name and does what the branch now
//!   says, and it records falsely only by running the engine's script under
//!   its own name, the first limit. Likewise a checkout can turn a file that
//!   another script runs, by a path whose last name is a hook's
//!   (`tools/post-merge`), into a link to the engine's script, and that run
//!   passes the check and records;
//! - another repository whose hooks are links into this hooks directory, or
//!   whose `core.hooksPath` (its own or the user's global one) names it, runs
//!   the engine's scripts under their own names. Each of its events passes the
//!   check and is recorded in this spool, with that repository's object ids.
//!   The hooks kept in `ori-original/` do not run for it, and neither does any
//!   hook of its own that its link stood for, a veto included. Nothing in this
//!   repository shows it, and `status` reports the hooks installed. ORI-T-0055
//!   is to drop a record whose object ids this repository does not hold.
//!
//! One false negative is known. A hooks directory that is itself named
//! `ori-original` makes every run skip. Only a `core.hooksPath` can make git
//! run hooks from such a directory, and `install` refuses one set in the
//! repository's own configuration.
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
//! - A `.git` swapped for a link while an installation runs is still written
//!   through, for the rest of that run. `.git` is looked at when the installer
//!   is built and at the start of [`GitHooks::install`] and
//!   [`GitHooks::status`], not between the writes that follow, and `std` has
//!   no `openat` to hold the directory between a look and a write; the next
//!   call refuses it ([`HooksError::GitDirNotADirectory`]). A hostile
//!   repository that holds still is in scope, and a live writer racing the
//!   installer is not claimed against, which is the limit `crate::watcher`
//!   states for its own checks.
//! - A linked worktree's `.git` is a file; the installer refuses it
//!   ([`HooksError::GitDirNotADirectory`]). Install into the repository that
//!   owns the worktree: hooks live in the common directory and run for every
//!   worktree.
//! - The scripts are POSIX `sh`. Git for Windows runs them with its bundled
//!   shell; the executing tests below run on Unix only.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ori_core::error::MethodologyRef;
use ori_core::types::Timestamp;

use crate::watcher::{PlainReadError, open_plain_file, read_plain_file};

/// The file in the spool directory the hooks append to.
pub const SPOOL_LOG_NAME: &str = "hook-events.log";

/// The marker on the second line of every script this module writes. A line
/// that starts with it, whatever version follows, is the engine's own.
const MARKER: &str = "# ori-studio-hook v";

/// The directory beside the hooks that holds the hooks that were here before,
/// each under its own file name.
const PRESERVED_DIR: &str = "ori-original";

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
    /// `.git/hooks`, or `ori-original/` in it, is a link or not a directory.
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
    /// An entry of the hooks directory is a link that, directly or through
    /// other links, reaches a hook this installer also replaces
    /// (`post-merge -> post-commit`, `pre-push -> relay -> post-commit`,
    /// `post-checkout -> post-merge`), or whose way runs through
    /// `ori-original/` before this installer has made what it would reach
    /// there (`ori-original/../post-merge`, which is the hooks directory's own
    /// `post-merge` once the directory exists). Kept as a link, or left in
    /// place, it would, or could once the install has run, resolve to the
    /// engine's own script: the hook it stood for, a `pre-push` veto included,
    /// would never run again, or the engine's script would run in an unrelated
    /// hook's place, which then no longer runs (the script's own check of the
    /// name it was run under keeps it from recording). Nothing was written.
    HookLinksToManagedHook {
        /// The entry that is a link.
        path: PathBuf,
        /// What that link says it points at, as written: the first hop of the
        /// chain.
        target: PathBuf,
    },
    /// An entry of the hooks directory is a link whose way, followed
    /// physically, enters a directory where a path names something different
    /// for each process (`post-checkout -> /proc/self/cwd/.git/hooks/post-merge`).
    /// The installer resolves it as its own process sees it; git's process,
    /// which runs the hook from the worktree's root, may see the engine's own
    /// script. Nothing was written.
    HookLinkDependsOnProcess {
        /// The entry that is a link.
        path: PathBuf,
        /// What that link says it points at, as written.
        target: PathBuf,
        /// The per-process directory the way entered, as the walk resolved it.
        through: PathBuf,
    },
    /// A hook appeared, or changed, between deciding what to do and doing it.
    /// Nothing was overwritten; running the installation again decides afresh.
    HookAppeared {
        /// The hook's path.
        path: PathBuf,
    },
    /// `.git/config` is a link, not a regular file, or too long to be read in
    /// full, so whether `core.hooksPath` is set cannot be established, and
    /// hooks installed without knowing might never run.
    ConfigNotReadable {
        /// The path.
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
    /// - [`HooksError::HooksPathConfigured`] and
    ///   [`HooksError::ConfigNotReadable`] cite AICD §14: hooks that git will
    ///   never run are installed and reporting nothing.
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
            Self::HooksPathConfigured { .. } | Self::ConfigNotReadable { .. } => 14,
            Self::GitDirNotADirectory { .. }
            | Self::SpoolNotPlain { .. }
            | Self::HooksDirNotADirectory { .. }
            | Self::HookNotAFile { .. }
            | Self::PreservedNameTaken { .. }
            | Self::HookAppeared { .. }
            | Self::HookLinksToManagedHook { .. }
            | Self::HookLinkDependsOnProcess { .. }
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
            Self::HookLinksToManagedHook { path, target } => write!(
                f,
                "refused: {} is a link to {}, which leads to a hook this installer replaces, or \
                 whose way runs through ori-original/, where this installer keeps the hooks it \
                 replaces, so the engine's own script could run in place of the hook it stands \
                 for; point it at a file of its own and install again",
                path.display(),
                target.display()
            )
            .and_then(|()| cite(f, self)),
            Self::HookLinkDependsOnProcess {
                path,
                target,
                through,
            } => write!(
                f,
                "refused: {} is a link to {}, whose way runs through {}, where a path names \
                 something different for each process that follows it, so what git's process \
                 would run cannot be judged from the installer's; point it at a file of its own \
                 and install again",
                path.display(),
                target.display(),
                through.display()
            )
            .and_then(|()| cite(f, self)),
            Self::HookAppeared { path } => write!(
                f,
                "refused: a hook appeared or changed at {} while installing; nothing was \
                 overwritten, run the installation again",
                path.display()
            )
            .and_then(|()| cite(f, self)),
            Self::ConfigNotReadable { path } => write!(
                f,
                "refused: {} is a link, not a regular file, or too long to read in full, so \
                 whether core.hooksPath is set cannot be established",
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
    /// Another actor's hook was there. It is kept at `preserved`, under its
    /// own file name, and the installed script runs it after recording.
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
    /// Whether a preserved hook from before the engine sits beside it. The
    /// script runs it after recording when it is executable; this field
    /// reports only that an entry is there. It is computed only once
    /// `ori-original/` has been found to be absent or a real directory:
    /// [`GitHooks::status`] refuses ([`HooksError::HooksDirNotADirectory`])
    /// before it reports anything otherwise.
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
        check_git_dir(&git_dir)?;
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
    /// [`HooksError::GitDirNotADirectory`] when `.git` has become a link or a
    /// file since [`GitHooks::new`], [`HooksError::ConfigNotReadable`] and
    /// [`HooksError::HooksPathConfigured`] when `.git/config` cannot be read or
    /// sets `core.hooksPath`, [`HooksError::HooksDirNotADirectory`] when the
    /// hooks directory is a link or not a directory, or when the
    /// `ori-original` in it is (whatever the hooks are: a link there would make
    /// every script run whatever it leads to, itself included),
    /// [`HooksError::HookNotAFile`], [`HooksError::HookLinksToManagedHook`],
    /// [`HooksError::HookLinkDependsOnProcess`] and
    /// [`HooksError::PreservedNameTaken`] when what is in the hooks
    /// directory cannot be kept safely, [`HooksError::HookAppeared`] when a
    /// hook appeared or changed while installing, or [`HooksError::Io`].
    pub fn install(&self) -> Result<InstallReport, HooksError> {
        check_git_dir(&self.git_dir)?;
        self.refuse_if_hooks_path_is_configured()?;
        let hooks_dir = self.hooks_dir();
        refuse_a_hooks_dir_that_is_not_a_directory(&hooks_dir)?;
        // `ori-original/` is the installer's: absent, or a real directory,
        // whatever the hooks are, and the links in it that the scripts run
        // (those named for a hook this installer replaces) are judged below
        // like the entries of the hooks directory. The scripts run
        // `ori-original/<their own name>` when it is executable, so a link
        // there (`ori-original -> .`) makes each script run itself.
        refuse_a_preserved_dir_that_is_not_ours(&hooks_dir)?;
        // Decide every hook before writing any.
        self.refuse_if_a_link_reaches_a_managed_hook(&hooks_dir)?;
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
            outcomes.push((hook, self.apply(hook, plan)?));
        }
        Ok(InstallReport { outcomes })
    }

    /// How each hook stands now: AICD §14.
    ///
    /// This is the liveness check for the hooks: it reads, never writes. It
    /// reports that the files are in place, not that git will run them (see
    /// the module doc's "What is not covered").
    ///
    /// It judges the hooks directory, and the links in it and in
    /// `ori-original/`, exactly as [`GitHooks::install`] does, and in the same
    /// order. A link made after an install that reaches a hook this installer
    /// replaces, or whose way depends on the process that follows it, makes it
    /// refuse and not report the hooks healthy: the next `install` would
    /// refuse that link, and the hook such a link stood for no longer runs. It
    /// does not look at `core.hooksPath`.
    ///
    /// # Errors
    ///
    /// [`HooksError::GitDirNotADirectory`] when `.git` has become a link or a
    /// file since [`GitHooks::new`], [`HooksError::HooksDirNotADirectory`] when
    /// the hooks directory is a link or not a directory, or when the
    /// `ori-original` in it is (as for [`GitHooks::install`]: each script would
    /// run itself, or whatever the link leads to, and a report that every hook
    /// is installed would be false), [`HooksError::HookLinksToManagedHook`] and
    /// [`HooksError::HookLinkDependsOnProcess`] when a link in the hooks
    /// directory, or one in `ori-original/` named for a hook this installer
    /// replaces, is one that `install` refuses, [`HooksError::HookNotAFile`]
    /// when a directory sits where a hook belongs, [`HooksError::Io`] when
    /// `.git`, the hooks directory, `ori-original/`, a link or a hook file
    /// cannot be looked at.
    pub fn status(&self) -> Result<Vec<HookStatus>, HooksError> {
        check_git_dir(&self.git_dir)?;
        let hooks_dir = self.hooks_dir();
        refuse_a_hooks_dir_that_is_not_a_directory(&hooks_dir)?;
        refuse_a_preserved_dir_that_is_not_ours(&hooks_dir)?;
        self.refuse_if_a_link_reaches_a_managed_hook(&hooks_dir)?;
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
            let chains_to_original = fs::symlink_metadata(preserved_path(&hooks_dir, hook)).is_ok();
            statuses.push(HookStatus {
                hook,
                state,
                chains_to_original,
            });
        }
        Ok(statuses)
    }

    fn plan(&self, hook: GitHook) -> Result<Plan, HooksError> {
        let hooks_dir = self.hooks_dir();
        let path = hooks_dir.join(hook.file_name());
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
                // A link to a hook we replace was refused before any plan was
                // made (`refuse_if_a_link_reaches_a_managed_hook`), whatever
                // its name. `install` refused such an `ori-original` already,
                // and this repeats the look for the hook it is about to keep.
                let directory = hooks_dir.join(PRESERVED_DIR);
                match fs::symlink_metadata(&directory) {
                    Ok(metadata) => {
                        if metadata.file_type().is_symlink() || !metadata.is_dir() {
                            return Err(HooksError::HooksDirNotADirectory { path: directory });
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(HooksError::Io {
                            path: directory,
                            source,
                        });
                    }
                }
                let preserved = preserved_path(&hooks_dir, hook);
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
        let (text, truncated) = match read_plain_file(path, READ_CAP) {
            Ok(read) => read,
            // Swapped for a link or a pipe since the look above: not ours.
            Err(PlainReadError::Symlink | PlainReadError::NotARegularFile) => {
                return Ok(Examined::Foreign);
            }
            Err(PlainReadError::Io(source)) => {
                return Err(HooksError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        if truncated || !is_ours(&text) {
            return Ok(Examined::Foreign);
        }
        Ok(Examined::Ours {
            current: text == self.script_for(hook),
            executable: is_executable(&metadata),
        })
    }

    /// Does one hook's part of the installation: AICD §13.
    ///
    /// The script is written to a uniquely named temporary file first. Then,
    /// by the plan:
    ///
    /// - [`Plan::Write`]: the temporary is hard-linked to the hook's name,
    ///   which fails if anything is there now, so a hook that appeared since
    ///   the plan is never replaced.
    /// - [`Plan::Rewrite`]: the hook is looked at again; if it is still the
    ///   engine's, the temporary is renamed over it.
    /// - [`Plan::Preserve`]: the existing hook is hard-linked (a link is
    ///   re-pointed, see `preserve`) to its place under `ori-original/`, which
    ///   fails if that name is taken, so a preserved hook is never replaced,
    ///   and then the temporary is renamed over the hook's name. At no moment
    ///   is there no hook, and if anything fails before the rename the
    ///   original is still in place.
    fn apply(&self, hook: GitHook, plan: Plan) -> Result<InstallOutcome, HooksError> {
        if matches!(plan, Plan::Keep) {
            return Ok(InstallOutcome::Unchanged);
        }
        let hooks_dir = self.hooks_dir();
        let path = hooks_dir.join(hook.file_name());
        let temporary = self.write_temporary(hook)?;
        let outcome = self.place(hook, plan, &temporary, &path, &hooks_dir);
        // After a rename the temporary is gone; after a link it is a second
        // name for the hook; after a refusal it is litter.
        let _ = fs::remove_file(&temporary);
        outcome
    }

    fn place(
        &self,
        hook: GitHook,
        plan: Plan,
        temporary: &Path,
        path: &Path,
        hooks_dir: &Path,
    ) -> Result<InstallOutcome, HooksError> {
        let io_error = |source| HooksError::Io {
            path: path.to_path_buf(),
            source,
        };
        match plan {
            Plan::Keep => Ok(InstallOutcome::Unchanged),
            Plan::Write => {
                link_new(temporary, path)?;
                Ok(InstallOutcome::Installed)
            }
            Plan::Rewrite => match self.examine(hook, path)? {
                Examined::Ours { .. } => {
                    fs::rename(temporary, path).map_err(io_error)?;
                    Ok(InstallOutcome::Updated)
                }
                Examined::Absent => {
                    link_new(temporary, path)?;
                    Ok(InstallOutcome::Updated)
                }
                Examined::Foreign | Examined::Directory => Err(HooksError::HookAppeared {
                    path: path.to_path_buf(),
                }),
            },
            Plan::Preserve => {
                let directory = hooks_dir.join(PRESERVED_DIR);
                fs::create_dir_all(&directory).map_err(|source| HooksError::Io {
                    path: directory.clone(),
                    source,
                })?;
                let preserved = preserved_path(hooks_dir, hook);
                preserve(path, &preserved)?;
                if let Err(source) = fs::rename(temporary, path) {
                    // The original is still in place; do not leave a second
                    // name for it that would make the next run refuse.
                    let _ = fs::remove_file(&preserved);
                    return Err(io_error(source));
                }
                Ok(InstallOutcome::InstalledPreserving { preserved })
            }
        }
    }

    /// Writes the script for `hook` to a new, uniquely named file in the hooks
    /// directory and returns its path.
    fn write_temporary(&self, hook: GitHook) -> Result<PathBuf, HooksError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let script = self.script_for(hook);
        let hooks_dir = self.hooks_dir();
        for _ in 0..32 {
            let temporary = hooks_dir.join(format!(
                ".{}.ori-tmp-{}-{}",
                hook.file_name(),
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            match write_new_executable(&temporary, script.as_bytes()) {
                Ok(()) => return Ok(temporary),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => {
                    let _ = fs::remove_file(&temporary);
                    return Err(HooksError::Io {
                        path: temporary,
                        source,
                    });
                }
            }
        }
        Err(HooksError::Io {
            path: hooks_dir,
            source: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "no unused temporary name after 32 tries",
            ),
        })
    }

    /// Refuses the installation, or the status report, if any entry of the
    /// hooks directory is a link that, followed to its end, reaches the place
    /// of a hook this installer replaces: AICD §13.
    ///
    /// Every entry is looked at, not only the four hooks, and a chain is
    /// followed, not one link (`follow_chain_to_managed_hook`). For each link
    /// the per-process walk comes first
    /// (`way_enters_a_per_process_directory`): a link whose way enters `/proc`
    /// or `/dev/fd` is refused ([`HooksError::HookLinkDependsOnProcess`])
    /// before the chain is read, since the chain is read as the installer's
    /// process sees the way and not as git's does. Two arrangements
    /// get through a check of one link on the four names. `pre-push -> relay
    /// -> post-commit`: once `pre-push` is kept, its link still ends at
    /// `post-commit`, which is now the engine's script, so the veto it stood
    /// for stops running. `post-checkout -> post-merge`: the link is not ours
    /// to move, and `post-merge` becomes the engine's script, so every checkout
    /// runs the engine's script in place of the hook the link stood for, which
    /// then no longer runs (the script's own check keeps it from recording).
    /// Entries are looked at in name order, so the refusal names the same one
    /// each time. A hooks directory that does not exist yet has nothing to
    /// refuse. A link made after this look is not seen by it, as with every
    /// decision `install` makes before its writes.
    ///
    /// The entries of `ori-original/` that the installed scripts run, which are
    /// those named for a hook this installer replaces, are looked at too when
    /// `ori-original/` is a real directory (`install` and `status` refused it
    /// otherwise): each that is a link goes through the same two checks, the
    /// per-process walk and then the chain, and a refusal names that entry. A
    /// link a first install kept there was judged where it stood in the hooks
    /// directory, so it passes again.
    ///
    /// A way that first fails at `ori-original/`, which the installer creates,
    /// or at an entry of it named for a hook it replaces, is refused whatever
    /// it would reach (`ori-original/../post-merge`; `M/post-merge` where `M`
    /// links to `ori-original/..`): what the install will make there is not
    /// modelled, and refusing fails closed. A way that first fails at any other
    /// missing directory is read physically, as git's process will find it if
    /// that directory is later made as a real one, and its text is read as a
    /// second look that refuses the link if that reading lands on a hook's
    /// place at that hop, an over-refusal kept because it fails closed
    /// (`follow_chain_to_managed_hook` gives both readings). A physical reading
    /// that meets more than `LINK_HOPS_CAP` links refuses the installation, or
    /// the status report, as an IO error. A failure to look at anything that is
    /// there, other than a missing entry, refuses the installation, or the
    /// status report, as an IO error ([`HooksError::Io`]) and does not pass it:
    /// a check that could not be made is not a check that found nothing
    /// (AICD §14).
    fn refuse_if_a_link_reaches_a_managed_hook(&self, hooks_dir: &Path) -> Result<(), HooksError> {
        let listing = match fs::read_dir(hooks_dir) {
            Ok(listing) => listing,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(HooksError::Io {
                    path: hooks_dir.to_path_buf(),
                    source,
                });
            }
        };
        let io_error = |source| HooksError::Io {
            path: hooks_dir.to_path_buf(),
            source,
        };
        let mut links = Vec::new();
        for entry in listing {
            let entry = entry.map_err(io_error)?;
            if entry.file_type().map_err(io_error)?.is_symlink() {
                links.push(entry.path());
            }
        }
        links.sort();
        // The entries of `ori-original/` that the scripts run, when it is a
        // real directory, are judged by the same two checks, after those of
        // the hooks directory.
        links.extend(preserved_links_named_for_a_managed_hook(hooks_dir)?);
        // The directory was just listed, so failing to resolve it is not
        // absence: the check cannot be made, and the install is refused
        // (AICD §14).
        let canonical_hooks = fs::canonicalize(hooks_dir).map_err(io_error)?;
        for link in links {
            // First, whether the way depends on the process that follows it:
            // if it does, the chain below, which is the installer's process's
            // reading, could judge it wrongly.
            let through = way_enters_a_per_process_directory(&link, CASE_INSENSITIVE_NAMES)
                .map_err(|source| HooksError::Io {
                    path: link.clone(),
                    source,
                })?;
            if let Some(through) = through {
                let target = fs::read_link(&link).map_err(|source| HooksError::Io {
                    path: link.clone(),
                    source,
                })?;
                return Err(HooksError::HookLinkDependsOnProcess {
                    path: link,
                    target,
                    through,
                });
            }
            let reached = follow_chain_to_managed_hook(
                hooks_dir,
                &canonical_hooks,
                &link,
                CASE_INSENSITIVE_NAMES,
            )
            .map_err(|source| HooksError::Io {
                path: link.clone(),
                source,
            })?;
            if let Some(target) = reached {
                return Err(HooksError::HookLinksToManagedHook { path: link, target });
            }
        }
        Ok(())
    }

    fn refuse_if_hooks_path_is_configured(&self) -> Result<(), HooksError> {
        let config = self.git_dir.join("config");
        let text = match read_plain_file(&config, READ_CAP) {
            Ok((text, false)) => text,
            // Past the cap, a `hooksPath` could be in the part not read.
            Ok((_, true)) | Err(PlainReadError::Symlink | PlainReadError::NotARegularFile) => {
                return Err(HooksError::ConfigNotReadable { path: config });
            }
            Err(PlainReadError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(PlainReadError::Io(source)) => {
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

/// Refuses the hooks directory unless it is absent or a real directory:
/// AICD §13.
///
/// A link in its place would make every write, and every read of a hook, go
/// wherever the link leads. This is looked at by [`GitHooks::install`] before
/// anything is decided and by [`GitHooks::status`] before anything is
/// reported. A hooks directory that is not there passes: `install` makes it,
/// and `status` reports every hook missing.
///
/// # Errors
///
/// [`HooksError::HooksDirNotADirectory`] when it is a link or not a directory,
/// [`HooksError::Io`] when it cannot be looked at for a reason other than not
/// being there.
fn refuse_a_hooks_dir_that_is_not_a_directory(hooks_dir: &Path) -> Result<(), HooksError> {
    match fs::symlink_metadata(hooks_dir) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(HooksError::HooksDirNotADirectory {
                    path: hooks_dir.to_path_buf(),
                });
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(HooksError::Io {
            path: hooks_dir.to_path_buf(),
            source,
        }),
    }
}

/// Refuses `ori-original/` in `hooks_dir` unless it is absent or a real
/// directory: AICD §13.
///
/// The installed scripts run `ori-original/<their own name>` when it is
/// executable, so a link there (`ori-original -> .`) makes each script find
/// itself and run itself, and a link out of the repository makes them run
/// whatever it leads to. A file there is not a directory a hook can be kept in.
/// This is looked at whatever the hooks are, by [`GitHooks::install`] before
/// anything is decided and by [`GitHooks::status`] before anything is
/// reported.
///
/// # Errors
///
/// [`HooksError::HooksDirNotADirectory`] naming `ori-original` when it is a link
/// or not a directory, [`HooksError::Io`] when it cannot be looked at for a
/// reason other than not being there.
fn refuse_a_preserved_dir_that_is_not_ours(hooks_dir: &Path) -> Result<(), HooksError> {
    let preserved_dir = hooks_dir.join(PRESERVED_DIR);
    match fs::symlink_metadata(&preserved_dir) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(HooksError::HooksDirNotADirectory {
                    path: preserved_dir,
                });
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(HooksError::Io {
            path: preserved_dir,
            source,
        }),
    }
}

/// The links in `hooks_dir/ori-original/` that are named for a hook this
/// installer replaces, sorted: the entries the installed scripts run.
///
/// Only a real directory is listed; a link or a file there was refused by
/// `refuse_a_preserved_dir_that_is_not_ours` before this is reached, and an
/// absent one has no entries. Anything that cannot be looked at, other than a
/// missing directory, is an IO error and refuses the install (AICD §14).
fn preserved_links_named_for_a_managed_hook(hooks_dir: &Path) -> Result<Vec<PathBuf>, HooksError> {
    let directory = hooks_dir.join(PRESERVED_DIR);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Ok(Vec::new()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(HooksError::Io {
                path: directory,
                source,
            });
        }
    }
    let listing = match fs::read_dir(&directory) {
        Ok(listing) => listing,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(HooksError::Io {
                path: directory,
                source,
            });
        }
    };
    let io_error = |source| HooksError::Io {
        path: directory.clone(),
        source,
    };
    let mut links = Vec::new();
    for entry in listing {
        let entry = entry.map_err(io_error)?;
        if names_a_managed_hook(&entry.file_name(), CASE_INSENSITIVE_NAMES)
            && entry.file_type().map_err(io_error)?.is_symlink()
        {
            links.push(entry.path());
        }
    }
    links.sort();
    Ok(links)
}

/// Creates `path`, which must not exist, writes `bytes` and makes it
/// executable.
fn write_new_executable(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    make_executable(path)
}

/// Gives `temporary` the name `path` if nothing has that name: a hard link
/// fails with `AlreadyExists` where a rename would silently replace.
fn link_new(temporary: &Path, path: &Path) -> Result<(), HooksError> {
    match fs::hard_link(temporary, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(HooksError::HookAppeared {
                path: path.to_path_buf(),
            })
        }
        Err(source) => Err(HooksError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Keeps the hook at `path` at `preserved`, which must not exist, without
/// removing it from `path`: AICD §13.
///
/// A regular file gets a hard link, so both names are the same file. A
/// symbolic link (on Unix) gets a new link with the same target, written
/// relative to the directory one level down that `preserved` is in, because a
/// relative target is read from the link's own directory. Both fail with
/// `AlreadyExists` if `preserved` is taken, so no preserved hook is replaced.
fn preserve(path: &Path, preserved: &Path) -> Result<(), HooksError> {
    let result = preserve_unchecked(path, preserved);
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(HooksError::PreservedNameTaken {
                path: preserved.to_path_buf(),
            })
        }
        Err(source) => Err(HooksError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(unix)]
fn preserve_unchecked(path: &Path, preserved: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        let target = fs::read_link(path)?;
        let target = if target.is_relative() {
            Path::new("..").join(target)
        } else {
            target
        };
        std::os::unix::fs::symlink(target, preserved)
    } else {
        fs::hard_link(path, preserved)
    }
}

#[cfg(not(unix))]
fn preserve_unchecked(path: &Path, preserved: &Path) -> io::Result<()> {
    fs::hard_link(path, preserved)
}

/// Whether `git_dir` is the repository's own directory: a real directory,
/// not a link and not a file.
///
/// Checked by [`GitHooks::new`] and again by every [`GitHooks::install`] and
/// [`GitHooks::status`], because `.git` can be replaced by a link after the
/// installer was built, and everything below it would then be read and
/// written through the link.
fn check_git_dir(git_dir: &Path) -> Result<(), HooksError> {
    let metadata = fs::symlink_metadata(git_dir).map_err(|source| HooksError::Io {
        path: git_dir.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(HooksError::GitDirNotADirectory {
            path: git_dir.to_path_buf(),
        });
    }
    Ok(())
}

/// The most links followed from one entry before the chain is given up on; the
/// limit the Linux kernel applies to one path is the same.
const LINK_HOPS_CAP: usize = 40;

/// Whether names in a hooks directory are compared without regard to case:
/// the default on macOS and Windows, where `Post-Commit` and `post-commit` are
/// one file. Elsewhere names are exact.
///
/// The cost of getting this wrong is not symmetric. Folding case on a
/// case-sensitive volume (an APFS volume formatted that way, say) refuses a
/// link to `Post-Commit` that was harmless, and the user renames it. Not
/// folding it on a case-insensitive one lets a link to the hook we replace
/// through, and the hook it stood for stops running. So the platforms whose
/// default is case-insensitive fold, and the rest do not; asking the file
/// system would need a file to be created in the hooks directory before
/// anything is decided.
///
/// The limit that remains: the final hop is compared by name, with ASCII case
/// folding chosen per platform, so on a volume whose case rule differs from its
/// platform's default (a case-insensitive directory on Linux, a case-sensitive
/// APFS volume) another spelling of a managed hook is judged by the platform
/// default. The resolver (`way_enters_what_the_install_creates`) also compares
/// `ori-original` and a hook-named entry of it by the platform default, so a
/// case-folding directory on a platform that does not fold (casefold ext4, vfat
/// on Linux) lets another spelling through.
const CASE_INSENSITIVE_NAMES: bool = cfg!(any(target_os = "macos", windows));

/// Whether `name` is the file name of a hook this installer replaces,
/// optionally ignoring ASCII case: AICD §13.
fn names_a_managed_hook(name: &std::ffi::OsStr, fold_case: bool) -> bool {
    name.to_str().is_some_and(|text| {
        GitHook::ALL.iter().any(|hook| {
            if fold_case {
                hook.file_name().eq_ignore_ascii_case(text)
            } else {
                hook.file_name() == text
            }
        })
    })
}

/// Whether `directory` and `hooks_dir` are one directory, by path or, where
/// the platform can say, by identity (which also sees through a difference of
/// letter case on a case-insensitive file system).
fn is_same_directory(directory: &Path, hooks_dir: &Path, canonical_hooks: &Path) -> bool {
    if directory == canonical_hooks {
        return true;
    }
    match (fs::metadata(directory), fs::metadata(hooks_dir)) {
        (Ok(left), Ok(right)) => match (identity_of(&left), identity_of(&right)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        },
        _ => false,
    }
}

/// `path` with `.` components dropped and each `..` removing the component
/// before it, by the text alone: no file is looked at.
///
/// A `..` never rises above a root or a drive prefix; it is dropped there. A
/// relative path that begins with `..` keeps it, since there is nothing before
/// it to remove. It reads a directory that does not exist yet, where
/// `canonicalize` has nothing to resolve; it is wrong for a `..` that follows
/// a link to a directory, which is why it is used only for a directory that is
/// not there, and only after `way_enters_what_the_install_creates` has looked
/// for one this install creates.
fn lexically_normal(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                Some(Component::ParentDir | Component::CurDir) | None => parts.push(component),
            },
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

/// `path` resolved by the operating system, if it is a directory that is
/// there; `None` if it is missing, a link that goes nowhere, or not a
/// directory.
///
/// The entry is looked at without following it first, so a missing one is
/// told from one that cannot be looked at, which is an error and not an
/// absence (AICD §14).
///
/// It is reached only after `way_enters_what_the_install_creates` found that
/// the way does not enter what the install creates. What it is given is the
/// text's reading of the way (`lexically_normal`), which is not the way itself
/// when a link comes before a `..`.
fn existing_directory(path: &Path) -> io::Result<Option<PathBuf>> {
    let absent = |error: &io::Error| {
        matches!(
            error.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        )
    };
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if absent(&error) => return Ok(None),
        Err(error) => return Err(error),
    }
    let directory = match fs::canonicalize(path) {
        Ok(directory) => directory,
        Err(error) if absent(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    match fs::metadata(&directory) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(directory)),
        Ok(_) => Ok(None),
        Err(error) if absent(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// One step of the way that `way_enters_what_the_install_creates` resolves.
enum Step {
    /// A prefix and root, or a root alone: the way restarts there.
    Root(PathBuf),
    /// `..`.
    Parent,
    /// A name.
    Name(std::ffi::OsString),
}

/// Puts the steps of `path` in front of `pending`, in order. `.` is dropped.
fn put_steps_in_front(pending: &mut VecDeque<Step>, path: &Path) {
    let mut steps: Vec<Step> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => steps.push(Step::Parent),
            Component::Normal(name) => steps.push(Step::Name(name.to_os_string())),
            Component::Prefix(_) | Component::RootDir => match steps.last_mut() {
                Some(Step::Root(root)) => root.push(component.as_os_str()),
                _ => steps.push(Step::Root(PathBuf::from(component.as_os_str()))),
            },
        }
    }
    for step in steps.into_iter().rev() {
        pending.push_front(step);
    }
}

/// Whether `name` is `expected`, optionally ignoring ASCII case.
fn is_named(name: &std::ffi::OsStr, expected: &str, fold_case: bool) -> bool {
    name.to_str().is_some_and(|text| {
        if fold_case {
            text.eq_ignore_ascii_case(expected)
        } else {
            text == expected
        }
    })
}

/// Whether the way `path` is one to refuse a link for: AICD §13.
///
/// `true` has two causes. The way, resolved one component at a time, first
/// fails at something this install creates; or it does not fail at all, so the
/// tree changed between the caller's look, which found a component missing, and
/// this one (a race). Refusing the install in that case fails closed, and
/// running it again decides afresh. `false` means the way first fails
/// somewhere else, or it meets a file; the caller then reads the way
/// physically and by its text (`follow_chain_to_managed_hook`, bullet (b)).
///
/// What the install creates, and which a link can run through before it exists,
/// is `ori-original/` under the hooks directory (`PRESERVED_DIR`) and an entry
/// of it named for a hook this installer replaces, which `preserve` makes for
/// each foreign hook. The first is looked for at the hooks directory, the
/// second in a `ori-original/` that is there under it.
///
/// The way is resolved physically, as the operating system resolves it now, and
/// not by its text. A work list of the components starts at the current
/// directory, or at the root a path begins with, canonicalized, so the
/// directory the walk stands in (`current`) is always an existing one that holds
/// no link, and its textual parent is its physical one. `.` is skipped. `..`
/// goes up from `current`, and stays at a root. A name is looked at without
/// following it:
///
/// - a link is read, and its target's components go to the front of the work
///   list, so a `..` after it leaves from where the link led and not from the
///   link; an absolute target restarts at its root, a relative one is read from
///   `current`, the link's own directory. More than `LINK_HOPS_CAP` links in one
///   call is an error, which refuses the install: a way that could not be
///   resolved is not a no (AICD §14);
/// - a directory becomes `current`;
/// - anything else is a file, and the way meets a non-directory and reaches
///   nothing: `false`;
/// - an entry that is not there is the first missing component, and the answer
///   is whether it is something the install creates: `ori-original` in the
///   hooks directory, or, in a directory called `ori-original` that is under the
///   hooks directory, a name of a hook this installer replaces. Anything else
///   is `false`;
/// - a name under a file is `false` as well; every other failure is returned.
///
/// If every component resolves, `true`: the caller found the way failing a
/// moment ago, so it changed under the check, and a way that cannot be told
/// from one that now reaches the engine's script is refused.
///
/// # Errors
///
/// Anything other than a missing entry, or a name under a file, is returned,
/// and the caller refuses the installation (AICD §14): a way that could not be
/// resolved is not a way that was found to reach nothing.
fn way_enters_what_the_install_creates(
    path: &Path,
    hooks_dir: &Path,
    canonical_hooks: &Path,
    fold_case: bool,
) -> io::Result<bool> {
    let mut pending: VecDeque<Step> = VecDeque::new();
    put_steps_in_front(&mut pending, path);
    let mut current = match pending.front() {
        Some(Step::Root(_)) => match pending.pop_front() {
            Some(Step::Root(root)) => fs::canonicalize(root)?,
            _ => return Ok(false),
        },
        _ => fs::canonicalize(std::env::current_dir()?)?,
    };
    let mut hops = 0usize;
    while let Some(step) = pending.pop_front() {
        match step {
            Step::Root(root) => current = fs::canonicalize(root)?,
            Step::Parent => {
                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                }
            }
            Step::Name(name) => {
                let candidate = current.join(&name);
                match fs::symlink_metadata(&candidate) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        hops += 1;
                        if hops > LINK_HOPS_CAP {
                            return Err(io::Error::other(format!(
                                "more than {LINK_HOPS_CAP} links in the way {}",
                                path.display()
                            )));
                        }
                        let target = fs::read_link(&candidate)?;
                        put_steps_in_front(&mut pending, &target);
                    }
                    Ok(metadata) if metadata.is_dir() => {
                        current = fs::canonicalize(&candidate)?;
                    }
                    Ok(_) => return Ok(false),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        let preserved_dir = is_named(&name, PRESERVED_DIR, fold_case)
                            && is_same_directory(&current, hooks_dir, canonical_hooks);
                        let preserved_entry = names_a_managed_hook(&name, fold_case)
                            && current
                                .file_name()
                                .is_some_and(|own| is_named(own, PRESERVED_DIR, fold_case))
                            && current.parent().is_some_and(|above| {
                                is_same_directory(above, hooks_dir, canonical_hooks)
                            });
                        return Ok(preserved_dir || preserved_entry);
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotADirectory => {
                        return Ok(false);
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }
    // Every component resolved, though the caller's `canonicalize` failed with
    // "not found" a moment ago: the tree changed between the two looks. The
    // readings that follow could misread such a way, so it is refused.
    Ok(true)
}

/// The first target of the link at `start`, as written, if following it and
/// every link after it reaches the place of a hook this installer replaces in
/// `hooks_dir`: AICD §13.
///
/// Each hop is resolved against the directory its own link is in, and the
/// directory part of the result is resolved by the operating system
/// (`canonicalize`), so `..`, a link to a directory, and a hooks directory
/// reached by another spelling all count, as do a chain through a link in the
/// hooks directory (`pre-push -> relay -> post-commit`) and one through a link
/// outside it. The last component is compared by name and is not required to
/// exist: a link to a hook that is not there yet would resolve to the
/// engine's script once it is. A link to a same-named hook in some other
/// directory (a team's shared hooks, say) does not count, and neither does a
/// chain longer than [`LINK_HOPS_CAP`], which no process could run either.
///
/// A directory that is not there cannot be resolved by the operating system,
/// and the way through it may run through what this install creates. So that
/// way is first resolved one component at a time
/// (`way_enters_what_the_install_creates`: each link is followed where it
/// stands, and each `..` is taken after the link before it), and what is done
/// with it depends on where it first fails:
///
/// - (a) A way that first fails at `ori-original/` of the hooks directory, or
///   at an entry of it named for a hook this installer replaces, is refused,
///   whatever it would reach (`ori-original/../post-merge`; `M/post-merge`
///   where `M` links to `ori-original/..`). `place` makes the directory, for
///   the first hook it keeps, and `preserve` an entry in it for each foreign
///   hook, as a hard link or as a link re-pointed one level down. What they
///   will make is not modelled: that would be a second implementation of
///   `preserve`, and a hook that is a link to a directory, which `preserve`
///   re-points, shows the two would differ. Refusing fails closed. It costs a
///   link whose way runs through a missing `ori-original/` and would reach
///   nothing, refused on every install while that directory is not there (a
///   repository with no foreign hook never gets one); a link through a missing
///   hook-named entry of it is refused on later installs too. No one writes
///   such a link, since the directory is Ori's and is not there yet.
/// - (b) A way that fails first at a directory that is not there, and not at
///   what the install creates, is read as git's process will find it if that
///   directory is later made as a real one (`walk_the_way`): links before the
///   missing component followed where they stand, names after it taken by
///   their text, and a `..` back to a directory that is there going on
///   physically. Its text (`lexically_normal`) is still read as a second look
///   at that hop, and refused if it lands on a hook's place (like any look, it
///   also refuses the install as an IO error when its landing cannot be looked
///   at): an over-refusal kept because it fails closed. Three ways show what
///   each reading does:
///   - `nowhere/../post-merge`: both readings land on the hooks directory;
///   - `hooks/L -> ../refs` with `L/remotes/../../hooks/post-merge`: only the
///     physical reading does, since the text lands on `hooks/hooks`. Git makes
///     `refs/remotes` at the first fetch, and from then on every checkout
///     would run the engine's script under the name `post-checkout`;
///   - `hooks/L -> ../refs` with `L/../nowhere/../post-merge`: only the text's
///     second look does, since the physical landing is `.git`.
///
///   What the readings land on is looked at: the walk goes on from the
///   physical landing when it is a directory that is there, and a link in it
///   is followed, since that is the way's real directory; else from the text's
///   landing, when that is a directory that is there (`hooks/L -> ../refs` with
///   `L/../sub/relay`, where `hooks/sub/relay` links to a hook: the physical
///   reading ends at `.git/sub`, which is not there, and the text lands on
///   `hooks/sub`), though that directory is where the text puts the way and
///   not necessarily where the way goes. Where neither is there, the chain
///   ends, as does a chain that ends at an entry that is not there (a
///   dangling link reaches nothing). A physical reading that
///   reads more than `LINK_HOPS_CAP` links is an error and not a no, and
///   refuses the install.
/// - (c) The spool's future path is read like any other missing directory, by
///   bullet (b)'s physical reading: `install` creates the spool, with any
///   missing ancestors, after this check, as real directories, which is what
///   that reading assumes. The spool is outside the repository
///   ([`HooksError::SpoolInsideTree`]) and its path is the engine's choice.
/// - (d) A link whose meaning depends on the process that follows it, on Linux
///   one through `/proc/self`, `/proc/thread-self` or `/dev/fd`, is resolved
///   as the installer's own process sees it, which is not what git's process
///   sees when it runs the hook. Such a link is refused before this walk, by
///   `way_enters_a_per_process_directory`
///   ([`HooksError::HookLinkDependsOnProcess`]). Git makes directories in `.git`
///   after the install and before it runs a hook (`logs/` at the first commit,
///   `rebase-merge/` during a rebase), so a way that does not exist now may
///   reach `/proc` then: past a missing component that walk follows the text of
///   the way until a `..` climbs back to a directory that is there, and walks on
///   physically from it. It assumes a directory made later is a real directory.
///   The spool's future path is one such directory. The working tree is judged
///   as it stands at the install, and a later checkout can make a link or a
///   file where a way passes, so that it reaches a hook it did not reach then.
///   The script's own check of the name it was run under keeps such a way from
///   recording (the module doc's section on a script not run by git for its own
///   event), and the next `install` or `status` refuses it. The
///   text reading of bullet (b) still resolves a `/proc/self` as the
///   installer's own process does, which is not git's; it does not decide a way
///   of this kind, because the per-process walk has refused it first. The
///   physical reading of bullet (b) does not resolve such a directory either: it
///   stops before it, and the way falls back to its text.
///
/// A link whose target cannot exist, because it goes through a regular file or
/// carries a name too long for the file system, is not a missing entry: it
/// refuses the installation as an IO error. That fails closed, and is
/// accepted. It holds where the operating system's own resolution meets the
/// regular file. Behind a missing directory, a way through a regular file is
/// read as reaching nothing, and the link passes, unless that way first fails
/// at what the install creates, rule (a), or the physical reading of rule (b)
/// reaches a hook, or its text lands on a hook's place. The physical reading
/// treats a file with steps after it as a directory that is not there. On
/// Windows, opening a path through a regular file reports it as not found, so
/// such a way is always read this way there.
///
/// # Errors
///
/// Anything other than a missing entry or directory is an error and not an
/// answer of "no": a link that cannot be read or an entry or directory that
/// cannot be looked at means the chain could not be followed, and so is more
/// than `LINK_HOPS_CAP` links in a way's physical reading (bullet (b)); the
/// caller refuses the installation (AICD §14).
fn follow_chain_to_managed_hook(
    hooks_dir: &Path,
    canonical_hooks: &Path,
    start: &Path,
    fold_case: bool,
) -> io::Result<Option<PathBuf>> {
    let written = fs::read_link(start)?;
    let mut link = start.to_path_buf();
    let mut target = written.clone();
    for _ in 0..LINK_HOPS_CAP {
        let joined = if target.is_absolute() {
            target.clone()
        } else {
            let Some(parent) = link.parent() else {
                return Ok(None);
            };
            parent.join(&target)
        };
        let Some(name) = joined.file_name().map(std::ffi::OsStr::to_os_string) else {
            return Ok(None);
        };
        let Some(parent) = joined.parent() else {
            return Ok(None);
        };
        let (directory, exists) = match fs::canonicalize(parent) {
            Ok(directory) => (directory, true),
            // A directory of the way is not there. If the way first fails at
            // what this install creates, the link is refused, whatever it
            // would reach. Otherwise the way is read physically
            // (`walk_the_way`), and its text is read as a second look that
            // refuses at a hook's place, or as an IO error when its landing
            // cannot be looked at. The walk goes on from the physical
            // landing when it is a directory that is there, else from the
            // text's.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if way_enters_what_the_install_creates(
                    parent,
                    hooks_dir,
                    canonical_hooks,
                    fold_case,
                )? {
                    return Ok(Some(written));
                }
                // The way as git's process will find it, if every directory made
                // after the install is a real one: links before the missing
                // component followed where they stand, names past it taken by
                // their text, and a `..` that climbs back to a directory that is
                // there going on physically.
                let physical = match walk_the_way(parent, fold_case)? {
                    WayEnd::Directory(directory) => Some(directory),
                    WayEnd::TooManyLinks => {
                        return Err(io::Error::other(format!(
                            "more than {LINK_HOPS_CAP} links in the way {}",
                            parent.display()
                        )));
                    }
                    WayEnd::PerProcess(_) | WayEnd::Elsewhere => None,
                };
                // The text's reading, kept as a second look that fails closed:
                // its landing is refused if it is a hook's place, or as an IO
                // error when its landing cannot be looked at.
                let lexical = lexically_normal(parent);
                let text = existing_directory(&lexical)?;
                if names_a_managed_hook(&name, fold_case)
                    && is_same_directory(
                        text.as_deref().unwrap_or(&lexical),
                        hooks_dir,
                        canonical_hooks,
                    )
                {
                    return Ok(Some(written));
                }
                match (physical, text) {
                    (Some(directory), _) | (None, Some(directory)) => (directory, true),
                    (None, None) => (lexical, false),
                }
            }
            Err(error) => return Err(error),
        };
        if names_a_managed_hook(&name, fold_case)
            && is_same_directory(&directory, hooks_dir, canonical_hooks)
        {
            return Ok(Some(written));
        }
        if !exists {
            return Ok(None);
        }
        // Not the place of a hook we replace: keep going if it is a link.
        let next = directory.join(&name);
        match fs::symlink_metadata(&next) {
            Ok(metadata) if metadata.file_type().is_symlink() => {}
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
        target = fs::read_link(&next)?;
        link = next;
    }
    Ok(None)
}

/// Whether `directory` is, or is under, a directory where a path names
/// something different for each process that follows it: AICD §13.
///
/// Those are `/proc` and everything under it (procfs, with `/proc/self`,
/// `/proc/thread-self` and `/proc/<pid>`), `/dev/fd` and everything under it
/// (fdescfs on macOS and the BSDs), and `/compat/linux/proc` and everything
/// under it (FreeBSD's linprocfs, which its Linux emulation mounts there). On
/// Linux `/dev/fd`, `/dev/stdin`, `/dev/stdout` and `/dev/stderr` are links into
/// `/proc/self`; they are followed there, so they count through `/proc`.
///
/// It is the text of `directory` alone, and no file is looked at: the
/// components must begin with the root and then `proc`; or the root, `dev`,
/// `fd`; or the root, `compat`, `linux`, `proc`. A relative path and a path
/// with a Windows prefix never match, and neither does `/procfs`, `/dev`,
/// `/dev/null`, `/compat/linux` or `/home/proc`. It is `false` everywhere but on
/// Unix, which is the only family with such a directory.
fn is_per_process_directory(directory: &Path, fold_case: bool) -> bool {
    if !cfg!(unix) {
        return false;
    }
    let mut components = directory.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return false;
    }
    let mut names = components.map_while(|component| match component {
        Component::Normal(name) => Some(name),
        _ => None,
    });
    match names.next() {
        Some(first) if is_named(first, "proc", fold_case) => true,
        Some(first) if is_named(first, "dev", fold_case) => names
            .next()
            .is_some_and(|second| is_named(second, "fd", fold_case)),
        Some(first) if is_named(first, "compat", fold_case) => {
            names
                .next()
                .is_some_and(|second| is_named(second, "linux", fold_case))
                && names
                    .next()
                    .is_some_and(|third| is_named(third, "proc", fold_case))
        }
        _ => false,
    }
}

/// Where `walk_the_way` stopped.
enum WayEnd {
    /// It entered this per-process directory.
    PerProcess(PathBuf),
    /// Its steps ran out in this directory, which is there; the path is canonical.
    Directory(PathBuf),
    /// Its steps ran out past a component that is not there, or at a file.
    Elsewhere,
    /// It read more than `LINK_HOPS_CAP` links.
    TooManyLinks,
}

/// Walks the way `path` one component at a time, physically while every
/// directory is there and by its text past the first that is not, and says
/// where it stopped (`WayEnd`): AICD §13.
///
/// It walks as `way_enters_what_the_install_creates` does, with the same
/// steps, and starts at the canonical root `path` begins with, or at the
/// canonical current directory. After every change of the directory the walk
/// stands in (the start, a root, a `..`, a name entered, whether it is there
/// or not) it asks `is_per_process_directory`, and on a yes stops with that
/// directory. So the walk stops on entering such a directory, before it reads
/// any link inside it: `/proc/self` and `/proc/self/cwd` are never read or
/// followed, because what they lead to is what this process is, and not what
/// git's process would find. A directory is asked about before it is
/// canonicalized, so the platform's `realpath` is never called on a per-process
/// file system.
///
/// While every directory the walk has entered is there, a name is looked at
/// without following it:
///
/// - a link is read, and its target's steps go to the front of the work list,
///   so one call follows a whole chain, through links inside the hooks
///   directory and outside it;
/// - a directory becomes the directory the walk stands in;
/// - a name that is not there, and a file that has more steps after it, are
///   where the walk leaves the file system (below); a file at the last step ends
///   the way, at `WayEnd::Elsewhere`.
///
/// **Past a component that is not there.** Git makes directories after the
/// install and before it runs a hook: `logs/` at the first commit, and
/// `rebase-merge/` during every rebase, both in `.git`. A way that does not
/// exist now can therefore reach `/proc` then (`../logs/` followed by enough
/// `../` and `proc/self/cwd/...`), and the walk does not stop at it. It keeps
/// the directory it stands in as a path, with a count of the trailing names of
/// it that are not there. A name met while that count is above 0 is added to
/// the path, and the count grows; nothing is looked at, because its parent is
/// not there, and so no link is read. A `..` with the count above 0 removes
/// the last name by its text and lowers the count, since a directory git makes
/// is a real directory and `..` from it returns where it was; at 0 it is the
/// physical parent. A root restarts physically. A file that has steps after it
/// is treated like a missing directory: it could be replaced by one. The walk
/// assumes that a directory made later is a real directory: that is the limit
/// of this rule. The spool's future path is one such directory. The working
/// tree is judged as it stands at the install, and a later checkout can put a
/// link or a file where a way passes, and make it reach a hook it did not
/// reach then. The script's own check of the name it was run under keeps such
/// a way from recording (the module doc's section on a script not run by git
/// for its own event), and the next `install` or `status` refuses it.
///
/// The walk ends in one of four ways. It stops at `WayEnd::PerProcess` on
/// entering a per-process directory. Its steps run out at `WayEnd::Directory`
/// in a directory that is there, and the path is canonical. That holds after a
/// climb back out of a missing component too: a `..` taken while the count is
/// above 0 removes a name taken by its text, and when the count is back to 0
/// the walk stands in the canonical directory it left. They run out at
/// `WayEnd::Elsewhere` past a component that is not there (the count above is
/// not 0), or at a file at the last step. More than `LINK_HOPS_CAP` links read
/// end it at `WayEnd::TooManyLinks`. That is not an error here, because no
/// process can follow such a way, and each caller says what it makes of it.
/// The cap counts every link read and none is read past a missing component,
/// so the walk always ends (`L -> missing/../L` reaches the cap).
///
/// # Errors
///
/// Anything other than a missing entry or a name under a file is returned, and
/// the caller refuses the installation (AICD §14): a way that could not be
/// looked at is not a way that was found to enter nothing.
fn walk_the_way(path: &Path, fold_case: bool) -> io::Result<WayEnd> {
    let mut pending: VecDeque<Step> = VecDeque::new();
    put_steps_in_front(&mut pending, path);
    let mut current = match pending.front() {
        Some(Step::Root(_)) => match pending.pop_front() {
            Some(Step::Root(root)) => fs::canonicalize(root)?,
            _ => return Ok(WayEnd::Elsewhere),
        },
        _ => fs::canonicalize(std::env::current_dir()?)?,
    };
    if is_per_process_directory(&current, fold_case) {
        return Ok(WayEnd::PerProcess(current));
    }
    // The trailing names of `current` that are not there. It is 0 while the
    // walk is physical, and `current` is then canonical.
    let mut missing_depth = 0usize;
    let mut hops = 0usize;
    while let Some(step) = pending.pop_front() {
        match step {
            Step::Root(root) => {
                current = fs::canonicalize(root)?;
                missing_depth = 0;
            }
            Step::Parent => {
                if missing_depth > 0 {
                    current.pop();
                    missing_depth -= 1;
                } else if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                }
            }
            Step::Name(name) if missing_depth > 0 => {
                current.push(&name);
                missing_depth += 1;
            }
            Step::Name(name) => {
                let candidate = current.join(&name);
                match fs::symlink_metadata(&candidate) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        hops += 1;
                        if hops > LINK_HOPS_CAP {
                            return Ok(WayEnd::TooManyLinks);
                        }
                        let target = fs::read_link(&candidate)?;
                        put_steps_in_front(&mut pending, &target);
                        // The directory the walk stands in did not change.
                        continue;
                    }
                    Ok(metadata) if metadata.is_dir() => {
                        // Before `canonicalize`, which is never called on a
                        // per-process file system. `current` is canonical and
                        // the entry is a real directory, so `candidate` is
                        // already the path, up to case, which the question
                        // folds.
                        if is_per_process_directory(&candidate, fold_case) {
                            return Ok(WayEnd::PerProcess(candidate));
                        }
                        current = fs::canonicalize(&candidate)?;
                    }
                    // A file at the last step ends the way; with steps after
                    // it, it is read like a directory that is not there.
                    Ok(_) if pending.is_empty() => return Ok(WayEnd::Elsewhere),
                    Ok(_) => {
                        current = candidate;
                        missing_depth = 1;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                        ) =>
                    {
                        current = candidate;
                        missing_depth = 1;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        if is_per_process_directory(&current, fold_case) {
            return Ok(WayEnd::PerProcess(current));
        }
    }
    if missing_depth == 0 {
        Ok(WayEnd::Directory(current))
    } else {
        Ok(WayEnd::Elsewhere)
    }
}

/// The per-process directory the way `path` enters, if it enters one: AICD §13.
///
/// The walk is `walk_the_way`, which the chain check's physical reading shares;
/// its doc says how the way is followed, and why the walk stops on entering
/// such a directory before it reads any link inside it. The answer is `Some`
/// only where the walk stopped in one. A way whose steps run out, or that
/// meets a file at its last step, enters none, and neither does a way that
/// reads more than `LINK_HOPS_CAP` links. That is not an error: no process can
/// follow such a way. `follow_chain_to_managed_hook` reads a chain of more than
/// `LINK_HOPS_CAP` final hops as no, and a loop it meets while resolving a
/// directory as an IO error, which refuses the install. This question reads
/// both as no, because the chain check that follows refuses the second.
///
/// What it does not recognise:
///
/// - a procfs or fdescfs mounted anywhere else, or bind-mounted, is not
///   recognised. An ordinary user can make one: FUSE (`fusermount` is setuid)
///   and unprivileged user namespaces let a user mount a per-process file
///   system, or bind-mount `/proc`, in directories of their own;
/// - Windows has no such directory, and a drive letter that differs for each
///   logon session (`subst`, a mapped drive) in an absolute target is not
///   judged;
/// - the variant links of DragonFly BSD are not judged;
/// - git run in another mount namespace is outside the model.
///
/// # Errors
///
/// Anything other than a missing entry or a name under a file is returned, and
/// the caller refuses the installation (AICD §14): a way that could not be
/// looked at is not a way that was found to enter nothing.
fn way_enters_a_per_process_directory(path: &Path, fold_case: bool) -> io::Result<Option<PathBuf>> {
    Ok(match walk_the_way(path, fold_case)? {
        WayEnd::PerProcess(directory) => Some(directory),
        WayEnd::Directory(_) | WayEnd::Elsewhere | WayEnd::TooManyLinks => None,
    })
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

/// Where the hook that was in `hooks_dir` before the engine's is kept: under
/// its own file name, so a hook that reads `$0` to learn which hook it is
/// still learns it.
fn preserved_path(hooks_dir: &Path, hook: GitHook) -> PathBuf {
    hooks_dir.join(PRESERVED_DIR).join(hook.file_name())
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
# ori-studio-hook v3: written by Ori Studio (ori-watch). The engine rewrites this file.
# Reports this git event to the engine by appending one line to the spool below,
# then runs the hook that was here before Ori Studio, if any: it is kept as
# ori-original/<this file's name> beside this file, so it runs under its own name.
# If the spool cannot be written this prints an error on standard error.
ori_hook=@HOOK@
ori_spool=@SPOOL@
ori_log=\"$ori_spool/hook-events.log\"
ori_original=\"$(dirname -- \"$0\")/ori-original/$ori_hook\"

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

# Git runs a hook as <hooks directory>/<its event's name>. Run under another
# name, or from ori-original/, this script was reached through a link or run by
# another script, not by git for its own event: it records nothing, runs
# nothing, and lets git go on. It reads its standard input first, so a writer
# that feeds this event is never cut off.
ori_run_as=$(basename -- \"$0\")
ori_run_from=$(basename -- \"$(dirname -- \"$0\")\")
if [ \"$ori_run_as\" != \"$ori_hook\" ] || [ \"$ori_run_from\" = 'ori-original' ]; then
    printf 'ori-studio: the %s hook was run as %s in %s, not by git for its own event; it recorded nothing and ran nothing.\\n' \"$ori_hook\" \"$(ori_clean \"$ori_run_as\")\" \"$(ori_clean \"$(dirname -- \"$0\")\")\" >&2
    [ -t 0 ] || cat >/dev/null 2>&1
    exit 0
fi

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
///
/// The log is opened without following a link and without waiting on a pipe,
/// and the opened handle (not the path) is what is checked and read. A log
/// that was replaced is detected by its identity (device and inode) on Unix as
/// well as by its length; where the platform offers no identity (Windows), by
/// length alone, so a replacement at least as long as the offset is read from
/// the offset. The identity is remembered by the reader, not by the saved
/// offset: a reader reopened with [`HookSpool::open_at`] adopts the log it
/// finds, so a log replaced while the engine was down is read from the saved
/// offset when it is long enough.
#[derive(Debug)]
pub struct HookSpool {
    log: PathBuf,
    offset: u64,
    identity: Option<(u64, u64)>,
}

/// The device and inode of a file, where the platform has them.
#[cfg(unix)]
fn identity_of(metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn identity_of(_metadata: &fs::Metadata) -> Option<(u64, u64)> {
    None
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
            identity: None,
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
        let (mut file, metadata) = match open_plain_file(&self.log) {
            Ok(opened) => opened,
            Err(PlainReadError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SpoolDrain::default());
            }
            Err(PlainReadError::Io(error)) => return Err(io_error(error)),
            Err(PlainReadError::Symlink | PlainReadError::NotARegularFile) => {
                return Err(HooksError::SpoolLogNotAFile {
                    path: self.log.clone(),
                });
            }
        };
        let mut drain = SpoolDrain::default();
        let identity = identity_of(&metadata);
        let replaced = self.identity.is_some() && identity != self.identity;
        if metadata.len() < self.offset || replaced {
            self.offset = 0;
            drain.restarted = true;
        }
        self.identity = identity;
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

    /// `follow_chain_to_managed_hook` read as a yes or a no: the shape the
    /// first tests of the chain check were written against. An error fails
    /// the test and is not read as a no, so a walk that could not be made
    /// cannot pass for one that found nothing. It exists on Unix only, where
    /// its test does: elsewhere it would be dead code.
    #[cfg(unix)]
    fn chain_reaches_managed_hook(
        hooks_dir: &Path,
        canonical_hooks: &Path,
        start: &Path,
        fold_case: bool,
    ) -> Option<PathBuf> {
        follow_chain_to_managed_hook(hooks_dir, canonical_hooks, start, fold_case)
            .expect("the walk does not fail here")
    }

    /// Where the installer keeps the hook that was in `hook`'s place before.
    fn preserved(repo: &Path, hook: GitHook) -> PathBuf {
        preserved_path(&repo.join(".git").join("hooks"), hook)
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
        let preserved = preserved(&repo, GitHook::PostCommit);
        assert_eq!(preserved.file_name().unwrap(), "post-commit");
        assert_eq!(
            preserved.parent().unwrap().file_name().unwrap(),
            "ori-original"
        );
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
        let older = preserved(&repo, GitHook::PrePush);
        fs::create_dir_all(older.parent().unwrap()).unwrap();
        fs::write(&older, "#!/bin/sh\necho older\n").unwrap();
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
        assert!(!preserved(&repo, GitHook::PostCommit).exists());
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
    fn ori_t_0054_a_hook_that_is_a_symlink_is_someone_elses_and_is_kept_not_written_through() {
        let scratch = Scratch::new("hooklink");
        let (repo, spool) = skeleton(&scratch);
        // A relative link, as a team's shared hooks directory is usually
        // wired: it is read from the link's own directory, which the
        // preserved copy sits one level below.
        let target = repo.join(".git").join("shared-hook.sh");
        fs::write(&target, "#!/bin/sh\necho shared\n").unwrap();
        let path = hook_path(&repo, GitHook::PostCommit);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("../shared-hook.sh", &path).unwrap();
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "#!/bin/sh\necho shared\n",
            "the link's target was written through"
        );
        let kept = preserved(&repo, GitHook::PostCommit);
        assert!(
            fs::symlink_metadata(&kept)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&kept).unwrap(),
            "#!/bin/sh\necho shared\n",
            "the preserved link no longer reaches the hook it stood for"
        );
        assert!(is_ours(&fs::read_to_string(&path).unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_hook_that_is_an_absolute_symlink_is_kept_with_its_target_unchanged() {
        let scratch = Scratch::new("hookabslink");
        let (repo, spool) = skeleton(&scratch);
        // An absolute target outside the repository: read from anywhere, so
        // the preserved link must carry it exactly as written.
        let target = scratch.path.join("team-hooks.sh");
        fs::write(&target, "#!/bin/sh\necho team\n").unwrap();
        let path = hook_path(&repo, GitHook::PrePush);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();

        let kept = preserved(&repo, GitHook::PrePush);
        assert!(
            fs::symlink_metadata(&kept)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_link(&kept).unwrap(),
            target,
            "an absolute target was rewritten"
        );
        assert_eq!(fs::read_to_string(&kept).unwrap(), "#!/bin/sh\necho team\n");
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "#!/bin/sh\necho team\n",
            "the link's target was written through"
        );
        assert!(is_ours(&fs::read_to_string(&path).unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_hook_that_links_to_another_hook_the_installer_replaces_is_refused() {
        let scratch = Scratch::new("hooksibling");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        // `pre-push` is `post-commit` by another name: the user's one script
        // serves both. Kept as a link it would resolve to the engine's
        // script, and the veto in it would stop vetoing.
        let theirs = "#!/bin/sh\necho theirs\nexit 1\n";
        fs::write(hooks_dir.join("post-commit"), theirs).unwrap();
        make_executable(&hooks_dir.join("post-commit")).unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();

        for target in [
            PathBuf::from("post-commit"),
            PathBuf::from("./post-commit"),
            PathBuf::from("../hooks/post-commit"),
            hooks_dir.join("post-commit"),
        ] {
            let link = hooks_dir.join("pre-push");
            let _ = fs::remove_file(&link);
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let error = hooks.install().unwrap_err();
            match &error {
                HooksError::HookLinksToManagedHook { path, target: seen } => {
                    assert_eq!(path, &link);
                    assert_eq!(seen, &target);
                }
                other => panic!("{target:?}: expected HookLinksToManagedHook, got {other}"),
            }
            assert_eq!(error.methodology_ref().unwrap().section, 13);
            // Nothing was written, for any hook.
            assert_eq!(
                fs::read_to_string(hooks_dir.join("post-commit")).unwrap(),
                theirs
            );
            assert_eq!(fs::read_link(&link).unwrap(), target);
            assert_eq!(hook_dir_names(&repo), vec!["post-commit", "pre-push"]);
            assert!(
                !spool.exists(),
                "the spool was created by a refused install"
            );
        }

        // A link to a same-named hook in another directory is the usual
        // shared-hooks arrangement, and is kept.
        let shared = repo.join(".git").join("shared");
        fs::create_dir_all(&shared).unwrap();
        fs::write(shared.join("pre-push"), "#!/bin/sh\necho shared\n").unwrap();
        let link = hooks_dir.join("pre-push");
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("../shared/pre-push", &link).unwrap();
        fs::remove_file(hooks_dir.join("post-commit")).unwrap();
        hooks.install().unwrap();
        assert_eq!(
            fs::read_to_string(preserved(&repo, GitHook::PrePush)).unwrap(),
            "#!/bin/sh\necho shared\n"
        );
    }

    /// A repository whose hooks directory holds the user's `post-commit`, a
    /// script that vetoes, and nothing else: the arrangement the link tests
    /// below build their links on.
    #[cfg(unix)]
    fn repository_with_a_users_post_commit(
        scratch: &Scratch,
    ) -> (PathBuf, PathBuf, PathBuf, &'static str) {
        let (repo, spool) = skeleton(scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let theirs = "#!/bin/sh\necho theirs\nexit 1\n";
        fs::write(hooks_dir.join("post-commit"), theirs).unwrap();
        make_executable(&hooks_dir.join("post-commit")).unwrap();
        (repo, spool, hooks_dir, theirs)
    }

    /// Installs and expects the refusal for a link, and that it wrote nothing.
    #[cfg(unix)]
    fn expect_refused_link(
        repo: &Path,
        spool: &Path,
        expected_path: &Path,
        expected_target: &Path,
        names_before: &[&str],
    ) {
        let error = GitHooks::new(repo, spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::HookLinksToManagedHook { path, target } => {
                assert_eq!(path, expected_path);
                assert_eq!(target, expected_target);
            }
            other => panic!("expected HookLinksToManagedHook, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        assert_eq!(
            hook_dir_names(repo),
            names_before,
            "a refused installation wrote into the hooks directory"
        );
        assert!(
            !spool.exists(),
            "the spool was created by a refused install"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_chain_of_links_that_ends_at_a_hook_the_installer_replaces_is_refused() {
        // `pre-push -> relay -> post-commit`: kept, `pre-push` would still end
        // at `post-commit`, which is the engine's script by then, and the
        // user's pre-push veto would stop vetoing.
        let scratch = Scratch::new("chain-inside");
        let (repo, spool, hooks_dir, theirs) = repository_with_a_users_post_commit(&scratch);
        std::os::unix::fs::symlink("relay", hooks_dir.join("pre-push")).unwrap();
        std::os::unix::fs::symlink("post-commit", hooks_dir.join("relay")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("pre-push"),
            Path::new("relay"),
            &["post-commit", "pre-push", "relay"],
        );
        assert_eq!(
            fs::read_to_string(hooks_dir.join("post-commit")).unwrap(),
            theirs
        );

        // The relay is not itself one of the four hooks, and is refused on
        // its own when nothing else points at it: it ends at a managed hook.
        let scratch = Scratch::new("chain-relay-alone");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        std::os::unix::fs::symlink("post-commit", hooks_dir.join("relay")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("relay"),
            Path::new("post-commit"),
            &["post-commit", "relay"],
        );

        // A chain through a link outside the hooks directory, spelled with
        // `..` at both hops.
        let scratch = Scratch::new("chain-outside");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let shared = repo.join(".git").join("shared");
        fs::create_dir_all(&shared).unwrap();
        std::os::unix::fs::symlink("../hooks/post-commit", shared.join("pre-push")).unwrap();
        std::os::unix::fs::symlink("../shared/pre-push", hooks_dir.join("pre-push")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("pre-push"),
            Path::new("../shared/pre-push"),
            &["post-commit", "pre-push"],
        );

        // Three hops, the first absolute, ending at a hook that is not there
        // yet and so would be created as the engine's. The links are scanned
        // in name order, so the intermediates sort after `pre-push`: the
        // refusal is for the three-hop link, not for a two-hop one behind it.
        let scratch = Scratch::new("chain-long");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        fs::remove_file(hooks_dir.join("post-commit")).unwrap();
        std::os::unix::fs::symlink(hooks_dir.join("x-a"), hooks_dir.join("pre-push")).unwrap();
        std::os::unix::fs::symlink("x-b", hooks_dir.join("x-a")).unwrap();
        std::os::unix::fs::symlink("./post-merge", hooks_dir.join("x-b")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("pre-push"),
            &hooks_dir.join("x-a"),
            &["pre-push", "x-a", "x-b"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_hook_that_is_not_ours_linked_to_one_the_installer_replaces_is_refused() {
        // `post-checkout -> post-merge`: `post-checkout` is not ours to move,
        // and `post-merge` becomes the engine's script, so every checkout
        // would record a `post-merge` event that never happened.
        let scratch = Scratch::new("unmanaged");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let theirs = "#!/bin/sh\necho theirs\n";
        fs::write(hooks_dir.join("post-merge"), theirs).unwrap();
        make_executable(&hooks_dir.join("post-merge")).unwrap();
        std::os::unix::fs::symlink("post-merge", hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("post-merge"),
            &["post-checkout", "post-merge"],
        );
        assert_eq!(
            fs::read_to_string(hooks_dir.join("post-merge")).unwrap(),
            theirs
        );

        // The same when the managed hook is not there yet: once installed it
        // is, and the link would reach it.
        fs::remove_file(hooks_dir.join("post-merge")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("post-merge"),
            &["post-checkout"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_links_that_do_not_reach_a_hook_the_installer_replaces_are_left_alone() {
        let scratch = Scratch::new("links-kept");
        let (repo, spool, hooks_dir, theirs) = repository_with_a_users_post_commit(&scratch);
        // A link to a script of the user's own in the hooks directory, one to
        // a same-named hook in another directory, a link out of the
        // repository, a dangling one, and a loop that touches no hook.
        fs::write(hooks_dir.join("checkout-script.sh"), "#!/bin/sh\necho c\n").unwrap();
        std::os::unix::fs::symlink("checkout-script.sh", hooks_dir.join("post-checkout")).unwrap();
        let shared = repo.join(".git").join("shared");
        fs::create_dir_all(&shared).unwrap();
        fs::write(shared.join("post-commit"), "#!/bin/sh\necho shared\n").unwrap();
        std::os::unix::fs::symlink("../shared/post-commit", hooks_dir.join("pre-commit")).unwrap();
        std::os::unix::fs::symlink(scratch.path.join("elsewhere"), hooks_dir.join("update"))
            .unwrap();
        std::os::unix::fs::symlink("loop-b", hooks_dir.join("loop-a")).unwrap();
        std::os::unix::fs::symlink("loop-a", hooks_dir.join("loop-b")).unwrap();

        let report = GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert_eq!(report.outcomes.len(), 4);
        // Their own post-commit is kept, chained, under its own name.
        assert_eq!(
            fs::read_to_string(preserved(&repo, GitHook::PostCommit)).unwrap(),
            theirs
        );
        // And what was linked is as it was.
        assert_eq!(
            fs::read_link(hooks_dir.join("post-checkout")).unwrap(),
            Path::new("checkout-script.sh")
        );
        assert_eq!(
            fs::read_link(hooks_dir.join("pre-commit")).unwrap(),
            Path::new("../shared/post-commit")
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_link_through_a_directory_the_installer_creates_later_is_refused() {
        // `ori-original/` is where the installer keeps a hook it moves, and it
        // is not there yet: the operating system cannot resolve `..` through
        // it, so the directory is read by its text, which is the hooks
        // directory itself. Kept, `pre-push` would end at the engine's
        // `post-merge`.
        let scratch = Scratch::new("chain-lexical");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("ori-original/../post-merge", hooks_dir.join("pre-push"))
            .unwrap();
        assert!(!hooks_dir.join("ori-original").exists());
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("pre-push"),
            Path::new("ori-original/../post-merge"),
            &["pre-push"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_link_through_a_missing_directory_onto_an_existing_one_is_followed() {
        // `ori-original/` is not there yet, but the user's `post-commit` is
        // kept, so the install creates it, and `ori-original/../../shared/relay`
        // is then `.git/shared/relay`. The operating system cannot resolve the
        // path today, and its text lands on `.git/shared`, a directory that is
        // there: the walk goes on from it and meets `relay`, which links to a
        // hook the installer replaces.
        let scratch = Scratch::new("chain-lexical-onto-existing");
        let (repo, spool, hooks_dir, theirs) = repository_with_a_users_post_commit(&scratch);
        let shared = repo.join(".git").join("shared");
        fs::create_dir_all(&shared).unwrap();
        std::os::unix::fs::symlink("../hooks/post-merge", shared.join("relay")).unwrap();
        std::os::unix::fs::symlink(
            "ori-original/../../shared/relay",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        assert!(!hooks_dir.join("ori-original").exists());
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("ori-original/../../shared/relay"),
            &["post-checkout", "post-commit"],
        );
        assert_eq!(
            fs::read_to_string(hooks_dir.join("post-commit")).unwrap(),
            theirs
        );

        // The same when the reading lands on a link to a directory, which is
        // a directory that is there: it is followed through the link.
        let scratch = Scratch::new("chain-lexical-onto-a-linked-directory");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let git_dir = repo.join(".git");
        let shared = git_dir.join("shared");
        fs::create_dir_all(&shared).unwrap();
        std::os::unix::fs::symlink("../hooks/post-merge", shared.join("relay")).unwrap();
        std::os::unix::fs::symlink("shared", git_dir.join("shared-link")).unwrap();
        std::os::unix::fs::symlink(
            "ori-original/../../shared-link/relay",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("ori-original/../../shared-link/relay"),
            &["post-checkout", "post-commit"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_dangling_link_to_a_missing_directory_is_not_refused() {
        // `pre-push -> nowhere/post-merge`: `nowhere/` is not there and is not
        // the hooks directory, so the link reaches nothing the installer
        // replaces. As for any dangling link, the install proceeds: the
        // link is a hook of someone else's, kept under `ori-original/` with
        // its target adjusted for the one level, and `pre-push` is the
        // engine's script.
        let scratch = Scratch::new("chain-dangling-directory");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("nowhere/post-merge", hooks_dir.join("pre-push")).unwrap();

        let report = GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert_eq!(report.outcomes.len(), 4);
        assert!(
            report.outcomes.iter().any(|(hook, outcome)| {
                *hook == GitHook::PrePush
                    && *outcome
                        == InstallOutcome::InstalledPreserving {
                            preserved: preserved(&repo, GitHook::PrePush),
                        }
            }),
            "{report:?}"
        );
        assert_eq!(
            fs::read_link(preserved(&repo, GitHook::PrePush)).unwrap(),
            Path::new("../nowhere/post-merge")
        );
        assert!(
            fs::read_to_string(hooks_dir.join("pre-push"))
                .unwrap()
                .contains(MARKER),
            "pre-push is not the engine's script"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_link_that_cannot_be_followed_refuses_the_install_as_an_io_error() {
        // A directory part that is a loop of links cannot be resolved, and the
        // reason is not that it is missing: the check could not be made, so
        // the install is refused and not passed (AICD §14).
        let scratch = Scratch::new("chain-unresolvable");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("loop-b", hooks_dir.join("loop-a")).unwrap();
        std::os::unix::fs::symlink("loop-a", hooks_dir.join("loop-b")).unwrap();
        std::os::unix::fs::symlink("loop-a/post-merge", hooks_dir.join("pre-push")).unwrap();

        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::Io { path, .. } => assert_eq!(path, &hooks_dir.join("pre-push")),
            other => panic!("expected an IO error naming the link, got {other}"),
        }
        assert_eq!(
            hook_dir_names(&repo),
            ["loop-a", "loop-b", "pre-push"],
            "a refused installation wrote into the hooks directory"
        );
        assert!(
            !spool.exists(),
            "the spool was created by a refused install"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_link_that_cannot_be_read_is_an_error_and_not_a_no() {
        let scratch = Scratch::new("chain-unreadable-start");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        // Not a link: reading it as one fails, and the failure is the answer.
        let plain = hooks_dir.join("plain");
        fs::write(&plain, "#!/bin/sh\n").unwrap();
        assert!(follow_chain_to_managed_hook(&hooks_dir, &canonical, &plain, false).is_err());
        // Not there at all.
        let missing = hooks_dir.join("missing");
        assert!(follow_chain_to_managed_hook(&hooks_dir, &canonical, &missing, false).is_err());
    }

    #[test]
    fn ori_t_0159_a_path_is_read_by_its_text_without_rising_above_the_root() {
        for (written, expected) in [
            ("/a/b/./c", "/a/b/c"),
            ("/a/b/../c", "/a/c"),
            ("/a/ori-original/..", "/a"),
            ("/a/../../b", "/b"),
            ("/..", "/"),
            ("a/b/../..", ""),
            ("../a", "../a"),
            ("a/../../b", "../b"),
        ] {
            assert_eq!(
                lexically_normal(Path::new(written)),
                PathBuf::from(expected),
                "{written}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0159_a_link_to_a_name_that_differs_only_in_case_is_refused_where_names_fold_case() {
        let scratch = Scratch::new("case");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let link = hooks_dir.join("post-checkout");
        std::os::unix::fs::symlink("Post-Commit", &link).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();

        // Both readings, whatever this platform's file system does: the
        // comparison is a parameter so each is tested everywhere.
        assert_eq!(
            chain_reaches_managed_hook(&hooks_dir, &canonical, &link, true),
            Some(PathBuf::from("Post-Commit")),
            "a case-folding platform did not see the link reach post-commit"
        );
        assert_eq!(
            chain_reaches_managed_hook(&hooks_dir, &canonical, &link, false),
            None,
            "an exact-name platform refused a link to a name it does not own"
        );
        assert!(names_a_managed_hook(std::ffi::OsStr::new("PRE-PUSH"), true));
        assert!(!names_a_managed_hook(
            std::ffi::OsStr::new("PRE-PUSH"),
            false
        ));
        assert!(!names_a_managed_hook(
            std::ffi::OsStr::new("pre-push.sample"),
            true
        ));

        // And what the installer does on the platform it runs on: macOS and
        // Windows fold, the rest do not.
        if CASE_INSENSITIVE_NAMES {
            expect_refused_link(
                &repo,
                &spool,
                &link,
                Path::new("Post-Commit"),
                &["post-checkout", "post-commit"],
            );
        } else {
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            assert_eq!(
                fs::read_link(&link).unwrap(),
                Path::new("Post-Commit"),
                "a link to a name this file system does not fold was moved"
            );
        }
    }

    /// Once `ori-original/` exists and `post-merge` is the engine's script,
    /// `link` resolves to that `post-merge`: the premise of the tests below,
    /// which a layout that did not reach it would make vacuous. Call it after
    /// the refusal has been checked, since it makes both.
    #[cfg(unix)]
    fn assert_link_reaches_post_merge_once_ori_original_exists(hooks_dir: &Path, link: &Path) {
        fs::create_dir(hooks_dir.join("ori-original")).unwrap();
        fs::write(hooks_dir.join("post-merge"), "#!/bin/sh\n").unwrap();
        assert_eq!(
            fs::canonicalize(link).unwrap(),
            fs::canonicalize(hooks_dir).unwrap().join("post-merge"),
            "the layout does not reach the engine's post-merge, so the test proves nothing"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_through_a_dangling_link_into_ori_original_is_refused() {
        // H2: `M -> ori-original/..` goes nowhere today, and is the hooks
        // directory once the install has made `ori-original/`, so
        // `post-checkout -> M/post-merge` would run the engine's script.
        let scratch = Scratch::new("way-dangling-link");
        let (repo, spool, hooks_dir, theirs) = repository_with_a_users_post_commit(&scratch);
        std::os::unix::fs::symlink("ori-original/..", hooks_dir.join("M")).unwrap();
        std::os::unix::fs::symlink("M/post-merge", hooks_dir.join("post-checkout")).unwrap();
        assert!(!hooks_dir.join("ori-original").exists());
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("M/post-merge"),
            &["M", "post-checkout", "post-commit"],
        );
        assert_eq!(
            fs::read_to_string(hooks_dir.join("post-commit")).unwrap(),
            theirs
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_dangling_link_into_ori_original_from_outside_the_hooks_directory_is_refused() {
        // H2b: the link that goes nowhere is `.git/D`, outside `hooks/`.
        let scratch = Scratch::new("way-dangling-link-outside");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        std::os::unix::fs::symlink("hooks/ori-original/..", repo.join(".git").join("D")).unwrap();
        std::os::unix::fs::symlink("../D/post-merge", hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("../D/post-merge"),
            &["post-checkout", "post-commit"],
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_dangling_link_with_an_absolute_target_into_ori_original_is_refused() {
        // H2 with the link in the way written as an absolute path: the way
        // restarts at the root and comes down to `ori-original/` again.
        let scratch = Scratch::new("way-dangling-absolute-link");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        std::os::unix::fs::symlink(
            hooks_dir.join("ori-original").join(".."),
            hooks_dir.join("M"),
        )
        .unwrap();
        std::os::unix::fs::symlink("M/post-merge", hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("M/post-merge"),
            &["M", "post-checkout", "post-commit"],
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_whose_way_goes_up_through_a_linked_directory_is_refused() {
        // H3: the way is `ori-original/../L/../../post-merge`, and once
        // `ori-original/` exists `L/..` leaves from `x/y`, the directory `L`
        // links to, so the way ends at `hooks/post-merge`. Read by its text
        // it ends at `.git`.
        let scratch = Scratch::new("way-up-through-link");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        fs::create_dir_all(hooks_dir.join("x").join("y")).unwrap();
        std::os::unix::fs::symlink("x/y", hooks_dir.join("L")).unwrap();
        std::os::unix::fs::symlink(
            "ori-original/../L/../../post-merge",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("ori-original/../L/../../post-merge"),
            &["L", "post-checkout", "post-commit", "x"],
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_whose_way_lands_beside_a_linked_directory_is_refused() {
        // H3b: the way is `ori-original/../../x/L/../post-merge`, where
        // `.git/x/L` links to `hooks/sub`. Read by its text it lands on
        // `.git/x`, a directory that is there; the system lands on `hooks`.
        let scratch = Scratch::new("way-beside-link");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let git_dir = repo.join(".git");
        fs::create_dir_all(hooks_dir.join("sub")).unwrap();
        fs::create_dir_all(git_dir.join("x")).unwrap();
        std::os::unix::fs::symlink("../hooks/sub", git_dir.join("x").join("L")).unwrap();
        std::os::unix::fs::symlink(
            "ori-original/../../x/L/../post-merge",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("ori-original/../../x/L/../post-merge"),
            &["post-checkout", "post-commit", "sub"],
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_that_goes_up_from_a_linked_directory_to_a_missing_ori_original_is_refused()
    {
        // The H3 layout with the missing directory after the link and its
        // `..`, so the resolver must follow `L` and take the `..` from where
        // `L` leads to know where `ori-original` would be: `hooks/x/y`, up
        // twice, is `hooks`. The text of `L/../..` is `.git`, where an
        // `ori-original` is not the installer's.
        let scratch = Scratch::new("way-up-from-link-then-missing");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        fs::create_dir_all(hooks_dir.join("x").join("y")).unwrap();
        std::os::unix::fs::symlink("x/y", hooks_dir.join("L")).unwrap();
        std::os::unix::fs::symlink(
            "L/../../ori-original/../post-merge",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("L/../../ori-original/../post-merge"),
            &["L", "post-checkout", "post-commit", "x"],
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_that_leaves_by_a_linked_directory_beside_the_hooks_to_a_missing_ori_original_is_refused()
     {
        // The H3b layout with the missing directory after the link: from
        // `hooks`, up to `.git`, into `x`, through `L` to `hooks/sub`, and up
        // from there to `hooks`. The text of `x/L/..` is `.git/x`.
        let scratch = Scratch::new("way-beside-link-then-missing");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let git_dir = repo.join(".git");
        fs::create_dir_all(hooks_dir.join("sub")).unwrap();
        fs::create_dir_all(git_dir.join("x")).unwrap();
        std::os::unix::fs::symlink("../hooks/sub", git_dir.join("x").join("L")).unwrap();
        std::os::unix::fs::symlink(
            "../x/L/../ori-original/../post-merge",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("../x/L/../ori-original/../post-merge"),
            &["post-checkout", "post-commit", "sub"],
        );
        assert_link_reaches_post_merge_once_ori_original_exists(
            &hooks_dir,
            &hooks_dir.join("post-checkout"),
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_way_that_fails_at_a_directory_the_install_does_not_create_is_still_read_by_its_text()
     {
        // `nowhere` is not `ori-original`, so the way reaches nothing, and no
        // install will change that. Its text is still read, as a second look
        // that fails closed: `nowhere/..` is the hooks directory, and
        // `nowhere/../../shared` is a directory that is there, with a link in
        // it to a hook the installer replaces. These are over-refusals, kept.
        let scratch = Scratch::new("way-text-over-refusal");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let shared = repo.join(".git").join("shared");
        fs::create_dir_all(&shared).unwrap();
        std::os::unix::fs::symlink("../hooks/post-merge", shared.join("relay")).unwrap();
        for written in ["nowhere/../post-merge", "nowhere/../../shared/relay"] {
            let link = hooks_dir.join("post-checkout");
            let _ = fs::remove_file(&link);
            std::os::unix::fs::symlink(written, &link).unwrap();
            expect_refused_link(
                &repo,
                &spool,
                &link,
                Path::new(written),
                &["post-checkout", "post-commit"],
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_through_a_hook_that_preserve_will_re_point_is_refused() {
        // H4, with no user `post-commit`: `pre-push` links to the real
        // directory `x`, so the install keeps it as `ori-original/pre-push`,
        // a link to `../x`, and `post-checkout ->
        // ori-original/pre-push/../post-merge` then goes through `x` and up to
        // `hooks`. The text of the way is `hooks/ori-original`.
        let scratch = Scratch::new("way-through-repointed-hook");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(hooks_dir.join("x")).unwrap();
        std::os::unix::fs::symlink("x", hooks_dir.join("pre-push")).unwrap();
        std::os::unix::fs::symlink(
            "ori-original/pre-push/../post-merge",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new("ori-original/pre-push/../post-merge"),
            &["post-checkout", "pre-push", "x"],
        );
        // What the install would have made: `preserve` re-points the link.
        fs::create_dir(hooks_dir.join("ori-original")).unwrap();
        std::os::unix::fs::symlink("../x", hooks_dir.join("ori-original").join("pre-push"))
            .unwrap();
        fs::write(hooks_dir.join("post-merge"), "#!/bin/sh\n").unwrap();
        assert_eq!(
            fs::canonicalize(hooks_dir.join("post-checkout")).unwrap(),
            fs::canonicalize(&hooks_dir).unwrap().join("post-merge"),
            "the layout does not reach the engine's post-merge, so the test proves nothing"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_link_through_a_missing_hook_entry_of_an_existing_ori_original_is_refused() {
        // After one install with a user's `post-commit`, `ori-original/`
        // holds `post-commit` and `pre-push` is the engine's script.
        // `pre-commit` is not a hook the installer replaces, so nothing of the
        // engine's stands in its place. `ori-original/pre-push` is missing and
        // is a hook's name, so the way is refused, whether or not this install
        // would make it: it would, the moment a user's own `pre-push`
        // replaced the engine's.
        let scratch = Scratch::new("way-through-missing-entry");
        let (repo, spool, hooks_dir, theirs) = repository_with_a_users_post_commit(&scratch);
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert_eq!(
            fs::read_to_string(preserved(&repo, GitHook::PostCommit)).unwrap(),
            theirs
        );
        assert!(fs::symlink_metadata(preserved(&repo, GitHook::PrePush)).is_err());
        assert!(
            fs::read_to_string(hooks_dir.join("pre-push"))
                .unwrap()
                .contains(MARKER)
        );
        std::os::unix::fs::symlink(
            "ori-original/pre-push/../post-merge",
            hooks_dir.join("pre-commit"),
        )
        .unwrap();
        // The first install made the spool; the refused one must not make it
        // again, which `expect_refused_link` looks for.
        fs::remove_dir_all(&spool).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("pre-commit"),
            Path::new("ori-original/pre-push/../post-merge"),
            &[
                "ori-original",
                "post-commit",
                "post-merge",
                "post-rewrite",
                "pre-commit",
                "pre-push",
            ],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_missing_entry_of_ori_original_that_is_not_a_hook_name_is_not_refused() {
        // The same layout, but the missing entry is `notahook`, which the
        // installer never makes in `ori-original/`: the way reaches nothing,
        // and the link is left as it was.
        let scratch = Scratch::new("way-through-missing-non-hook");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        std::os::unix::fs::symlink("ori-original/notahook/x", hooks_dir.join("pre-commit"))
            .unwrap();
        let report = GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert!(report.is_unchanged(), "{report:?}");
        assert_eq!(
            fs::read_link(hooks_dir.join("pre-commit")).unwrap(),
            Path::new("ori-original/notahook/x")
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_loop_where_a_missing_directory_is_read_by_its_text_refuses_the_install() {
        // `nowhere` is not `ori-original`, so the way is not refused for
        // entering what the install creates. Its text,
        // `nowhere/../../loopdir`, lands on `.git/loopdir`, which links to
        // itself: the landing cannot be resolved, and the reason is not that
        // it is missing, so the install is refused as an IO error and not
        // passed (AICD §14).
        let scratch = Scratch::new("way-text-lands-on-a-loop");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        std::os::unix::fs::symlink("loopdir", repo.join(".git").join("loopdir")).unwrap();
        std::os::unix::fs::symlink(
            "nowhere/../../loopdir/relay",
            hooks_dir.join("post-checkout"),
        )
        .unwrap();

        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::Io { path, .. } => assert_eq!(path, &hooks_dir.join("post-checkout")),
            other => panic!("expected an IO error naming the link, got {other}"),
        }
        assert_eq!(
            hook_dir_names(&repo),
            ["post-checkout", "post-commit"],
            "a refused installation wrote into the hooks directory"
        );
        assert!(
            !spool.exists(),
            "the spool was created by a refused install"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_way_through_more_links_than_any_process_follows_is_an_error() {
        // A link to itself in the way: the resolver follows links itself, so
        // it needs a bound of its own, and a way it cannot finish is an error
        // that refuses the install (AICD §14), not an answer of "no".
        let scratch = Scratch::new("way-link-cap");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("loop", hooks_dir.join("loop")).unwrap();
        let error = way_enters_what_the_install_creates(
            &hooks_dir.join("loop").join("x"),
            &hooks_dir,
            &canonical,
            false,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains(&LINK_HOPS_CAP.to_string()),
            "the error does not name the cap: {error}"
        );

        // The bound is the cap and not another number: a chain of exactly
        // that many links, ending at a real directory, is followed to its end
        // (the way then fails at `x`, which is not the installer's), and one
        // more is an error.
        let chain = |prefix: &str, links: usize| {
            fs::create_dir(hooks_dir.join(format!("{prefix}-dir"))).unwrap();
            for number in 1..=links {
                let target = if number == links {
                    format!("{prefix}-dir")
                } else {
                    format!("{prefix}{}", number + 1)
                };
                std::os::unix::fs::symlink(target, hooks_dir.join(format!("{prefix}{number}")))
                    .unwrap();
            }
            hooks_dir.join(format!("{prefix}1")).join("x")
        };
        let at_the_cap = chain("a", LINK_HOPS_CAP);
        assert!(
            !way_enters_what_the_install_creates(&at_the_cap, &hooks_dir, &canonical, false)
                .unwrap()
        );
        let past_the_cap = chain("b", LINK_HOPS_CAP + 1);
        assert!(
            way_enters_what_the_install_creates(&past_the_cap, &hooks_dir, &canonical, false)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_what_the_install_creates_is_named_without_regard_to_case_where_names_fold_case() {
        // Both readings, whatever this platform's file system does: the
        // comparison is a parameter so each is tested everywhere. `ORI-ORIGINAL`
        // is a different directory where names are exact, and the way through
        // it reaches nothing.
        let scratch = Scratch::new("way-case");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let enters = |path: PathBuf, fold_case: bool| {
            way_enters_what_the_install_creates(&path, &hooks_dir, &canonical, fold_case).unwrap()
        };

        // The directory itself.
        assert!(enters(hooks_dir.join("ORI-ORIGINAL"), true));
        assert!(!enters(hooks_dir.join("ORI-ORIGINAL"), false));
        assert!(enters(hooks_dir.join("ori-original"), false));

        // A hook's name in a directory called `ori-original` that is there.
        fs::create_dir(hooks_dir.join("ori-original")).unwrap();
        assert!(enters(
            hooks_dir.join("ori-original").join("PRE-PUSH"),
            true
        ));
        assert!(!enters(
            hooks_dir.join("ori-original").join("PRE-PUSH"),
            false
        ));
        assert!(enters(
            hooks_dir.join("ori-original").join("pre-push"),
            false
        ));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_relative_way_is_resolved_from_the_current_directory() {
        // The installer passes absolute paths, but the resolver takes any: a
        // relative one starts at the current directory. This one climbs to
        // the root (a `..` there stays) and comes down to the hooks directory,
        // without the test changing the directory of the process. The same
        // way with one climb too few starts in an ancestor of the current
        // directory that is not the root, so it does not come down to the
        // hooks directory and is not the installer's.
        let scratch = Scratch::new("way-relative");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let depth = std::env::current_dir().unwrap().components().count();
        let up: PathBuf = std::iter::repeat_n("..", depth + 1).collect();
        let relative = |tail: &str| up.join(canonical.strip_prefix("/").unwrap()).join(tail);
        assert!(
            way_enters_what_the_install_creates(
                &relative("ori-original"),
                &hooks_dir,
                &canonical,
                false
            )
            .unwrap()
        );
        assert!(
            !way_enters_what_the_install_creates(
                &relative("nowhere"),
                &hooks_dir,
                &canonical,
                false
            )
            .unwrap()
        );
        if depth > 1 {
            let short: PathBuf = std::iter::repeat_n("..", depth - 2).collect();
            let one_climb_short = short
                .join(canonical.strip_prefix("/").unwrap())
                .join("ori-original");
            assert!(
                !way_enters_what_the_install_creates(
                    &one_climb_short,
                    &hooks_dir,
                    &canonical,
                    false
                )
                .unwrap(),
                "a relative way did not start at the current directory"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_a_way_that_resolves_after_the_system_said_it_did_not_is_refused() {
        // The resolver is reached only after `canonicalize` failed with "not
        // found". A way whose every component exists now means the tree
        // changed between the two looks, and the text reading that would
        // follow could misread it (an H3-shaped way), so it is refused and the
        // install is run again.
        let scratch = Scratch::new("way-resolves-after-all");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(hooks_dir.join("x").join("y")).unwrap();
        std::os::unix::fs::symlink("x/y", hooks_dir.join("L")).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        for way in [
            hooks_dir.clone(),
            hooks_dir.join("x").join("y"),
            hooks_dir.join("L").join(".."),
            hooks_dir.join("L").join("..").join(".."),
        ] {
            assert!(
                way_enters_what_the_install_creates(&way, &hooks_dir, &canonical, false).unwrap(),
                "{way:?} resolves and was not refused"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0174_an_ori_original_that_is_not_the_hooks_directorys_own_is_not_what_the_install_creates()
     {
        // The install creates the hooks directory's own `ori-original/` and a
        // hook's name in it, and nothing else, so a missing directory of that
        // name elsewhere, or a hook's name missing from a directory that is
        // not that one, is not refused by the rule. Each case below is made so
        // that one narrowing condition alone separates it from the controls.
        let scratch = Scratch::new("way-not-the-hooks-own");
        let (repo, _) = skeleton(&scratch);
        let git_dir = repo.join(".git");
        let hooks_dir = git_dir.join("hooks");
        fs::create_dir_all(hooks_dir.join("sub").join("ori-original")).unwrap();
        fs::create_dir_all(git_dir.join("x").join("ori-original")).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let enters = |path: PathBuf| {
            way_enters_what_the_install_creates(&path, &hooks_dir, &canonical, false).unwrap()
        };

        // The first clause needs the missing `ori-original` to be under the
        // hooks directory: here it would be under `.git`.
        assert!(!enters(git_dir.join("ori-original").join("..")));
        // The second clause needs the directory to be named `ori-original`:
        // here the missing hook is in `hooks/sub`, whose parent is `hooks`.
        assert!(!enters(hooks_dir.join("sub").join("pre-push")));
        // And needs that directory to be under the hooks directory: these two
        // are named `ori-original`, but under `.git/x` and under `hooks/sub`.
        assert!(!enters(
            git_dir.join("x").join("ori-original").join("pre-push")
        ));
        assert!(!enters(
            hooks_dir.join("sub").join("ori-original").join("pre-push")
        ));

        // The controls: the hooks directory's own, missing and then there.
        assert!(enters(hooks_dir.join("ori-original").join("..")));
        fs::create_dir(hooks_dir.join("ori-original")).unwrap();
        assert!(enters(hooks_dir.join("ori-original").join("pre-push")));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_git_dir_replaced_by_a_link_after_the_installer_was_built_is_refused() {
        let scratch = Scratch::new("hooksgitswap");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();

        // `.git` is moved away and a link to another directory put back.
        let elsewhere = scratch.dir("elsewhere");
        fs::rename(repo.join(".git"), scratch.path.join("moved-git")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, repo.join(".git")).unwrap();
        let error = hooks.install().unwrap_err();
        assert!(
            matches!(error, HooksError::GitDirNotADirectory { .. }),
            "{error}"
        );
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        let error = hooks.status().unwrap_err();
        assert!(
            matches!(error, HooksError::GitDirNotADirectory { .. }),
            "{error}"
        );
        assert_eq!(
            fs::read_dir(&elsewhere).unwrap().count(),
            0,
            "something was written through the link"
        );
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
            // Built as text: on Windows the scratch path is verbatim (`\\?\`),
            // and `join` normalizes a `..` away on a verbatim path, which
            // would leave a plain sibling directory to judge.
            {
                let mut text = scratch.path.clone().into_os_string();
                for part in ["x", "..", "spool"] {
                    text.push(std::path::MAIN_SEPARATOR_STR);
                    text.push(part);
                }
                PathBuf::from(text)
            },
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

    /// Runs `work` on its own thread and fails, rather than hangs, if it does
    /// not finish: a call that blocks forever must be a failing test. Used by
    /// the tests that make a pipe, so Unix only.
    #[cfg(unix)]
    fn within<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{what} did not return within 5 s: it blocked"))
    }

    /// The names in the hooks directory, sorted.
    fn hook_dir_names(repo: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(repo.join(".git").join("hooks"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn ori_t_0054_a_preserved_hook_that_appears_after_the_plan_is_never_overwritten() {
        let scratch = Scratch::new("raceolder");
        let (repo, spool) = skeleton(&scratch);
        let existing = hook_path(&repo, GitHook::PrePush);
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, "#!/bin/sh\necho theirs\n").unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        let plan = hooks.plan(GitHook::PrePush).unwrap();
        assert!(matches!(plan, Plan::Preserve));

        // Between the plan and the work, an older preserved hook appears.
        let older = preserved(&repo, GitHook::PrePush);
        fs::create_dir_all(older.parent().unwrap()).unwrap();
        fs::write(&older, "#!/bin/sh\necho older\n").unwrap();
        let error = hooks.apply(GitHook::PrePush, plan).unwrap_err();
        assert!(
            matches!(error, HooksError::PreservedNameTaken { .. }),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(&older).unwrap(),
            "#!/bin/sh\necho older\n"
        );
        assert_eq!(
            fs::read_to_string(&existing).unwrap(),
            "#!/bin/sh\necho theirs\n"
        );
        assert_eq!(hook_dir_names(&repo), vec!["ori-original", "pre-push"]);
    }

    #[test]
    fn ori_t_0054_a_hook_that_appears_or_changes_after_the_plan_is_not_replaced() {
        let scratch = Scratch::new("raceappear");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        let path = hook_path(&repo, GitHook::PostMerge);

        // Absent at the plan, theirs by the work.
        let plan = hooks.plan(GitHook::PostMerge).unwrap();
        assert!(matches!(plan, Plan::Write));
        fs::write(&path, "#!/bin/sh\necho theirs\n").unwrap();
        let error = hooks.apply(GitHook::PostMerge, plan).unwrap_err();
        assert!(matches!(error, HooksError::HookAppeared { .. }), "{error}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "#!/bin/sh\necho theirs\n"
        );
        assert_eq!(hook_dir_names(&repo), vec!["post-merge"]);
        assert_eq!(error.methodology_ref().unwrap().section, 13);

        // Ours at the plan, theirs by the work.
        fs::remove_file(&path).unwrap();
        hooks.install().unwrap();
        let other = GitHooks::new(&repo, &scratch.path.join("other-spool")).unwrap();
        let plan = other.plan(GitHook::PostMerge).unwrap();
        assert!(matches!(plan, Plan::Rewrite));
        fs::write(&path, "#!/bin/sh\necho theirs, later\n").unwrap();
        let error = other.apply(GitHook::PostMerge, plan).unwrap_err();
        assert!(matches!(error, HooksError::HookAppeared { .. }), "{error}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "#!/bin/sh\necho theirs, later\n"
        );
        assert!(
            !hook_dir_names(&repo)
                .iter()
                .any(|name| name.contains("ori-tmp")),
            "a temporary file was left behind: {:?}",
            hook_dir_names(&repo)
        );
    }

    #[test]
    fn ori_t_0054_concurrent_installs_never_lose_the_hook_that_was_there() {
        for round in 0..20 {
            let scratch = Scratch::new(&format!("concurrent{round}"));
            let (repo, spool) = skeleton(&scratch);
            let theirs = [
                (GitHook::PostCommit, "#!/bin/sh\necho their post-commit\n"),
                (GitHook::PrePush, "#!/bin/sh\necho their pre-push\n"),
            ];
            for (hook, text) in theirs {
                let path = hook_path(&repo, hook);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, text).unwrap();
                make_executable(&path).unwrap();
            }
            let hooks = std::sync::Arc::new(GitHooks::new(&repo, &spool).unwrap());
            let results: Vec<_> = (0..6)
                .map(|_| {
                    let hooks = std::sync::Arc::clone(&hooks);
                    std::thread::spawn(move || hooks.install())
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();
            for result in &results {
                match result {
                    Ok(_)
                    | Err(
                        HooksError::PreservedNameTaken { .. } | HooksError::HookAppeared { .. },
                    ) => {}
                    Err(other) => panic!("round {round}: an unexpected failure: {other}"),
                }
            }
            assert!(
                results.iter().any(Result::is_ok),
                "round {round}: no install won"
            );
            for (hook, text) in theirs {
                assert_eq!(
                    fs::read_to_string(preserved(&repo, hook)).unwrap_or_default(),
                    text,
                    "round {round}: the hook that was there is not preserved"
                );
            }
            // A last install settles whatever the losers left undone.
            hooks.install().unwrap();
            for status in hooks.status().unwrap() {
                assert_eq!(
                    status.state,
                    HookState::Installed,
                    "round {round}: {status:?}"
                );
            }
            assert!(
                !hook_dir_names(&repo)
                    .iter()
                    .any(|name| name.contains("ori-tmp")),
                "round {round}: {:?}",
                hook_dir_names(&repo)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_git_config_that_is_a_pipe_a_link_or_huge_is_refused_without_blocking() {
        let scratch = Scratch::new("config");
        let (repo, spool) = skeleton(&scratch);
        let config = repo.join(".git").join("config");
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        let refuse = |hooks: &GitHooks, what: &str| {
            let hooks = hooks.clone();
            let error = within(what, move || hooks.install()).unwrap_err();
            assert!(
                matches!(error, HooksError::ConfigNotReadable { .. }),
                "{what}: {error}"
            );
            assert_eq!(error.methodology_ref().unwrap().section, 14, "{what}");
        };

        // A named pipe: open(2) on it with no writer would wait for ever.
        let status = std::process::Command::new("mkfifo")
            .arg(&config)
            .status()
            .unwrap();
        assert!(status.success());
        refuse(&hooks, "install over a .git/config that is a pipe");
        fs::remove_file(&config).unwrap();

        // A link to a file that says hooksPath is not followed or believed.
        let elsewhere = scratch.path.join("elsewhere.cfg");
        fs::write(&elsewhere, "[core]\n\thooksPath = x\n").unwrap();
        std::os::unix::fs::symlink(&elsewhere, &config).unwrap();
        refuse(&hooks, "install over a .git/config that is a link");
        fs::remove_file(&config).unwrap();

        // Too long to read in full: a hooksPath could be past the cap.
        let line = "# padding padding padding padding padding padding padding\n";
        fs::write(&config, line.repeat((READ_CAP as usize / line.len()) + 8)).unwrap();
        refuse(&hooks, "install over a .git/config longer than the cap");
        assert!(
            !repo.join(".git").join("hooks").exists(),
            "nothing was written"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_spool_log_that_is_a_pipe_is_refused_without_blocking() {
        let scratch = Scratch::new("logpipe");
        let spool = scratch.dir("spool");
        let status = std::process::Command::new("mkfifo")
            .arg(spool.join(SPOOL_LOG_NAME))
            .status()
            .unwrap();
        assert!(status.success());
        let error = within("draining a log that is a pipe", move || {
            HookSpool::open(&spool).drain()
        })
        .unwrap_err();
        assert!(
            matches!(error, HooksError::SpoolLogNotAFile { .. }),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_log_replaced_by_a_longer_file_is_read_again_from_the_start() {
        let scratch = Scratch::new("logswap");
        let spool = scratch.dir("spool");
        let log = spool.join(SPOOL_LOG_NAME);
        fs::write(&log, "1\tpost-commit\ta\tb\n2\tpost-commit\tc\td\n").unwrap();
        let mut reader = HookSpool::open(&spool);
        let first = reader.drain().unwrap();
        assert_eq!(first.events.len(), 2);
        assert!(!first.restarted);

        // A different file, longer than the offset, takes the log's name.
        let replacement = spool.join("replacement");
        fs::write(
            &replacement,
            "3\tpost-commit\te\tf\n4\tpost-commit\tg\th\n5\tpost-commit\ti\tj\n6\tpost-merge\t0\tk\n",
        )
        .unwrap();
        fs::rename(&replacement, &log).unwrap();
        let second = reader.drain().unwrap();
        assert!(
            second.restarted,
            "a replaced log must be said to be restarted"
        );
        assert_eq!(second.events.len(), 4, "{second:?}");
        assert_eq!(second.events[0].fields, vec!["e", "f"]);
    }

    // ---- ORI-T-0180 and ORI-T-0168: what depends on the process, and ori-original ----

    /// What a refused installation must leave behind it: the entries of the
    /// hooks directory as they were, and no spool.
    fn assert_nothing_written(repo: &Path, spool: &Path, names_before: &[&str]) {
        assert_eq!(
            hook_dir_names(repo),
            names_before,
            "a refused installation wrote into the hooks directory"
        );
        assert!(
            !spool.exists(),
            "the spool was created by a refused install"
        );
    }

    /// Installs and expects `HookLinkDependsOnProcess` for `link`, as written
    /// `target`, through `through`, and that nothing was written.
    #[cfg(target_os = "linux")]
    fn expect_refused_for_the_process(
        repo: &Path,
        spool: &Path,
        link: &Path,
        target: &Path,
        through: &Path,
        names_before: &[&str],
    ) {
        let error = GitHooks::new(repo, spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::HookLinkDependsOnProcess {
                path,
                target: seen,
                through: entered,
            } => {
                assert_eq!(path, link);
                assert_eq!(seen, target);
                assert_eq!(entered, through);
            }
            other => panic!("expected HookLinkDependsOnProcess, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        let text = error.to_string();
        assert!(
            text.contains(&through.display().to_string()) && text.ends_with("(AICD §13)"),
            "{text}"
        );
        assert_nothing_written(repo, spool, names_before);
        assert_eq!(fs::read_link(link).unwrap(), target);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_hook_link_through_proc_self_is_refused() {
        let scratch = Scratch::new("proc-self");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let link = hooks_dir.join("post-checkout");
        let target = Path::new("/proc/self/cwd/.git/hooks/post-merge");
        std::os::unix::fs::symlink(target, &link).unwrap();

        // The premise: the installer resolves the link from its own working
        // directory, the package directory, which has no `.git`, so the check
        // that reads the chain passes it. For git, whose working directory is
        // the worktree's root, it is the engine's own `post-merge`.
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let reached = follow_chain_to_managed_hook(&hooks_dir, &canonical, &link, false);
        assert!(
            matches!(reached, Ok(None)),
            "the premise fails: the chain check alone does not pass this link from the test \
             process's working directory, so the test proves nothing about the new walk: \
             {reached:?}"
        );

        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_hook_link_through_a_link_to_proc_is_refused() {
        // `.git/x` is outside the hooks directory, so it is not scanned
        // itself; the link in the hooks directory goes through it.
        let scratch = Scratch::new("proc-through-link");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("/proc/self/cwd", repo.join(".git").join("x")).unwrap();
        let link = hooks_dir.join("post-checkout");
        let target = Path::new("../x/.git/hooks/post-merge");
        std::os::unix::fs::symlink(target, &link).unwrap();
        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_hook_link_to_dev_stdin_is_refused() {
        // On Linux `/dev/stdin` is a link into `/proc/self`: it is followed
        // there, so it counts through `/proc`.
        if fs::symlink_metadata("/dev/stdin").is_err() {
            eprintln!("/dev/stdin is not here, so the case is not exercised");
            return;
        }
        let scratch = Scratch::new("dev-stdin");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let link = hooks_dir.join("post-checkout");
        let target = Path::new("/dev/stdin");
        std::os::unix::fs::symlink(target, &link).unwrap();
        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0180_a_hook_link_to_dev_null_is_kept() {
        // A common way to disable a hook, and the same for every process.
        let scratch = Scratch::new("dev-null");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let link = hooks_dir.join("post-checkout");
        std::os::unix::fs::symlink("/dev/null", &link).unwrap();
        GitHooks::new(&repo, &spool).unwrap().install().unwrap();
        assert_eq!(
            fs::read_link(&link).unwrap(),
            Path::new("/dev/null"),
            "a link to /dev/null was moved or replaced"
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        for hook in GitHook::ALL {
            assert!(is_ours(
                &fs::read_to_string(hook_path(&repo, hook)).unwrap()
            ));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_the_per_process_walk_stops_at_the_first_per_process_directory() {
        let scratch = Scratch::new("per-process-walk");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let walk = |name: &str| way_enters_a_per_process_directory(&hooks_dir.join(name), false);

        // A link to `/proc` enters it; the walk stops there.
        std::os::unix::fs::symlink("/proc", hooks_dir.join("to-proc")).unwrap();
        assert_eq!(walk("to-proc").unwrap(), Some(PathBuf::from("/proc")));

        // `/procx` is not `/proc`, and it is not there.
        assert!(!Path::new("/procx").exists());
        std::os::unix::fs::symlink("/procx", hooks_dir.join("to-procx")).unwrap();
        assert_eq!(walk("to-procx").unwrap(), None);

        // The cap is `LINK_HOPS_CAP` links: a chain of that many that ends at
        // `/proc` is followed to it, and one link more is not an error but a
        // way no process can follow, which enters nothing.
        let chain = |prefix: &str, links: usize| {
            for number in 1..=links {
                let target = if number == links {
                    "/proc".to_owned()
                } else {
                    format!("{prefix}{}", number + 1)
                };
                std::os::unix::fs::symlink(target, hooks_dir.join(format!("{prefix}{number}")))
                    .unwrap();
            }
            format!("{prefix}1")
        };
        let at_the_cap = chain("at-", LINK_HOPS_CAP);
        assert_eq!(walk(&at_the_cap).unwrap(), Some(PathBuf::from("/proc")));
        let past_the_cap = chain("past-", LINK_HOPS_CAP + 1);
        assert_eq!(
            walk(&past_the_cap).unwrap(),
            None,
            "a chain past the cap is no, not an error"
        );

        // A link to itself.
        std::os::unix::fs::symlink("self-loop", hooks_dir.join("self-loop")).unwrap();
        assert_eq!(walk("self-loop").unwrap(), None);

        // A link into a per-process directory's own links is not read: the
        // walk stops on entering `/proc`, before `self`.
        std::os::unix::fs::symlink("/proc/self/cwd", hooks_dir.join("to-cwd")).unwrap();
        assert_eq!(walk("to-cwd").unwrap(), Some(PathBuf::from("/proc")));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0180_which_directories_are_per_process() {
        for fold_case in [false, true] {
            for path in [
                "/proc",
                "/proc/1/cwd",
                "/dev/fd",
                "/dev/fd/3",
                "/compat/linux/proc",
                "/compat/linux/proc/1/cwd",
            ] {
                assert!(
                    is_per_process_directory(Path::new(path), fold_case),
                    "{path} (fold_case {fold_case}) is per-process"
                );
            }
            for path in [
                "/dev",
                "/dev/null",
                "/procfs",
                "/home/proc",
                "proc",
                "/compat",
                "/compat/linux",
                "/compat/linux/procfs",
            ] {
                assert!(
                    !is_per_process_directory(Path::new(path), fold_case),
                    "{path} (fold_case {fold_case}) is not per-process"
                );
            }
        }
        for path in ["/PROC", "/DEV/FD", "/COMPAT/LINUX/PROC"] {
            assert!(
                !is_per_process_directory(Path::new(path), false),
                "{path} is not per-process where names are exact"
            );
            assert!(
                is_per_process_directory(Path::new(path), true),
                "{path} is per-process where names fold case"
            );
        }
    }

    /// The repository for the `ori-original` tests: a hooks directory and
    /// nothing in it.
    fn repository_with_an_empty_hooks_directory(scratch: &Scratch) -> (PathBuf, PathBuf, PathBuf) {
        let (repo, spool) = skeleton(scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        (repo, spool, hooks_dir)
    }

    /// Installs and expects `HooksDirNotADirectory` for `expected`, and that
    /// nothing was written.
    fn expect_refused_ori_original(
        repo: &Path,
        spool: &Path,
        expected: &Path,
        names_before: &[&str],
    ) {
        let error = GitHooks::new(repo, spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, expected),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        assert_nothing_written(repo, spool, names_before);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_an_ori_original_that_links_to_the_hooks_directory_is_refused_with_no_hook_of_anyone_elses()
     {
        let scratch = Scratch::new("ori-original-dot");
        let (repo, spool, hooks_dir) = repository_with_an_empty_hooks_directory(&scratch);
        let preserved_dir = hooks_dir.join("ori-original");
        std::os::unix::fs::symlink(".", &preserved_dir).unwrap();

        // The premise: through the link, `ori-original/<name>` is the hooks
        // directory again, at every depth, so a script that runs it when it is
        // executable would run itself.
        assert_eq!(
            fs::canonicalize(preserved_dir.join("ori-original")).unwrap(),
            fs::canonicalize(&hooks_dir).unwrap(),
            "the layout is not the one that loops, so the test proves nothing"
        );

        expect_refused_ori_original(&repo, &spool, &preserved_dir, &["ori-original"]);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_an_ori_original_that_links_out_of_the_repository_is_refused() {
        let scratch = Scratch::new("ori-original-out");
        let (repo, spool, hooks_dir) = repository_with_an_empty_hooks_directory(&scratch);
        let elsewhere = scratch.dir("elsewhere");
        let theirs = "#!/bin/sh\necho theirs\n";
        fs::write(elsewhere.join("post-commit"), theirs).unwrap();
        make_executable(&elsewhere.join("post-commit")).unwrap();
        let preserved_dir = hooks_dir.join("ori-original");
        std::os::unix::fs::symlink(&elsewhere, &preserved_dir).unwrap();

        expect_refused_ori_original(&repo, &spool, &preserved_dir, &["ori-original"]);
        assert_eq!(
            fs::read_dir(&elsewhere).unwrap().count(),
            1,
            "something was written beyond the link"
        );
        assert_eq!(
            fs::read_to_string(elsewhere.join("post-commit")).unwrap(),
            theirs
        );
    }

    #[test]
    fn ori_t_0168_an_ori_original_that_is_a_file_is_refused() {
        let scratch = Scratch::new("ori-original-file");
        let (repo, spool, hooks_dir) = repository_with_an_empty_hooks_directory(&scratch);
        let preserved_dir = hooks_dir.join("ori-original");
        fs::write(&preserved_dir, "not a directory\n").unwrap();

        expect_refused_ori_original(&repo, &spool, &preserved_dir, &["ori-original"]);
        assert_eq!(
            fs::read_to_string(&preserved_dir).unwrap(),
            "not a directory\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_an_ori_original_made_a_link_after_an_install_is_refused_on_the_next() {
        let scratch = Scratch::new("ori-original-after");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let written: Vec<Vec<u8>> = GitHook::ALL
            .iter()
            .map(|hook| fs::read(hook_path(&repo, *hook)).unwrap())
            .collect();

        let preserved_dir = hooks_dir.join("ori-original");
        std::os::unix::fs::symlink(".", &preserved_dir).unwrap();
        let error = hooks.install().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, &preserved_dir),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(
            hook_dir_names(&repo),
            [
                "ori-original",
                "post-commit",
                "post-merge",
                "post-rewrite",
                "pre-push"
            ],
            "a refused installation wrote into the hooks directory"
        );
        // The first install made the spool; the refused one added nothing to it.
        assert_eq!(fs::read_dir(&spool).unwrap().count(), 0);
        for (hook, before) in GitHook::ALL.iter().zip(&written) {
            assert_eq!(
                &fs::read(hook_path(&repo, *hook)).unwrap(),
                before,
                "{hook:?} changed under a refused installation"
            );
        }
    }

    // ---- ORI-T-0180, repair 1: ways through directories that are not there yet ----

    /// The text of a link whose way climbs out of what is there, goes on past
    /// `prefix` (a way through directories that are not there, which git
    /// makes later) and ends at the engine's `post-merge` through the working
    /// directory of whatever process follows it: `prefix`, forty `../`, then
    /// `proc/self/cwd/.git/hooks/post-merge`.
    #[cfg(target_os = "linux")]
    fn way_through_proc_after(prefix: &str) -> String {
        format!(
            "{prefix}{}proc/self/cwd/.git/hooks/post-merge",
            "../".repeat(40)
        )
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_way_through_a_directory_git_creates_later_is_still_judged() {
        // `.git/logs` is made by the first commit. Until then the way stops at
        // it, and the text of what follows used to be read as the installer's
        // process sees `/proc/self`.
        let scratch = Scratch::new("later-logs");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        assert!(!repo.join(".git").join("logs").exists());
        let link = hooks_dir.join("post-checkout");
        let written = way_through_proc_after("../logs/");
        let target = Path::new(&written);
        std::os::unix::fs::symlink(target, &link).unwrap();

        // The premise: the chain check alone passes it.
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let reached = follow_chain_to_managed_hook(&hooks_dir, &canonical, &link, false);
        assert!(
            matches!(reached, Ok(None)),
            "the premise fails: the chain check alone does not pass this link, so the test \
             proves nothing about the walk past a missing component: {reached:?}"
        );

        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_way_through_a_missing_worktree_directory_is_still_judged() {
        // A checkout of a branch that tracks `d/f` makes `d/`, in the working
        // tree, and runs `post-checkout` after it.
        let scratch = Scratch::new("later-worktree-dir");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        assert!(!repo.join("d").exists());
        let link = hooks_dir.join("post-checkout");
        let written = way_through_proc_after("../../d/");
        let target = Path::new(&written);
        std::os::unix::fs::symlink(target, &link).unwrap();
        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_way_that_descends_into_missing_names_and_climbs_back_is_judged() {
        // The text climbs back through two missing levels, and then through
        // forty more `..`, which are physical again.
        let scratch = Scratch::new("later-descends");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let link = hooks_dir.join("post-checkout");
        let written = way_through_proc_after("../logs/a/b/../../../");
        let target = Path::new(&written);
        std::os::unix::fs::symlink(target, &link).unwrap();
        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_way_that_climbs_back_from_a_missing_name_goes_on_physically() {
        // `..` after a missing name returns to the directory that is there, and
        // the next name is looked at there: here it is a link, which the walk
        // must read. Read as text alone, `.git/x/.git/hooks` is not
        // anything, and the way would pass.
        let scratch = Scratch::new("later-climbs-back");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("/proc/self/cwd", repo.join(".git").join("x")).unwrap();
        let link = hooks_dir.join("post-checkout");
        let target = Path::new("../logs/../x/.git/hooks/post-merge");
        std::os::unix::fs::symlink(target, &link).unwrap();

        // The premise: the chain check alone passes it.
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let reached = follow_chain_to_managed_hook(&hooks_dir, &canonical, &link, false);
        assert!(
            matches!(reached, Ok(None)),
            "the premise fails: the chain check alone does not pass this link, so the test \
             proves nothing about the walk: {reached:?}"
        );

        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_the_walk_past_a_missing_component_follows_the_text_and_ends() {
        let scratch = Scratch::new("later-walk");
        let (repo, _) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let walk = |name: &str| way_enters_a_per_process_directory(&hooks_dir.join(name), false);

        // Names that stay missing reach nothing, however deep.
        std::os::unix::fs::symlink("missing/a/b", hooks_dir.join("stays-missing")).unwrap();
        assert_eq!(walk("stays-missing").unwrap(), None);
        // A `..` that climbs only to a name that is not there does not reach
        // the root, so it does not reach `/proc`.
        std::os::unix::fs::symlink(
            "missing/../../../../../../../../../../nowhere/proc",
            hooks_dir.join("not-a-proc"),
        )
        .unwrap();
        assert_eq!(walk("not-a-proc").unwrap(), None);

        // A link back to itself through a missing name: the cap ends it, and
        // it is no answer of an error.
        std::os::unix::fs::symlink("missing/../loop", hooks_dir.join("loop")).unwrap();
        assert_eq!(walk("loop").unwrap(), None);

        // A file that has steps after it is read like a directory that is not
        // there: it could be replaced by one.
        fs::write(hooks_dir.join("file.sh"), "#!/bin/sh\n").unwrap();
        let through_a_file = way_through_proc_after("file.sh/../");
        std::os::unix::fs::symlink(&through_a_file, hooks_dir.join("through-a-file")).unwrap();
        assert_eq!(
            walk("through-a-file").unwrap(),
            Some(PathBuf::from("/proc"))
        );
        // At the last step a file ends the way.
        std::os::unix::fs::symlink("file.sh", hooks_dir.join("to-file")).unwrap();
        assert_eq!(walk("to-file").unwrap(), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_the_per_process_walk_runs_before_the_chain_check() {
        // `/proc/self/root` is `/` for every process, so the chain check alone
        // resolves this way to the hooks directory and refuses it as reaching
        // `post-merge`. The way enters `/proc`, though, and what is said about it
        // is that it depends on the process.
        let scratch = Scratch::new("walk-first");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        std::os::unix::fs::symlink("/proc/self/root", repo.join(".git").join("x")).unwrap();
        let link = hooks_dir.join("post-checkout");
        let written = format!("../x{}/.git/hooks/post-merge", repo.display());
        let target = Path::new(&written);
        std::os::unix::fs::symlink(target, &link).unwrap();

        // The premise: the chain check alone does refuse it.
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let reached = follow_chain_to_managed_hook(&hooks_dir, &canonical, &link, false);
        assert!(
            matches!(&reached, Ok(Some(seen)) if seen == target),
            "the premise fails: the chain check alone does not refuse this link, so the order \
             is not what the test pins: {reached:?}"
        );

        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            target,
            Path::new("/proc"),
            &["post-checkout"],
        );
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    #[test]
    fn ori_t_0180_a_hook_link_to_dev_fd_is_refused() {
        // `/dev/fd` is a directory there (fdescfs): the walk asks about it
        // before it asks the platform to resolve it. This runs on the macOS leg
        // of CI.
        let scratch = Scratch::new("dev-fd");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let link = hooks_dir.join("post-checkout");
        let target = Path::new("/dev/fd");
        std::os::unix::fs::symlink(target, &link).unwrap();
        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::HookLinkDependsOnProcess {
                path,
                target: seen,
                through,
            } => {
                assert_eq!(path, &link);
                assert_eq!(seen, target);
                assert_eq!(through, Path::new("/dev/fd"));
            }
            other => panic!("expected HookLinkDependsOnProcess, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        assert_nothing_written(&repo, &spool, &["post-checkout"]);
    }

    // ---- ORI-T-0168, repair 1: the entries of ori-original the scripts run ----

    /// A repository whose hooks directory holds a real `ori-original/`, in
    /// which the link `post-commit -> target` has been put by hand, and nothing
    /// else.
    #[cfg(unix)]
    fn repository_with_a_link_in_ori_original(
        scratch: &Scratch,
        target: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let (repo, spool, hooks_dir) = repository_with_an_empty_hooks_directory(scratch);
        let kept = hooks_dir.join("ori-original");
        fs::create_dir(&kept).unwrap();
        let link = kept.join("post-commit");
        std::os::unix::fs::symlink(target, &link).unwrap();
        (repo, spool, link)
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_a_link_inside_ori_original_that_reaches_a_replaced_hook_is_refused() {
        // The scripts run `ori-original/post-commit`; through this link that is
        // the engine's own `post-merge`.
        let scratch = Scratch::new("kept-link-to-hook");
        let (repo, spool, link) = repository_with_a_link_in_ori_original(&scratch, "../post-merge");
        expect_refused_link(
            &repo,
            &spool,
            &link,
            Path::new("../post-merge"),
            &["ori-original"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_a_link_inside_ori_original_to_its_own_hook_is_refused() {
        // `ori-original/post-commit -> ../post-commit`: the engine's script
        // would run itself a second time.
        let scratch = Scratch::new("kept-link-to-itself");
        let (repo, spool, link) =
            repository_with_a_link_in_ori_original(&scratch, "../post-commit");
        expect_refused_link(
            &repo,
            &spool,
            &link,
            Path::new("../post-commit"),
            &["ori-original"],
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ori_t_0180_a_link_inside_ori_original_through_proc_is_refused() {
        let scratch = Scratch::new("kept-link-through-proc");
        let target = "/proc/self/cwd/.git/hooks/post-merge";
        let (repo, spool, link) = repository_with_a_link_in_ori_original(&scratch, target);
        expect_refused_for_the_process(
            &repo,
            &spool,
            &link,
            Path::new(target),
            Path::new("/proc"),
            &["ori-original"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_a_preserved_link_passes_a_second_install() {
        // A link that the first install kept, re-pointed one level down to the
        // same target, was judged where it stood, and is judged again.
        let scratch = Scratch::new("kept-link-again");
        let (repo, spool) = skeleton(&scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let script = repo.join(".git").join("shared-hook.sh");
        fs::write(&script, "#!/bin/sh\necho shared\n").unwrap();
        std::os::unix::fs::symlink("../shared-hook.sh", hooks_dir.join("post-commit")).unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let kept = preserved(&repo, GitHook::PostCommit);
        assert_eq!(
            fs::read_link(&kept).unwrap(),
            Path::new("../../shared-hook.sh")
        );
        let names = hook_dir_names(&repo);
        let installed: Vec<Vec<u8>> = GitHook::ALL
            .iter()
            .map(|hook| fs::read(hook_path(&repo, *hook)).unwrap())
            .collect();

        let report = hooks.install().unwrap();
        assert!(report.is_unchanged(), "{report:?}");
        assert_eq!(hook_dir_names(&repo), names);
        assert_eq!(
            fs::read_link(&kept).unwrap(),
            Path::new("../../shared-hook.sh")
        );
        for (hook, before) in GitHook::ALL.iter().zip(&installed) {
            assert_eq!(&fs::read(hook_path(&repo, *hook)).unwrap(), before);
        }
        assert_eq!(
            fs::read_to_string(&kept).unwrap(),
            "#!/bin/sh\necho shared\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0168_status_refuses_an_ori_original_made_a_link_after_an_install() {
        let scratch = Scratch::new("status-ori-original");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        assert!(
            hooks
                .status()
                .unwrap()
                .iter()
                .all(|status| status.state == HookState::Installed)
        );
        let preserved_dir = repo.join(".git").join("hooks").join("ori-original");
        std::os::unix::fs::symlink(".", &preserved_dir).unwrap();

        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, &preserved_dir),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    // ---- ORI-T-0185: status judges the links as an install does ----

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_status_refuses_a_link_added_after_an_install() {
        let scratch = Scratch::new("status-link-after");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        assert!(
            hooks
                .status()
                .unwrap()
                .iter()
                .all(|status| status.state == HookState::Installed)
        );
        // A link in the hooks directory, made after the install: every
        // checkout would run the engine's `post-merge` script in place of the
        // hook the link stood for.
        let link = repo.join(".git").join("hooks").join("post-checkout");
        std::os::unix::fs::symlink("post-merge", &link).unwrap();
        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HookLinksToManagedHook { path, target } => {
                assert_eq!(path, &link);
                assert_eq!(target, Path::new("post-merge"));
            }
            other => panic!("expected HookLinksToManagedHook, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);

        // A link in `ori-original/` named for a hook the installer replaces,
        // made after the install: the script would run the engine's own.
        // `pre-push` and not the kept `post-commit`, so no kept hook is lost.
        let scratch = Scratch::new("status-link-in-ori-original-after");
        let (repo, spool, hooks_dir, _) = repository_with_a_users_post_commit(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        assert!(fs::symlink_metadata(preserved(&repo, GitHook::PostCommit)).is_ok());
        assert!(hooks.status().is_ok());
        let entry = hooks_dir.join(PRESERVED_DIR).join("pre-push");
        std::os::unix::fs::symlink("../post-merge", &entry).unwrap();
        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HookLinksToManagedHook { path, target } => {
                assert_eq!(path, &entry);
                assert_eq!(target, Path::new("../post-merge"));
            }
            other => panic!("expected HookLinksToManagedHook, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_status_refuses_a_link_through_a_per_process_directory() {
        let scratch = Scratch::new("status-per-process");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let link = repo.join(".git").join("hooks").join("post-checkout");
        let target = Path::new("/proc/self/cwd/.git/hooks/post-merge");
        std::os::unix::fs::symlink(target, &link).unwrap();
        // Matched here and not through `expect_refused_for_the_process`, which
        // is Linux-only and calls `install`. Where `/proc` is not there the
        // walk still stops on its name.
        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HookLinkDependsOnProcess {
                path,
                through,
                target: seen,
            } => {
                assert_eq!(path, &link);
                assert_eq!(seen, target);
                assert_eq!(through, Path::new("/proc"));
            }
            other => panic!("expected HookLinkDependsOnProcess, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_status_refuses_a_hooks_directory_that_is_a_link_or_a_file() {
        let scratch = Scratch::new("status-hooks-dir-link");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let hooks_dir = repo.join(".git").join("hooks");
        let aside = repo.join(".git").join("hooks-aside");
        fs::rename(&hooks_dir, &aside).unwrap();
        std::os::unix::fs::symlink("hooks-aside", &hooks_dir).unwrap();
        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, &hooks_dir),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);

        let scratch = Scratch::new("status-hooks-dir-file");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let hooks_dir = repo.join(".git").join("hooks");
        fs::rename(&hooks_dir, repo.join(".git").join("hooks-aside")).unwrap();
        fs::write(&hooks_dir, "not a directory\n").unwrap();
        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, &hooks_dir),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_status_accepts_a_link_a_first_install_kept() {
        // A user's `pre-push -> ../shared-hook.sh`, kept by the first install
        // as a link one level down, was judged where it stood and is judged
        // again by `status`, which must not refuse what `install` kept.
        let scratch = Scratch::new("status-kept-link");
        let (repo, spool) = skeleton(&scratch);
        let target = repo.join(".git").join("shared-hook.sh");
        fs::write(&target, "#!/bin/sh\necho shared\n").unwrap();
        let path = hook_path(&repo, GitHook::PrePush);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("../shared-hook.sh", &path).unwrap();
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();

        let statuses = hooks.status().expect("status accepts what install kept");
        let pre_push = statuses
            .iter()
            .find(|status| status.hook == GitHook::PrePush)
            .unwrap();
        assert_eq!(pre_push.state, HookState::Installed);
        assert!(pre_push.chains_to_original);
        assert!(
            statuses
                .iter()
                .filter(|status| status.hook != GitHook::PrePush)
                .all(|status| !status.chains_to_original)
        );
    }

    // ---- ORI-T-0185: a way past a missing directory is read physically ----

    /// A repository skeleton with the directories these tests link through:
    /// `.git/hooks` and `.git/refs`.
    #[cfg(unix)]
    fn repository_with_hooks_and_refs(scratch: &Scratch) -> (PathBuf, PathBuf, PathBuf) {
        let (repo, spool) = skeleton(scratch);
        let hooks_dir = repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        fs::create_dir_all(repo.join(".git").join("refs")).unwrap();
        (repo, spool, hooks_dir)
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_a_way_past_a_directory_git_makes_later_is_read_physically() {
        // Layout (ii). `L -> ../refs`, and `refs/remotes` is not there until
        // the first `git fetch`. Read by its text, `L/remotes/../../hooks`
        // lands on `hooks/hooks`; read as git's process will find it once
        // `refs/remotes` exists, `L` is `.git/refs`, the first `..` is
        // `.git/refs` again, the second is `.git`, and `hooks` from there is
        // the hooks directory.
        let scratch = Scratch::new("physical-ii");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        let written = "L/remotes/../../hooks/post-merge";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["L", "post-checkout"],
        );

        // Layout (i). `.git/y -> objects/info`, and `.git/logs` is not there
        // until the first commit. The text of the way leaves `.git` for the
        // working tree; the physical reading climbs out of `objects/info` to
        // `.git`, and then into the hooks directory.
        let scratch = Scratch::new("physical-i");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        fs::create_dir_all(repo.join(".git").join("objects").join("info")).unwrap();
        std::os::unix::fs::symlink("objects/info", repo.join(".git").join("y")).unwrap();
        let written = "../logs/../y/../../hooks/post-merge";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        assert!(!repo.join(".git").join("logs").exists());
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["post-checkout"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_a_way_past_a_missing_directory_goes_on_from_its_physical_landing() {
        // The text of `L/remotes/../../relay` lands on `hooks/relay`, which is
        // not there; the physical landing is `.git`, which holds the relay.
        let scratch = Scratch::new("physical-landing");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        std::os::unix::fs::symlink("hooks/post-merge", repo.join(".git").join("relay")).unwrap();
        assert!(!hooks_dir.join("relay").exists());
        let written = "L/remotes/../../relay";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["L", "post-checkout"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_a_physical_reading_that_meets_a_loop_refuses_as_an_io_error() {
        // The text of the way lands on `hooks/refs/loop`, which is not there,
        // so only the physical reading meets the loop at `.git/refs/loop`.
        let scratch = Scratch::new("physical-loop");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        std::os::unix::fs::symlink("loop", repo.join(".git").join("refs").join("loop")).unwrap();
        let link = hooks_dir.join("post-checkout");
        std::os::unix::fs::symlink("L/remotes/../../refs/loop/post-merge", &link).unwrap();
        assert!(!hooks_dir.join("refs").exists());

        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::Io { path, source } => {
                assert_eq!(path, &link);
                assert!(
                    source.to_string().contains("links in the way"),
                    "the error does not name the cap: {source}"
                );
            }
            other => panic!("expected an IO error naming the link, got {other}"),
        }
        assert_nothing_written(&repo, &spool, &["L", "post-checkout"]);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_the_chain_check_reads_layout_ii_physically() {
        let scratch = Scratch::new("physical-unit");
        let (_, _, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        let written = "L/remotes/../../hooks/post-merge";
        let link = hooks_dir.join("post-checkout");
        std::os::unix::fs::symlink(written, &link).unwrap();
        let canonical = fs::canonicalize(&hooks_dir).unwrap();
        let reached = follow_chain_to_managed_hook(&hooks_dir, &canonical, &link, false).unwrap();
        assert_eq!(reached, Some(PathBuf::from(written)));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_the_text_reading_still_refuses_a_landing_on_the_hooks_directory() {
        // The physical landing of `L/../nowhere/..` is `.git`, which holds no
        // hook; its text lands on `.git/hooks`.
        let scratch = Scratch::new("text-second-look");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        let written = "L/../nowhere/../post-merge";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["L", "post-checkout"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_a_way_whose_physical_reading_ends_past_a_missing_name_goes_on_from_its_text() {
        // The physical reading of `L/../sub` ends at `.git/sub`, which is not
        // there; the text lands on `.git/hooks/sub`, which holds the relay.
        let scratch = Scratch::new("text-landing");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        fs::create_dir_all(hooks_dir.join("sub")).unwrap();
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        std::os::unix::fs::symlink("../post-merge", hooks_dir.join("sub").join("relay")).unwrap();
        assert!(!repo.join(".git").join("sub").exists());
        let written = "L/../sub/relay";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["L", "post-checkout", "sub"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_a_text_landing_on_a_loop_refuses_as_an_io_error() {
        // The physical reading of `L/../nowhere/../loopdir` climbs back from
        // the missing `.git/nowhere` and ends at `.git/loopdir`, which is not
        // there. The text lands on `hooks/loopdir`, a loop: the text reading's
        // own error path. The link `loopdir` itself passes the scan, like the
        // other kept loop links.
        let scratch = Scratch::new("text-loop");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        std::os::unix::fs::symlink("loopdir", hooks_dir.join("loopdir")).unwrap();
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        let link = hooks_dir.join("post-checkout");
        std::os::unix::fs::symlink("L/../nowhere/../loopdir/x", &link).unwrap();
        assert!(!repo.join(".git").join("loopdir").exists());

        let error = GitHooks::new(&repo, &spool).unwrap().install().unwrap_err();
        match &error {
            HooksError::Io { path, .. } => assert_eq!(path, &link),
            other => panic!("expected an IO error naming the link, got {other}"),
        }
        assert_nothing_written(&repo, &spool, &["L", "loopdir", "post-checkout"]);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_status_checks_ori_original_before_the_link_scan() {
        // `ori-original` is a link named for a hook. The scan of the links would
        // meet it first and refuse it as a link to a managed hook; `status`
        // judges in the order `install` does, `ori-original` first, so what it
        // says is that `ori-original` is not a directory.
        let scratch = Scratch::new("status-order");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let preserved_dir = repo.join(".git").join("hooks").join(PRESERVED_DIR);
        assert!(
            fs::symlink_metadata(&preserved_dir).is_err(),
            "a clean install with no hook of the user's makes no `ori-original`"
        );
        std::os::unix::fs::symlink("post-merge", &preserved_dir).unwrap();

        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, &preserved_dir),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_the_physical_landing_is_checked_for_a_hook_when_the_text_lands_on_a_directory_too()
     {
        // `L/remotes/../../hooks`, as git's process will find it once
        // `refs/remotes` exists, lands on `.git/hooks`. Its text lands on
        // `hooks/hooks`, which is there and holds no hook, so the second look
        // finds nothing and only the physical landing can refuse the link.
        let scratch = Scratch::new("physical-text-directory");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        fs::create_dir_all(hooks_dir.join("hooks")).unwrap();
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        let written = "L/remotes/../../hooks/post-merge";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        assert!(!repo.join(".git").join("refs").join("remotes").exists());
        assert!(!hooks_dir.join("post-merge").exists());
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["L", "hooks", "post-checkout"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0185_a_file_at_the_physical_landing_falls_back_to_the_text() {
        // The physical reading of `L/../nowhere/../sub` ends at `.git/sub`,
        // which is a regular file at the last step, so it reaches no directory.
        // The text lands on `hooks/sub`, which holds the relay to a hook.
        let scratch = Scratch::new("physical-file-landing");
        let (repo, spool, hooks_dir) = repository_with_hooks_and_refs(&scratch);
        fs::write(repo.join(".git").join("sub"), "a file\n").unwrap();
        fs::create_dir_all(hooks_dir.join("sub")).unwrap();
        std::os::unix::fs::symlink("../refs", hooks_dir.join("L")).unwrap();
        std::os::unix::fs::symlink("../post-merge", hooks_dir.join("sub").join("relay")).unwrap();
        let written = "L/../nowhere/../sub/relay";
        std::os::unix::fs::symlink(written, hooks_dir.join("post-checkout")).unwrap();
        expect_refused_link(
            &repo,
            &spool,
            &hooks_dir.join("post-checkout"),
            Path::new(written),
            &["L", "post-checkout", "sub"],
        );
    }

    #[test]
    fn ori_t_0185_status_refuses_a_hooks_directory_that_is_a_file() {
        // No link is made, so this runs where links cannot be made too.
        let scratch = Scratch::new("status-hooks-dir-is-a-file");
        let (repo, spool) = skeleton(&scratch);
        let hooks = GitHooks::new(&repo, &spool).unwrap();
        hooks.install().unwrap();
        let hooks_dir = repo.join(".git").join("hooks");
        fs::rename(&hooks_dir, repo.join(".git").join("hooks-aside")).unwrap();
        fs::write(&hooks_dir, "not a directory\n").unwrap();

        let error = hooks.status().unwrap_err();
        match &error {
            HooksError::HooksDirNotADirectory { path } => assert_eq!(path, &hooks_dir),
            other => panic!("expected HooksDirNotADirectory, got {other}"),
        }
        assert_eq!(error.methodology_ref().unwrap().section, 13);
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

        /// Spawns `command`, again while the kernel refuses to run a script
        /// that is "busy". Another test's `fork` can hold the write handle of a
        /// script this test wrote a moment ago until that child's own `exec`,
        /// and the kernel then refuses to run the script. The refusal passes in
        /// milliseconds.
        fn spawn_when_not_busy(command: &mut Command) -> std::process::Child {
            let mut busy = 0;
            loop {
                match command.spawn() {
                    Ok(child) => return child,
                    Err(error)
                        if error.kind() == io::ErrorKind::ExecutableFileBusy && busy < 100 =>
                    {
                        busy += 1;
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(error) => panic!("the command could not be run: {error}"),
                }
            }
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
        fn ori_t_0054_a_commit_in_a_linked_worktree_reaches_the_main_repositorys_hooks_and_refs() {
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

        /// A hook manager that decides what to do from the name it was run
        /// under (`basename "$0"`), as the reviewer's probe does.
        const DISPATCHES_ON_ITS_NAME: &str = "#!/bin/sh
hookName=$(basename \"$0\")
case \"$hookName\" in
  pre-push) echo \"manager: pre-push checks failed, refusing the push\" >&2; exit 1 ;;
  *) echo \"manager: no hook configured for '$hookName', nothing to run\" >&2; exit 0 ;;
esac
";

        #[test]
        fn ori_t_0054_a_preserved_hook_that_dispatches_on_its_own_name_still_runs_and_still_vetoes()
        {
            let scratch = Scratch::new("basename");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            let hooks_dir = repo.join(".git").join("hooks");
            fs::create_dir_all(&hooks_dir).unwrap();
            for name in ["pre-push", "post-commit"] {
                let path = hooks_dir.join(name);
                fs::write(&path, DISPATCHES_ON_ITS_NAME).unwrap();
                make_executable(&path).unwrap();
            }
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();

            // post-commit: the preserved hook must be told it is post-commit.
            let commit = git(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            let stderr = String::from_utf8_lossy(&commit.stderr).into_owned();
            assert!(commit.status.success(), "{stderr}");
            assert!(
                stderr.contains("no hook configured for 'post-commit'"),
                "the preserved hook ran under another name: {stderr:?}"
            );

            // pre-push: its veto must still stop the push.
            let bare = scratch.dir("bare.git");
            git_ok(&scratch, &bare, &["init", "-q", "--bare"]);
            git_ok(
                &scratch,
                &repo,
                &["remote", "add", "origin", bare.to_str().unwrap()],
            );
            let push = git(&scratch, &repo, &["push", "-q", "origin", "main"]);
            let stderr = String::from_utf8_lossy(&push.stderr).into_owned();
            assert!(
                !push.status.success(),
                "the preserved veto was dropped: {stderr:?}"
            );
            assert!(stderr.contains("pre-push checks failed"), "{stderr:?}");
            // Ours recorded the push attempt before handing over.
            assert!(log_of(&spool).contains("pre-push"));
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

        #[test]
        fn ori_t_0185_a_checkout_that_makes_a_kept_link_reach_the_engine_records_nothing() {
            // Layout B. On `main`, `d` does not exist, so `post-checkout ->
            // ../../d/post-merge` reaches nothing and the install keeps it. On
            // `other` the tree has `d -> .git/hooks`, and the same link then
            // reaches the engine's own `post-merge`: what a checkout will put
            // at a path cannot be known at the install, so the script checks
            // the name it was run under.
            let scratch = Scratch::new("checkout-layout-b");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            git_ok(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            git_ok(&scratch, &repo, &["checkout", "-q", "-b", "other"]);
            std::os::unix::fs::symlink(".git/hooks", repo.join("d")).unwrap();
            git_ok(&scratch, &repo, &["add", "d"]);
            git_ok(
                &scratch,
                &repo,
                &["commit", "-q", "-m", "a link to the hooks"],
            );
            git_ok(&scratch, &repo, &["checkout", "-q", "main"]);
            assert!(
                fs::symlink_metadata(repo.join("d")).is_err(),
                "`d` is still in the tree on main"
            );
            let hooks_dir = repo.join(".git").join("hooks");
            fs::create_dir_all(&hooks_dir).unwrap();
            std::os::unix::fs::symlink("../../d/post-merge", hooks_dir.join("post-checkout"))
                .unwrap();

            GitHooks::new(&repo, &spool)
                .unwrap()
                .install()
                .expect("this ticket does not refuse the layout at the install");

            let output = git(&scratch, &repo, &["checkout", "other"]);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            assert!(output.status.success(), "the checkout failed: {stderr}");
            assert!(
                fs::symlink_metadata(repo.join("d")).is_ok(),
                "the checkout did not make `d`, so the test proves nothing"
            );
            assert!(
                !log_of(&spool).contains("\tpost-merge\t"),
                "a checkout recorded a false post-merge: {:?}",
                log_of(&spool)
            );
            assert!(
                stderr.contains("not by git for its own event"),
                "the script did not say it was not run for its own event: {stderr:?}"
            );
        }

        #[test]
        fn ori_t_0185_a_script_run_from_ori_original_records_once() {
            let scratch = Scratch::new("run-from-ori-original");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            // `ori-original/post-commit` links to the engine's own script: run
            // by the script that just recorded, it is the same script a second
            // time, under its own name but from `ori-original/`.
            let kept = repo.join(".git").join("hooks").join(PRESERVED_DIR);
            fs::create_dir_all(&kept).unwrap();
            std::os::unix::fs::symlink("../post-commit", kept.join("post-commit")).unwrap();

            let output = git(&scratch, &repo, &["commit", "-q", "-m", "first"]);
            assert!(
                output.status.success(),
                "the commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let log = log_of(&spool);
            assert_eq!(log.lines().count(), 1, "{log:?}");
            assert!(log.contains("\tpost-commit\t"), "{log:?}");
        }

        #[test]
        fn ori_t_0185_a_script_run_under_another_name_reads_all_its_input_and_exits_zero() {
            let scratch = Scratch::new("another-name");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            let hooks_dir = repo.join(".git").join("hooks");
            let link = hooks_dir.join("not-a-hook");
            // 256 lines of 1024 bytes: more than a pipe holds, so a script that
            // exits without reading would close the pipe on the writer.
            let line = format!("{}\n", "x".repeat(1023));
            for hook in ["pre-push", "post-rewrite"] {
                let _ = fs::remove_file(&link);
                std::os::unix::fs::symlink(hook, &link).unwrap();
                let mut command = Command::new(&link);
                command
                    .arg("origin")
                    .current_dir(&repo)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child = spawn_when_not_busy(&mut command);
                let mut input = child.stdin.take().unwrap();
                for _ in 0..256 {
                    input
                        .write_all(line.as_bytes())
                        .unwrap_or_else(|error| panic!("{hook}: the writer was cut off: {error}"));
                }
                drop(input);
                let output = child.wait_with_output().unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                assert_eq!(output.status.code(), Some(0), "{hook}: {stderr}");
                assert_eq!(log_of(&spool), "", "{hook}: a record was written");
                assert!(
                    stderr.contains("not by git for its own event"),
                    "{hook}: {stderr:?}"
                );
                // The name and the directory are printed apart, so a long
                // temporary path cannot hide the name the script was run as.
                assert!(
                    stderr.contains("was run as not-a-hook in ")
                        && stderr.contains(&format!("the {hook} hook")),
                    "{hook}: {stderr:?}"
                );
            }
        }

        #[test]
        fn ori_t_0185_a_guarded_run_runs_no_kept_hook_and_no_git_and_prints_a_clean_name() {
            // The guard promises that a script run under another name records
            // nothing and runs nothing: not the hook kept in `ori-original/`,
            // which would run with another event's arguments, and not `git`,
            // since the guard comes before any git call. Each is a marker file
            // that only a run writes. A control first runs each script under
            // its own name with the same `PATH`, so a marker that never shows
            // up is not one the test could not have seen.
            let scratch = Scratch::new("guarded-run");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            let hooks_dir = repo.join(".git").join("hooks");
            let kept_dir = hooks_dir.join(PRESERVED_DIR);
            fs::create_dir_all(&kept_dir).unwrap();

            let marker_dir = scratch.dir("markers");
            let marker = |name: &str| marker_dir.join(name);
            for hook in ["pre-push", "post-rewrite"] {
                let kept = kept_dir.join(hook);
                fs::write(
                    &kept,
                    format!("#!/bin/sh\n: > '{}'\n", marker(hook).display()),
                )
                .unwrap();
                make_executable(&kept).unwrap();
            }
            // A `git` that only says it was run, found before the real one.
            let shim_dir = scratch.dir("shim");
            let shim = shim_dir.join("git");
            fs::write(
                &shim,
                format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker("git").display()),
            )
            .unwrap();
            make_executable(&shim).unwrap();
            let path = std::env::join_paths(std::iter::once(shim_dir.clone()).chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
            ))
            .unwrap();

            let run = |script: &Path| -> Output {
                let mut command = Command::new(script);
                command
                    .arg("origin")
                    .current_dir(&repo)
                    .env("PATH", &path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child = spawn_when_not_busy(&mut command);
                let mut input = child.stdin.take().unwrap();
                // A few lines. A writer that is cut off is the business of the
                // test that sends more than a pipe holds, so a failure to write
                // is not this test's.
                for _ in 0..3 {
                    let _ = input.write_all(b"refs/heads/main aaaa refs/heads/main bbbb\n");
                }
                drop(input);
                child.wait_with_output().unwrap()
            };

            // The control: each script under its own name runs the kept hook and
            // `git`, and records.
            for hook in ["pre-push", "post-rewrite"] {
                let output = run(&hooks_dir.join(hook));
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                assert!(
                    marker(hook).exists(),
                    "control: the kept {hook} did not run under its own name: {stderr:?}"
                );
                assert!(
                    marker("git").exists(),
                    "control: no git ran under {hook}'s own name: {stderr:?}"
                );
                fs::remove_file(marker(hook)).unwrap();
                fs::remove_file(marker("git")).unwrap();
            }
            let before = log_of(&spool);
            assert!(!before.is_empty(), "control: nothing was recorded");

            // A link to `pre-push` whose name holds a space and an ESC byte, and
            // a link to `post-rewrite` under a plain name.
            let cases = [
                ("pre-push", "not a hook\x1b", "was run as not?a?hook? in "),
                ("post-rewrite", "not-a-hook", "was run as not-a-hook in "),
            ];
            for (hook, name, shown) in cases {
                let link = hooks_dir.join(name);
                std::os::unix::fs::symlink(hook, &link).unwrap();
                let output = run(&link);
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                assert_eq!(output.status.code(), Some(0), "{hook}: {stderr:?}");
                for ran in ["pre-push", "post-rewrite", "git"] {
                    assert!(
                        !marker(ran).exists(),
                        "{hook} as {name:?}: {ran} was run: {stderr:?}"
                    );
                }
                assert!(stderr.contains(shown), "{hook} as {name:?}: {stderr:?}");
                assert!(
                    !output.stderr.contains(&0x1b),
                    "{hook} as {name:?}: an ESC byte reached standard error: {stderr:?}"
                );
                assert_eq!(
                    log_of(&spool),
                    before,
                    "{hook} as {name:?}: a record was written"
                );
                fs::remove_file(&link).unwrap();
            }
        }

        #[test]
        fn ori_t_0185_the_script_records_under_its_own_name() {
            // The other side of the guard, through the same `Command` path as
            // the test above: run directly as `post-merge`, the script records.
            let scratch = Scratch::new("own-name");
            let repo = real_repo(&scratch);
            let spool = scratch.path.join("spool");
            GitHooks::new(&repo, &spool).unwrap().install().unwrap();
            let output = Command::new(hook_path(&repo, GitHook::PostMerge))
                .arg("0")
                .current_dir(&repo)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let drain = HookSpool::open(&spool).drain().unwrap();
            assert_eq!(drain.malformed, 0);
            assert_eq!(drain.events.len(), 1, "{:?}", log_of(&spool));
            assert_eq!(drain.events[0].hook, GitHook::PostMerge);
            assert_eq!(drain.events[0].fields[0], "0");
        }
    }
}
