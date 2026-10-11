//! The tree watcher: AICD §13, AICD §23.
//!
//! [`TreeWatcher`] watches one directory tree, a product repository's root or
//! one session's worktree, and reports what changed in it as typed values:
//! the path relative to that root, the kind of change, and the time it was
//! observed ([`Change`]). `spec/ARCHITECTURE.md` section 5.2 gives the
//! consumer: each change is matched against active sessions, and one that
//! matches none is an unattributed change. Matching is `Attribution`'s job
//! (ORI-T-0055); this module only has to deliver a change, completely and
//! fast. [`RefPoller`] is the other half of section 5.2's first sentence,
//! "plus polling of refs".
//!
//! ```mermaid
//! flowchart LR
//!   FS[file system] --> N[notify backend thread]
//!   N -->|translate, stamp| Q[bounded queue]
//!   Q --> B[next_batch: confine, ignore, coalesce]
//!   B --> C[Batch of Change]
//!   N -.->|overflow, error| M[MissedChanges]
//!   Q -.->|full| M
//!   M --> C
//!   G[git refs on disk] --> R[RefPoller] --> RC[RefChange]
//! ```
//!
//! # Which session a change belongs to is not decided here
//!
//! The watcher reports every change under its root, `spec/` included, and
//! knows nothing of sessions, tickets or identities. `spec/ARCHITECTURE.md`
//! section 5.2 puts the matching (against active sessions, and against human
//! edits in `spec/` made through Ori Studio) in `Attribution`. A session's
//! worktree is a directory like any other: the engine runs one [`TreeWatcher`]
//! on the product root and, to attribute a change inside a session's worktree
//! to that session (criterion ORI-P1-015), one on each worktree, or hands the
//! batch to attribution with the root it came from. The tests here show a
//! change inside a worktree is *delivered*, with a path relative to it; none of
//! them shows it is *attributed* (ORI-P1-015 asks for "attributed to the
//! session; no incident"), and ORI-P1-015 is not claimed as covered until
//! ORI-T-0055's attribution has its own tests.
//!
//! # This module never writes
//!
//! `spec/LLD.md` section 2 lists `ori-watch`'s "Must not" as "Modify the
//! tree". Nothing here opens a file for writing or creates, renames or deletes
//! anything. It reads directory entries and file metadata, never a file's
//! content from the working tree, and reads `.git/` only in [`RefPoller`]
//! (small files, bounded).
//!
//! # Confinement: a path outside the root is never reported or read
//!
//! A hostile or merely careless repository can hold a symbolic link that
//! points out of itself. Following one would make the watcher report, as
//! "inside the product", a change to a file that is not in it. The rule is the
//! one `crates/ori-memory/src/indexer.rs` and `code_map.rs` already follow
//! (they decide every entry by `DirEntry::file_type`, which never follows a
//! link): **a link is an entry, never a road.** Three layers enforce it:
//!
//! 1. The root is canonicalised once, when the watch starts, and handed to the
//!    backend with `Config::with_follow_symlinks(false)`, so the backend does
//!    not register watches through a link (notify's default is to follow).
//! 2. Every event path must lie lexically under the root and its remainder
//!    must consist only of normal components (`relative_inside`): a `..`, a
//!    root, a drive prefix or a path outside the root is refused, not
//!    trimmed. `strip_prefix` is lexical, which is why layer 3 exists.
//! 3. Every proper ancestor of a reported path, below the root, is looked at
//!    with `symlink_metadata` (never following) and must be a real directory.
//!    A path that goes through a link is counted in [`Batch::rejected`] and
//!    dropped. The last component may itself be a link: a link created or
//!    retargeted inside the root is a change to the tree (git tracks links),
//!    and it is reported as that entry, never resolved. On Windows `std`
//!    reports a junction as a link too, which `code_map.rs` documents.
//!
//! 4. A directory that appears is listed by the new-directory scan (below)
//!    only after it, and every directory above it, has been looked at again,
//!    without following, immediately before it is listed; a directory that has
//!    become a link in the meantime is counted in [`Batch::rejected`] and not
//!    listed.
//!
//! **What this does not claim.** Each check looks at one instant, and `std`
//! has no `openat` to hold a directory between a check and its use. A process
//! that can write inside the root and races the watcher can swap a directory
//! for a link between the last look and the listing of a new directory (or
//! between an event and the ancestor check). The scan then lists the link's
//! target and reports the names in it as `Created`, under an in-root path.
//! Nothing is opened and no file content is read, so nothing leaves the
//! target but names, and the names are not attributed to anything. This is
//! the threat model `code_map.rs` documents in its module doc ("Threat
//! model"): a hostile repository that holds still is in scope, and a live
//! writer racing the walk is not claimed. Neither this module nor the
//! backend opens a reported path or reads a file's content from the tree.
//!
//! # What is ignored, and why
//!
//! - **`.git`, at any depth, in any letter case.** The repository's own
//!   metadata is not an edit to the tree: git does not track it, `git status`
//!   never lists it, and git rewrites it on every command. Reporting it would
//!   make every commit an "edit" and bury real ones. Any case, because on a
//!   case-insensitive file system (macOS and Windows by default) `.GIT` is
//!   the same directory (the reasoning `code_map.rs` records). A `.git` entry
//!   that is a file (a linked worktree's pointer) is ignored the same way.
//!   Commits, merges and pushes reach the engine through `GitHooks` and
//!   [`RefPoller`] instead; that is the reading `crate::hooks` states.
//! - **Build output**: `node_modules` at any depth and `target` at the root
//!   only ([`IgnoreRule`], [`WatchConfig::default`]). They are regenerated by
//!   a build, are ignored by git in this product's own layout, and a build
//!   writes thousands of files in them. `target` is root-only because
//!   elsewhere the name is ordinary (a Maven module's `target/` is not this
//!   product's build output, and a source directory named `target` must not be
//!   a blind spot). **This is a blind spot by design**: a change inside an
//!   ignored directory is never reported, so an engine whose product tracks
//!   such a directory in git passes its own rules through
//!   [`TreeWatcher::watch_with`].
//! - **Directories.** A directory is not a change git can see; the files in it
//!   are. A directory that appears is scanned for the files already in it
//!   (below), and an event on a directory itself is dropped.
//!
//! # How a burst is coalesced
//!
//! An editor's save is several events (create a temporary, write, rename it
//! over the target), and a build touches one file many times. [`TreeWatcher::next_batch`]
//! waits for a first event, then keeps collecting until the tree has been quiet
//! for [`WatchConfig::quiet`] or [`WatchConfig::max_hold`] has passed since
//! the first event, whichever is first, so a steady stream is still delivered
//! at least every `max_hold`. Within a batch each path is reported once:
//! the kinds fold by `merge_kinds`, created then modified is created, modified
//! then removed is removed, removed then created is modified (a replace). A
//! path created and removed inside one window nets to nothing and is counted
//! in [`Batch::transient`], never dropped without a trace. `kind` is the best
//! classification the operating system gave: inotify cannot tell an atomic
//! replace from a create, so a replaced file may read as created.
//!
//! The latency this adds is the quiet window plus the operating system's own
//! delivery time; the measured figure is in the pull request report and
//! asserted by `tests::ori_t_0054_the_latency_from_a_write_to_a_reported_change_is_bounded`.
//!
//! A directory that appears is scanned (bounded, links not followed) for the
//! files already in it, because the backend registers its watch a moment after
//! the directory is created, and a file written in that moment would
//! otherwise never be reported. The scan runs after a short settle delay so
//! the watch is in place.
//!
//! # A dropped event is a recorded state, never silence (AICD §14)
//!
//! A watcher that has lost events and says nothing is the defect AICD §14
//! names "present but reporting nothing". Four things make the watcher lose
//! events, and each one sets [`MissedChanges`], which is delivered in the next
//! [`Batch`] and held by [`TreeWatcher::health`] until the engine reconciles:
//!
//! - the backend reports an overflow or a rescan notice
//!   ([`MissedReason::Overflow`]), such as an inotify queue overflow;
//! - the backend reports an error ([`MissedReason::BackendError`]);
//! - this module's own bounded queue filled because the consumer did not drain
//!   it ([`MissedReason::QueueFull`]), counted at the producer so it cannot be
//!   lost;
//! - the root itself was removed ([`MissedReason::RootGone`]), or a directory
//!   scan ran out of budget ([`MissedReason::ScanBudget`]).
//!
//! Reconciling means finding out what changed since: a full comparison of the
//! tree against git is the engine's to run (it spawns processes; this crate
//! does not). The engine drains with [`TreeWatcher::next_batch`], takes a
//! [`ReconcileToken`] from [`TreeWatcher::begin_reconcile`], compares, and
//! hands the token to [`TreeWatcher::mark_reconciled`], which clears the state
//! only if nothing was recorded after the token was taken: a loss during the
//! comparison is not covered by it.
//!
//! **A backend that stops is reported.** When the backend's thread ends (notify
//! panics in it, for one), the queue disconnects, [`TreeWatcher::next_batch`]
//! returns [`WatchError::Stopped`] and [`TreeWatcher::health`] reads
//! [`Health::Stopped`]; a stopped watcher is replaced, not reconciled. That
//! holds only because the watcher keeps no sender of its own in a production
//! build; a held sender would keep the queue connected for ever and a dead
//! backend would read as a quiet tree. Dropping a watcher whose backend is dead
//! cannot panic the caller either: notify's drop `unwrap`s a send to the dead
//! thread, and [`TreeWatcher`]'s own drop catches that panic (the panic hook
//! still prints it). That holds under `panic = "unwind"`, the default, which
//! this workspace does not change.
//!
//! # What `Health::Watching` does not vouch for
//!
//! The states above are the losses this module can see. These notify does not
//! report, so none of them sets [`MissedChanges`]; treat [`Health::Watching`]
//! as "the backend reported nothing wrong", and keep the periodic comparison
//! of the tree with git as the check that does not depend on the backend:
//!
//! - On Linux, the walk that registers the initial watches skips a directory
//!   it cannot read, with no error (`filter_dir` in notify's `inotify.rs`).
//!   A directory unreadable at start is never watched, and changes in it are
//!   never seen.
//! - On Linux, when a directory is created later, adding the watch for it
//!   drops every error but the watch limit (the same file, where it handles
//!   the events it read). A directory whose watch fails is silently unwatched
//!   from then on; this module's scan lists what is in it once, not after.
//! - On Windows, notify does not flag a `ReadDirectoryChangesW` buffer
//!   overflow, and a directory whose watch hits an unidentified error is
//!   unwatched and only logged (`windows.rs`, `handle_event`).
//! - A file created in a new directory after this module's scan of it and
//!   before the backend's watch on it is registered. The settle delay before
//!   the scan narrows that window; it does not close it.
//!
//! # Blocking calls
//!
//! [`TreeWatcher::next_batch`] and [`RefPoller::poll`] block the calling
//! thread (the first for up to its `wait` plus the batch window, and a little
//! longer when it scans a new directory). The engine runs them with
//! `spawn_blocking` (`spec/CONVENTIONS.md`, "Rust"); nothing here needs a
//! runtime.
//!
//! # Polling refs
//!
//! [`RefPoller`] reads `.git/HEAD`, `.git/packed-refs` and `.git/refs/**`
//! directly and reports refs that appeared, moved or vanished since the last
//! poll. It spawns no process (only `ori-runtime` does) and follows no link:
//! `HEAD`, `packed-refs` and every ref file are read as plain files
//! (`open_plain_file`: looked at without following, opened with `O_NOFOLLOW`
//! and `O_NONBLOCK` where those are verified, and the opened handle checked),
//! so a `.git/HEAD` that is a link, or a pipe, cannot turn another file's first
//! line into an event or make the poll wait; `.git/refs` must be a real
//! directory and is walked by `DirEntry::file_type`. A `HEAD` or `refs/` that
//! is not plain is refused ([`WatchError::RefStorageNotPlain`]); a ref file
//! inside `refs/` that is a link or a pipe is skipped and listed in
//! [`RefPoll::skipped`]. The same limit as above holds: a directory swapped
//! for a link by a live writer between the listing and the read is not
//! claimed against.
//!
//! The loose refs are read before `packed-refs`. `git pack-refs` writes the
//! new `packed-refs` first and prunes the loose files after, so the other order
//! can read the old packed file and the pruned directory and see a branch that
//! never moved deleted, then created.
//! It is the detector that does not depend on a hook: it sees a commit made
//! with `--no-verify`, a hook that was removed, and a push (git updates
//! `refs/remotes/<remote>/<branch>` after a successful one). The engine calls
//! [`RefPoller::poll`] on its tick; this module owns no timer.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(test)]
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TrySendError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ori_core::error::MethodologyRef;
use ori_core::types::Timestamp;

/// How long the tree must be quiet before a batch is delivered: AICD §23.
///
/// A save is several events; waiting this long folds them into one change.
/// It is also the floor of the latency this module adds to criterion
/// ORI-P1-014's two seconds.
pub const DEFAULT_QUIET: Duration = Duration::from_millis(100);

/// The longest a batch is held once its first event arrived: AICD §23.
///
/// Bounds the delay under a continuous stream of events, so a busy tree is
/// still reported at least this often.
pub const DEFAULT_MAX_HOLD: Duration = Duration::from_millis(500);

/// How many raw events may wait for the consumer before further ones are
/// counted as dropped ([`MissedReason::QueueFull`]).
pub const DEFAULT_QUEUE_CAPACITY: usize = 65_536;

/// The most directory entries one scan of a newly created directory visits
/// before it stops and records [`MissedReason::ScanBudget`].
pub const NEW_DIRECTORY_SCAN_ENTRY_CAP: usize = 10_000;

/// The deepest a scan of a newly created directory descends.
pub const NEW_DIRECTORY_SCAN_DEPTH_CAP: usize = 64;

/// The most distinct reasons one [`MissedChanges`] lists; further ones only
/// raise [`MissedChanges::occurrences`].
const MISSED_REASON_CAP: usize = 16;

/// A pause before scanning a newly created directory, so the backend has
/// registered its watch on it first.
const SCAN_SETTLE: Duration = Duration::from_millis(20);

/// What happened to a path: AICD §23.
///
/// The best classification the operating system gave; see the module doc.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ChangeKind {
    /// The path appeared. A file replaced atomically may read as this.
    Created,
    /// The path's content or metadata changed.
    Modified,
    /// The path is gone. Its former type is no longer knowable, so a removed
    /// directory is one entry for its path.
    Removed,
}

/// One change to the watched tree: AICD §23.
///
/// `spec/DATA_MODEL.md` section 2 gives `UnattributedChange` a `path` and a
/// `kind` of "edit, commit, push"; every [`Change`] here is the "edit" case,
/// and commits and pushes are [`RefChange`] and `crate::hooks::HookEvent`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    /// The path relative to the watched root, in the platform's separators,
    /// holding only normal components. Never absolute, never `..`.
    pub path: PathBuf,
    /// What happened to it.
    pub kind: ChangeKind,
    /// When the first event of this path in its batch reached this module.
    pub observed_at: Timestamp,
}

impl Change {
    /// The path as UTF-8 text with `/` separators, which is how a recorded
    /// event names a file; `None` when a component is not UTF-8, because a
    /// lossy conversion would map two names onto one (the reasoning
    /// `indexer.rs` records for `SkipReason::NonUtf8Path`).
    #[must_use]
    pub fn path_text(&self) -> Option<String> {
        let mut parts = Vec::new();
        for component in self.path.components() {
            parts.push(component.as_os_str().to_str()?);
        }
        Some(parts.join("/"))
    }
}

/// Why the watcher may have lost changes: AICD §14.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MissedReason {
    /// The backend said events were lost or must be rescanned, such as an
    /// inotify queue overflow.
    Overflow,
    /// The backend reported an error. The text is data from the operating
    /// system, never an instruction.
    BackendError {
        /// What the backend said.
        message: String,
    },
    /// This module's queue filled because the consumer did not drain it.
    QueueFull {
        /// How many events were dropped.
        dropped: u32,
    },
    /// The watched root itself was removed.
    RootGone,
    /// A scan of a newly created directory stopped at its budget, or could
    /// not read a directory.
    ScanBudget {
        /// The directory, relative to the root.
        directory: PathBuf,
    },
}

/// The recorded state "this watcher may have missed changes": AICD §14.
///
/// Delivered once in the [`Batch`] that follows it and held by
/// [`TreeWatcher::health`] until [`TreeWatcher::mark_reconciled`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissedChanges {
    /// The distinct causes, in the order first seen, at most sixteen.
    pub reasons: Vec<MissedReason>,
    /// How many times a cause was recorded, counting repeats.
    pub occurrences: u32,
    /// When the first one was recorded.
    pub first_observed_at: Timestamp,
}

impl MissedChanges {
    fn new(reason: MissedReason, at: Timestamp) -> Self {
        Self {
            reasons: vec![reason],
            occurrences: 1,
            first_observed_at: at,
        }
    }

    fn record(&mut self, reason: MissedReason) {
        self.occurrences = self.occurrences.saturating_add(1);
        if self.reasons.len() < MISSED_REASON_CAP && !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    fn absorb(&mut self, other: &Self) {
        self.occurrences = self.occurrences.saturating_add(other.occurrences);
        self.first_observed_at = self.first_observed_at.min(other.first_observed_at);
        for reason in &other.reasons {
            if self.reasons.len() < MISSED_REASON_CAP && !self.reasons.contains(reason) {
                self.reasons.push(reason.clone());
            }
        }
    }
}

/// Whether the watcher can be trusted to have seen everything: AICD §14.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Health {
    /// Nothing is known to have been missed.
    Watching,
    /// Something may have been missed and nobody has reconciled yet.
    MayHaveMissedChanges(MissedChanges),
    /// The backend's thread has ended and no further event will ever arrive.
    /// Set when [`TreeWatcher::next_batch`] sees it, and for good: a stopped
    /// watcher is replaced, not reconciled.
    Stopped,
}

/// A marker taken when a reconciliation begins: AICD §14.
///
/// [`TreeWatcher::mark_reconciled`] clears the missed state only if nothing
/// was recorded after the token was taken.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconcileToken {
    generation: u64,
}

/// What [`TreeWatcher::next_batch`] hands back for one quiet period:
/// AICD §23.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Batch {
    /// Each changed path once, earliest first.
    pub changes: Vec<Change>,
    /// Set when changes may have been lost; see [`MissedChanges`].
    pub missed: Option<MissedChanges>,
    /// Events dropped by an ignore rule ([`IgnoreRule`]).
    pub ignored: usize,
    /// Paths that were created and removed inside the window and so net to
    /// nothing.
    pub transient: usize,
    /// Events whose path lay outside the root or went through a link, and so
    /// were refused (see the module doc's "Confinement").
    pub rejected: usize,
}

/// A name the watcher does not report: AICD §23.
///
/// `.git` is always ignored and is not configurable.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum IgnoreRule {
    /// A directory or file of this exact name at any depth.
    AnyDepth(OsString),
    /// A directory or file of this exact name directly under the root.
    TopLevel(OsString),
}

/// How a [`TreeWatcher`] behaves: AICD §23.
#[derive(Clone, Debug)]
pub struct WatchConfig {
    /// See [`DEFAULT_QUIET`].
    pub quiet: Duration,
    /// See [`DEFAULT_MAX_HOLD`].
    pub max_hold: Duration,
    /// See [`DEFAULT_QUEUE_CAPACITY`]. At least one.
    pub queue_capacity: usize,
    /// Names ignored besides `.git`.
    pub ignore: Vec<IgnoreRule>,
}

impl Default for WatchConfig {
    /// The defaults the module doc states: `node_modules` at any depth,
    /// `target` at the root.
    fn default() -> Self {
        Self {
            quiet: DEFAULT_QUIET,
            max_hold: DEFAULT_MAX_HOLD,
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            ignore: vec![
                IgnoreRule::AnyDepth(OsString::from("node_modules")),
                IgnoreRule::TopLevel(OsString::from("target")),
            ],
        }
    }
}

/// Everything the watcher and the ref poller refuse or cannot do: AICD §14.
///
/// Mirrors `WorktreeError`'s split: a control refusing an action carries a
/// methodology reason ([`WatchError::methodology_ref`]); an I/O failure does
/// not, because a reference in front of a human that explains nothing is the
/// defect AICD §39 records.
#[derive(Debug)]
#[non_exhaustive]
pub enum WatchError {
    /// The root could not be resolved.
    Root {
        /// The path as given.
        path: PathBuf,
        /// What the operating system said.
        source: io::Error,
    },
    /// The root resolved to something that is not a directory.
    RootNotADirectory {
        /// The resolved path.
        path: PathBuf,
    },
    /// The notify backend refused to start or to watch the root, for example
    /// because the operating system's watch limit is reached.
    Backend {
        /// What the backend said.
        message: String,
    },
    /// The backend thread ended; no further event will ever arrive. Never
    /// silent: the consumer is told here.
    Stopped,
    /// Reading the repository's metadata failed.
    Io {
        /// The path being read.
        path: PathBuf,
        /// What the operating system said.
        source: io::Error,
    },
    /// `.git` is a link or is not a directory, so refs are not read through it.
    GitDirNotADirectory {
        /// The path of `.git`.
        path: PathBuf,
    },
    /// `HEAD` or `refs/` is a link or not the plain file or directory git
    /// writes, so it is not read: followed, a link would turn whatever it
    /// points at into refs, and a first line of some other file into an event.
    RefStorageNotPlain {
        /// The path that is not plain.
        path: PathBuf,
    },
    /// The repository stores refs in a format this poller cannot read, and
    /// reading none would be silence.
    UnsupportedRefStorage {
        /// The path that shows it.
        path: PathBuf,
    },
}

impl WatchError {
    /// The methodology section a refusal rests on, for the refusals only.
    ///
    /// [`WatchError::UnsupportedRefStorage`] cites AICD §14: a poller that
    /// cannot read this repository's refs and reports no change would be
    /// "present but reporting nothing". [`WatchError::GitDirNotADirectory`]
    /// and [`WatchError::RefStorageNotPlain`] cite AICD §13: git is the audit
    /// trail, and reading it through a link would audit some other
    /// repository.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        let section = match self {
            Self::UnsupportedRefStorage { .. } => 14,
            Self::GitDirNotADirectory { .. } | Self::RefStorageNotPlain { .. } => 13,
            _ => return None,
        };
        Some(MethodologyRef {
            section,
            subsection: None,
        })
    }
}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root { path, source } => {
                write!(
                    f,
                    "cannot resolve the watch root {}: {source}",
                    path.display()
                )
            }
            Self::RootNotADirectory { path } => {
                write!(f, "the watch root is not a directory: {}", path.display())
            }
            Self::Backend { message } => write!(f, "the file-system watcher failed: {message}"),
            Self::Stopped => {
                f.write_str("the file-system watcher stopped; changes are no longer being observed")
            }
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::GitDirNotADirectory { path } => write!(
                f,
                "refused: {} is a link or not a directory, so refs are not read through it ({})",
                path.display(),
                MethodologyRef {
                    section: 13,
                    subsection: None
                }
            ),
            Self::RefStorageNotPlain { path } => write!(
                f,
                "refused: {} is a link or not a plain file or directory, so it is not read \
                 ({})",
                path.display(),
                MethodologyRef {
                    section: 13,
                    subsection: None
                }
            ),
            Self::UnsupportedRefStorage { path } => write!(
                f,
                "refused: {} shows a ref storage format this poller cannot read, and polling \
                 nothing would be present but reporting nothing ({})",
                path.display(),
                MethodologyRef {
                    section: 14,
                    subsection: None
                }
            ),
        }
    }
}

impl Error for WatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Root { source, .. } | Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// What the backend's thread sends the consumer.
#[derive(Debug)]
enum Raw {
    Change {
        path: PathBuf,
        kind: ChangeKind,
        at: Timestamp,
    },
    Missed(MissedReason, Timestamp),
}

/// State the backend's thread and the consumer share.
#[derive(Debug, Default)]
struct Shared {
    /// Events the queue had no room for.
    dropped: AtomicU32,
    /// Makes the backend's thread die at its next event, so a test can watch
    /// the consumer find out.
    #[cfg(test)]
    panic_in_backend: std::sync::atomic::AtomicBool,
}

/// Which rename halves the backend has just delivered, so the combined event
/// that follows them is not counted a second time.
#[derive(Debug, Default)]
struct RenameTracking {
    from: Option<usize>,
    to: Option<usize>,
}

/// Watches one tree and reports changes: AICD §13, AICD §23.
///
/// See the module doc for confinement, ignoring, coalescing and what happens
/// when events are lost. Dropping the value stops the backend.
#[derive(Debug)]
pub struct TreeWatcher {
    root: PathBuf,
    config: WatchConfig,
    receiver: Receiver<Raw>,
    shared: Arc<Shared>,
    health: Health,
    /// Counts the times a missed state was recorded, for [`ReconcileToken`].
    generation: u64,
    /// A second sender, so a test can put a notice on the queue as the backend
    /// would. It exists only in test builds: in a production build the
    /// backend's handler holds the only sender, so the queue disconnects, and
    /// [`WatchError::Stopped`] is returned, the moment the backend's thread
    /// ends. A sender held here would keep it connected for ever.
    #[cfg(test)]
    injector: Option<SyncSender<Raw>>,
    /// Held for its `Drop`: dropping it stops the backend thread. Taken in
    /// [`TreeWatcher`]'s own `Drop` so a dead backend cannot panic the caller.
    backend: Option<RecommendedWatcher>,
}

impl Drop for TreeWatcher {
    /// Stops the backend, and survives a backend that is already dead.
    ///
    /// notify's `INotifyWatcher::drop` does `self.channel.send(Shutdown).unwrap()`
    /// ("we expect the event loop to live"). If the event loop's thread has
    /// panicked, that send fails and the `unwrap` panics, inside whatever
    /// dropped the watcher: the engine. The drop is caught here instead. The
    /// panic message is still printed by the panic hook, so it is not silent,
    /// but it ends here. This holds under the default `panic = "unwind"`; a
    /// build with `panic = "abort"` would abort, and this workspace does not
    /// set it.
    fn drop(&mut self) {
        if let Some(backend) = self.backend.take() {
            let _ = catch_unwind(AssertUnwindSafe(move || drop(backend)));
        }
    }
}

impl TreeWatcher {
    /// Starts watching `root` with the default configuration: AICD §23.
    ///
    /// # Errors
    ///
    /// [`WatchError::Root`] or [`WatchError::RootNotADirectory`] when `root`
    /// is not a directory that can be resolved, [`WatchError::Backend`] when
    /// the operating system refuses the watch.
    pub fn watch(root: &Path) -> Result<Self, WatchError> {
        Self::watch_with(root, WatchConfig::default())
    }

    /// Starts watching `root` with `config`: AICD §23.
    ///
    /// The watch is registered when this returns, so a change made after the
    /// call is observed.
    ///
    /// # Errors
    ///
    /// As [`TreeWatcher::watch`].
    pub fn watch_with(root: &Path, config: WatchConfig) -> Result<Self, WatchError> {
        let root = fs::canonicalize(root).map_err(|source| WatchError::Root {
            path: root.to_path_buf(),
            source,
        })?;
        let metadata = fs::metadata(&root).map_err(|source| WatchError::Root {
            path: root.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(WatchError::RootNotADirectory { path: root });
        }
        let (sender, receiver) = mpsc::sync_channel(config.queue_capacity.max(1));
        let shared = Arc::new(Shared::default());
        #[cfg(test)]
        let injector = Some(sender.clone());
        let handler_shared = Arc::clone(&shared);
        let mut renames = RenameTracking::default();
        let handler = move |result: notify::Result<Event>| {
            #[cfg(test)]
            if handler_shared.panic_in_backend.load(Ordering::SeqCst) {
                panic!("test: the backend's thread dies here");
            }
            for raw in translate(result, now(), &mut renames) {
                match sender.try_send(raw) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        handler_shared.dropped.fetch_add(1, Ordering::SeqCst);
                    }
                    // The consumer is gone; the watcher is being dropped.
                    Err(TrySendError::Disconnected(_)) => {}
                }
            }
        };
        let mut backend =
            RecommendedWatcher::new(handler, Config::default().with_follow_symlinks(false))
                .map_err(|error| WatchError::Backend {
                    message: error.to_string(),
                })?;
        backend
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|error| WatchError::Backend {
                message: error.to_string(),
            })?;
        Ok(Self {
            root,
            config,
            receiver,
            shared,
            health: Health::Watching,
            generation: 0,
            #[cfg(test)]
            injector,
            backend: Some(backend),
        })
    }

    /// The canonical root every [`Change::path`] is relative to.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether the watcher may have missed changes: AICD §14.
    #[must_use]
    pub fn health(&self) -> Health {
        self.health.clone()
    }

    /// Takes the marker a reconciliation starts from: AICD §14.
    ///
    /// Drain the watcher with [`TreeWatcher::next_batch`] first, then take
    /// the token, then compare the tree with git, then hand the token to
    /// [`TreeWatcher::mark_reconciled`].
    #[must_use]
    pub fn begin_reconcile(&self) -> ReconcileToken {
        ReconcileToken {
            generation: self.generation,
        }
    }

    /// Clears [`Health::MayHaveMissedChanges`] after the engine has compared
    /// the tree with git and attributed or raised whatever differed:
    /// AICD §14.
    ///
    /// Clears only what the reconciliation covered. It returns `false` and
    /// leaves the state as it is when a missed state was recorded after
    /// `token` was taken (the comparison began before the loss, so it may not
    /// include it), when events were dropped that no batch has reported yet,
    /// or when the backend has stopped. A notice still waiting in the queue
    /// is recorded when the next batch takes it, after which the state is not
    /// clear: a token only ever errs towards keeping the state.
    pub fn mark_reconciled(&mut self, token: ReconcileToken) -> bool {
        if matches!(self.health, Health::Stopped)
            || token.generation != self.generation
            || self.shared.dropped.load(Ordering::SeqCst) > 0
        {
            return false;
        }
        self.health = Health::Watching;
        true
    }

    /// Waits up to `wait` for a change, then collects until the tree is quiet,
    /// and returns the coalesced batch: AICD §23.
    ///
    /// Returns `Ok(None)` when nothing at all arrived within `wait`. A batch
    /// whose events were all ignored is still returned, with empty
    /// [`Batch::changes`] and the counters set, so ignoring is visible.
    ///
    /// # Errors
    ///
    /// [`WatchError::Stopped`] when the backend thread has ended.
    pub fn next_batch(&mut self, wait: Duration) -> Result<Option<Batch>, WatchError> {
        let first = match self.receiver.recv_timeout(wait) {
            Ok(raw) => Some(raw),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                self.health = Health::Stopped;
                return Err(WatchError::Stopped);
            }
        };
        let mut accumulator = Accumulator::new(&self.root, &self.config.ignore);
        let received = first.is_some();
        if let Some(raw) = first {
            accumulator.push(raw);
            let started = Instant::now();
            loop {
                let elapsed = started.elapsed();
                if elapsed >= self.config.max_hold {
                    break;
                }
                let slice = self.config.quiet.min(self.config.max_hold - elapsed);
                match self.receiver.recv_timeout(slice) {
                    Ok(raw) => accumulator.push(raw),
                    Err(RecvTimeoutError::Timeout) => break,
                    // Deliver what was collected; the next call reports it.
                    Err(RecvTimeoutError::Disconnected) => {
                        self.health = Health::Stopped;
                        break;
                    }
                }
            }
        }
        let dropped = self.shared.dropped.swap(0, Ordering::SeqCst);
        if dropped > 0 {
            accumulator.miss(MissedReason::QueueFull { dropped }, now());
        }
        let batch = accumulator.finish();
        if let Some(missed) = &batch.missed {
            self.generation += 1;
            match &mut self.health {
                Health::Watching => self.health = Health::MayHaveMissedChanges(missed.clone()),
                Health::MayHaveMissedChanges(held) => held.absorb(missed),
                Health::Stopped => {}
            }
        }
        if received || batch.missed.is_some() {
            Ok(Some(batch))
        } else {
            Ok(None)
        }
    }

    /// Puts a notice on the queue as the backend would, for tests.
    #[cfg(test)]
    fn inject(&self, raw: Raw) {
        self.injector
            .as_ref()
            .expect("the injector has not been dropped")
            .try_send(raw)
            .expect("the queue has room for a test notice");
    }

    /// Ends the backend the way a panic in its thread would, for tests: the
    /// handler (the only other sender) is dropped with the backend, and so is
    /// the injector, which leaves the production path to find the queue
    /// disconnected.
    #[cfg(test)]
    fn kill_backend_thread(&mut self) {
        self.shared.panic_in_backend.store(true, Ordering::SeqCst);
        self.injector = None;
    }
}

/// The current time as a [`Timestamp`]; before the epoch reads as zero.
fn now() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_millis(millis)
}

/// Turns one backend result into the values this module queues.
///
/// Reads are ignored (`Access` events are what a reader causes, not an edit);
/// a rename becomes a removal of the old name and a creation of the new one.
fn translate(
    result: notify::Result<Event>,
    at: Timestamp,
    renames: &mut RenameTracking,
) -> Vec<Raw> {
    let event = match result {
        Ok(event) => event,
        Err(error) => {
            return vec![Raw::Missed(
                MissedReason::BackendError {
                    message: error.to_string(),
                },
                at,
            )];
        }
    };
    let mut raws = Vec::new();
    if event.need_rescan() {
        raws.push(Raw::Missed(MissedReason::Overflow, at));
    }
    let change = |path: &PathBuf, kind: ChangeKind| Raw::Change {
        path: path.clone(),
        kind,
        at,
    };
    match event.kind {
        EventKind::Create(_) => {
            raws.extend(
                event
                    .paths
                    .iter()
                    .map(|path| change(path, ChangeKind::Created)),
            );
        }
        EventKind::Remove(_) => {
            raws.extend(
                event
                    .paths
                    .iter()
                    .map(|path| change(path, ChangeKind::Removed)),
            );
        }
        EventKind::Modify(ModifyKind::Name(mode)) => match mode {
            RenameMode::From => {
                renames.from = event.tracker();
                raws.extend(
                    event
                        .paths
                        .iter()
                        .map(|path| change(path, ChangeKind::Removed)),
                );
            }
            RenameMode::To => {
                renames.to = event.tracker();
                raws.extend(
                    event
                        .paths
                        .iter()
                        .map(|path| change(path, ChangeKind::Created)),
                );
            }
            RenameMode::Both => {
                // inotify sends the two halves and then this combined event
                // for one rename, with one tracker. Counting all three would
                // report the old name removed twice, which for a temporary
                // that already netted to nothing reports a removal of a file
                // that was never seen.
                let tracker = event.tracker();
                let already_counted =
                    tracker.is_some() && tracker == renames.from && tracker == renames.to;
                *renames = RenameTracking::default();
                if !already_counted && let [from, to] = event.paths.as_slice() {
                    raws.push(change(from, ChangeKind::Removed));
                    raws.push(change(to, ChangeKind::Created));
                }
            }
            // An unspecific rename names a path that now exists (the new
            // name) or does not (the old one).
            RenameMode::Any | RenameMode::Other => {
                for path in &event.paths {
                    let kind = if fs::symlink_metadata(path).is_ok() {
                        ChangeKind::Created
                    } else {
                        ChangeKind::Removed
                    };
                    raws.push(change(path, kind));
                }
            }
        },
        EventKind::Modify(_) | EventKind::Any => {
            raws.extend(
                event
                    .paths
                    .iter()
                    .map(|path| change(path, ChangeKind::Modified)),
            );
        }
        EventKind::Other => {
            // An `Other` with paths and no rescan flag is something happened
            // here that the backend cannot name.
            if !event.need_rescan() {
                raws.extend(
                    event
                        .paths
                        .iter()
                        .map(|path| change(path, ChangeKind::Modified)),
                );
            }
        }
        EventKind::Access(_) => {}
    }
    raws
}

/// Folds a second kind of change to a path into the first: AICD §23.
///
/// `None` means the two cancel: the path was created and removed inside one
/// window.
fn merge_kinds(previous: ChangeKind, next: ChangeKind) -> Option<ChangeKind> {
    use ChangeKind::{Created, Modified, Removed};
    match (previous, next) {
        (Created, Created | Modified) => Some(Created),
        (Created, Removed) => None,
        (Modified, Modified | Created) | (Removed, Created | Modified) => Some(Modified),
        (Modified | Removed, Removed) => Some(Removed),
    }
}

/// The part of `path` under `root`, refusing anything that is not plainly
/// inside it: AICD §23.
///
/// The check is lexical. `path` must start with `root` and what remains must
/// hold only normal components, so a `..`, a root, or a drive prefix in the
/// remainder is refused rather than trimmed. The result is empty for the root
/// itself.
fn relative_inside(root: &Path, path: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(root).ok()?;
    let mut clean = PathBuf::new();
    for component in rest.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(clean)
}

/// Whether every proper ancestor of `relative` under `root` is a real
/// directory, looked at without following links: AICD §23.
///
/// An ancestor that does not exist is not a link and is accepted (the path
/// vanished; nothing can be followed through it). Any other failure refuses.
fn ancestors_are_real_directories(root: &Path, relative: &Path) -> bool {
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            return true;
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return false;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
            Err(_) => return false,
        }
    }
    true
}

/// What a second look at a directory found.
enum DirectoryCheck {
    /// A real directory under real directories.
    Real,
    /// Not there, or no longer a directory: nothing to list.
    Gone,
    /// It, or a directory above it, is a link now.
    Refused,
}

/// Looks at `relative` and its ancestors without following links, to decide
/// whether it may be listed: AICD §23.
fn check_directory(root: &Path, relative: &Path) -> DirectoryCheck {
    if !ancestors_are_real_directories(root, relative) {
        return DirectoryCheck::Refused;
    }
    match fs::symlink_metadata(root.join(relative)) {
        Ok(metadata) if metadata.file_type().is_symlink() => DirectoryCheck::Refused,
        Ok(metadata) if metadata.is_dir() => DirectoryCheck::Real,
        Ok(_) => DirectoryCheck::Gone,
        Err(error) if error.kind() == io::ErrorKind::NotFound => DirectoryCheck::Gone,
        Err(_) => DirectoryCheck::Refused,
    }
}

/// Whether `relative` is a name the watcher does not report: AICD §23.
fn is_ignored(relative: &Path, rules: &[IgnoreRule]) -> bool {
    for (index, component) in relative.components().enumerate() {
        let name = component.as_os_str();
        if name
            .to_str()
            .is_some_and(|text| text.eq_ignore_ascii_case(".git"))
        {
            return true;
        }
        for rule in rules {
            match rule {
                IgnoreRule::AnyDepth(wanted) if name == wanted.as_os_str() => return true,
                IgnoreRule::TopLevel(wanted) if index == 0 && name == wanted.as_os_str() => {
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// One path's folded state inside a batch.
#[derive(Debug)]
struct Entry {
    kind: ChangeKind,
    first_at: Timestamp,
}

/// Builds one [`Batch`] from raw events.
struct Accumulator<'a> {
    root: &'a Path,
    rules: &'a [IgnoreRule],
    entries: BTreeMap<PathBuf, Entry>,
    scans: BTreeSet<(PathBuf, Timestamp)>,
    scan_visited: usize,
    ignored: usize,
    transient: usize,
    rejected: usize,
    missed: Option<MissedChanges>,
}

impl<'a> Accumulator<'a> {
    fn new(root: &'a Path, rules: &'a [IgnoreRule]) -> Self {
        Self {
            root,
            rules,
            entries: BTreeMap::new(),
            scans: BTreeSet::new(),
            scan_visited: 0,
            ignored: 0,
            transient: 0,
            rejected: 0,
            missed: None,
        }
    }

    fn miss(&mut self, reason: MissedReason, at: Timestamp) {
        match &mut self.missed {
            Some(missed) => missed.record(reason),
            None => self.missed = Some(MissedChanges::new(reason, at)),
        }
    }

    fn push(&mut self, raw: Raw) {
        match raw {
            Raw::Missed(reason, at) => self.miss(reason, at),
            Raw::Change { path, kind, at } => self.change(&path, kind, at),
        }
    }

    fn change(&mut self, path: &Path, kind: ChangeKind, at: Timestamp) {
        let Some(relative) = relative_inside(self.root, path) else {
            self.rejected += 1;
            return;
        };
        if relative.as_os_str().is_empty() {
            // The root itself: its removal is the watch dying; anything else
            // is a directory event.
            if kind == ChangeKind::Removed {
                self.miss(MissedReason::RootGone, at);
            }
            return;
        }
        if is_ignored(&relative, self.rules) {
            self.ignored += 1;
            return;
        }
        if !self.entries.contains_key(&relative)
            && !ancestors_are_real_directories(self.root, &relative)
        {
            self.rejected += 1;
            return;
        }
        if kind != ChangeKind::Removed
            && let Ok(metadata) = fs::symlink_metadata(self.root.join(&relative))
            && metadata.is_dir()
        {
            // A directory is not a change git sees. A new one may already
            // hold files the backend will never report.
            if kind == ChangeKind::Created {
                self.scans.insert((relative, at));
            }
            return;
        }
        self.add(relative, kind, at);
    }

    fn add(&mut self, relative: PathBuf, kind: ChangeKind, at: Timestamp) {
        match self.entries.remove(&relative) {
            None => {
                self.entries.insert(relative, Entry { kind, first_at: at });
            }
            Some(previous) => match merge_kinds(previous.kind, kind) {
                Some(merged) => {
                    self.entries.insert(
                        relative,
                        Entry {
                            kind: merged,
                            first_at: previous.first_at,
                        },
                    );
                }
                None => self.transient += 1,
            },
        }
    }

    /// Scans the directories that appeared during the window for the files
    /// already in them.
    fn scan_new_directories(&mut self) {
        let scans = std::mem::take(&mut self.scans);
        if scans.is_empty() {
            return;
        }
        std::thread::sleep(SCAN_SETTLE);
        for (directory, at) in scans {
            let mut stack = vec![(directory, 0usize)];
            while let Some((dir, depth)) = stack.pop() {
                // The directory was checked when its event arrived, at least
                // `quiet` ago. A directory that was removed and replaced by a
                // link in that time would be listed through the link, so it
                // is looked at again, with its ancestors, just before it is
                // read.
                match check_directory(self.root, &dir) {
                    DirectoryCheck::Real => {}
                    DirectoryCheck::Gone => continue,
                    DirectoryCheck::Refused => {
                        self.rejected += 1;
                        continue;
                    }
                }
                let listing = match fs::read_dir(self.root.join(&dir)) {
                    Ok(listing) => listing,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(_) => {
                        self.miss(MissedReason::ScanBudget { directory: dir }, at);
                        continue;
                    }
                };
                for entry in listing {
                    let Ok(entry) = entry else {
                        self.miss(
                            MissedReason::ScanBudget {
                                directory: dir.clone(),
                            },
                            at,
                        );
                        continue;
                    };
                    self.scan_visited += 1;
                    if self.scan_visited > NEW_DIRECTORY_SCAN_ENTRY_CAP {
                        self.miss(MissedReason::ScanBudget { directory: dir }, at);
                        return;
                    }
                    let relative = dir.join(entry.file_name());
                    if is_ignored(&relative, self.rules) {
                        self.ignored += 1;
                        continue;
                    }
                    let Ok(file_type) = entry.file_type() else {
                        continue;
                    };
                    if file_type.is_symlink() || !file_type.is_dir() {
                        // A link is an entry of its own, never followed.
                        self.add(relative, ChangeKind::Created, at);
                    } else if depth < NEW_DIRECTORY_SCAN_DEPTH_CAP {
                        stack.push((relative, depth + 1));
                    } else {
                        self.miss(
                            MissedReason::ScanBudget {
                                directory: relative,
                            },
                            at,
                        );
                    }
                }
            }
        }
    }

    fn finish(mut self) -> Batch {
        self.scan_new_directories();
        let mut changes: Vec<Change> = self
            .entries
            .into_iter()
            .map(|(path, entry)| Change {
                path,
                kind: entry.kind,
                observed_at: entry.first_at,
            })
            .collect();
        changes.sort_by(|left, right| {
            left.observed_at
                .cmp(&right.observed_at)
                .then_with(|| left.path.cmp(&right.path))
        });
        Batch {
            changes,
            missed: self.missed,
            ignored: self.ignored,
            transient: self.transient,
            rejected: self.rejected,
        }
    }
}

/// What a ref's name belongs to: AICD §13.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RefClass {
    /// `HEAD`.
    Head,
    /// `refs/heads/*`: a local branch moved, which is a commit, a merge, a
    /// rebase or a reset on it.
    Branch,
    /// `refs/remotes/*`: git updates these after a successful push, and after
    /// a fetch.
    RemoteTracking,
    /// `refs/tags/*`.
    Tag,
    /// Anything else under `refs/`.
    Other,
}

/// A ref that appeared, moved or vanished between two polls: AICD §13.
///
/// `before` and `after` are the first line of the ref as stored, at most 256
/// bytes: an object id, or `ref: <target>` for a symbolic ref. They are data
/// from the repository, never instructions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefChange {
    /// The ref's name, `/`-separated (`HEAD`, `refs/heads/main`).
    pub name: String,
    /// The value at the previous poll; `None` when the ref is new.
    pub before: Option<String>,
    /// The value now; `None` when the ref is gone.
    pub after: Option<String>,
    /// When this poll ran.
    pub observed_at: Timestamp,
}

impl RefChange {
    /// What the ref is.
    #[must_use]
    pub fn class(&self) -> RefClass {
        if self.name == "HEAD" {
            RefClass::Head
        } else if self.name.starts_with("refs/heads/") {
            RefClass::Branch
        } else if self.name.starts_with("refs/remotes/") {
            RefClass::RemoteTracking
        } else if self.name.starts_with("refs/tags/") {
            RefClass::Tag
        } else {
            RefClass::Other
        }
    }
}

/// A ref file the poller did not read, and why: AICD §14.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedRef {
    /// The path of the entry.
    pub path: PathBuf,
    /// Why it was left out.
    pub reason: SkippedRefReason,
}

/// Why a ref entry was left out: AICD §14.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SkippedRefReason {
    /// A symbolic link: followed, it could read a file anywhere.
    Symlink,
    /// Not a regular file or a directory.
    NotARegularFile,
    /// Longer than the bound for a ref file.
    TooLarge,
    /// The name is not UTF-8, so it cannot be a recorded ref name.
    NonUtf8Name,
    /// The entry could not be read.
    Unreadable {
        /// What the operating system said.
        error: String,
    },
}

/// The result of one [`RefPoller::poll`]: AICD §13.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RefPoll {
    /// Refs that appeared, moved or vanished since the previous poll, by name.
    pub changes: Vec<RefChange>,
    /// Entries that were not read.
    pub skipped: Vec<SkippedRef>,
    /// Set when the walk hit its bound; removals are then not inferred.
    pub incomplete: bool,
}

/// The most refs one poll reads before it stops and says it is incomplete.
const REF_ENTRY_CAP: usize = 100_000;
/// The longest a loose ref file may be.
const REF_FILE_BYTES_CAP: u64 = 4096;
/// The most of `packed-refs` that is read.
const PACKED_REFS_BYTES_CAP: u64 = 64 * 1024 * 1024;
/// The deepest `refs/` is descended.
const REF_DEPTH_CAP: usize = 32;
/// The most of one ref's value that is kept.
const REF_VALUE_CAP: usize = 256;

/// Every ref read in one pass, with what was left out.
struct Snapshot {
    refs: BTreeMap<String, String>,
    skipped: Vec<SkippedRef>,
    incomplete: bool,
}

/// Polls a repository's refs and reports what moved: AICD §13.
///
/// See the module doc's "Polling refs". The repository's `.git` must be a
/// real directory; for a linked worktree, poll the repository that owns it:
/// every worktree's branches are refs of that repository.
#[derive(Debug)]
pub struct RefPoller {
    git_dir: PathBuf,
    last: BTreeMap<String, String>,
}

impl RefPoller {
    /// Opens the poller on the repository at `repo_root` and takes the
    /// baseline, so the first [`RefPoller::poll`] reports only what changed
    /// after this call: AICD §13.
    ///
    /// # Errors
    ///
    /// [`WatchError::GitDirNotADirectory`] when `.git` is a link or a file,
    /// [`WatchError::RefStorageNotPlain`] when `HEAD` or `refs/` is a link or
    /// a pipe, [`WatchError::UnsupportedRefStorage`] when the repository uses
    /// the reftable format, [`WatchError::Io`] when the refs cannot be read.
    pub fn open(repo_root: &Path) -> Result<Self, WatchError> {
        let git_dir = repo_root.join(".git");
        let metadata = fs::symlink_metadata(&git_dir).map_err(|source| WatchError::Io {
            path: git_dir.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(WatchError::GitDirNotADirectory { path: git_dir });
        }
        let reftable = git_dir.join("reftable");
        if fs::symlink_metadata(&reftable).is_ok() {
            return Err(WatchError::UnsupportedRefStorage { path: reftable });
        }
        let mut poller = Self {
            git_dir,
            last: BTreeMap::new(),
        };
        poller.last = poller.snapshot()?.refs;
        Ok(poller)
    }

    /// Reads the refs now and reports the difference from the last poll:
    /// AICD §13.
    ///
    /// # Errors
    ///
    /// [`WatchError::RefStorageNotPlain`] when `HEAD` or `refs/` has become a
    /// link or a pipe, [`WatchError::Io`] when they cannot be read.
    pub fn poll(&mut self) -> Result<RefPoll, WatchError> {
        let Snapshot {
            refs: current,
            skipped,
            incomplete,
        } = self.snapshot()?;
        let at = now();
        let mut changes = Vec::new();
        for (name, value) in &current {
            match self.last.get(name) {
                Some(before) if before == value => {}
                before => changes.push(RefChange {
                    name: name.clone(),
                    before: before.cloned(),
                    after: Some(value.clone()),
                    observed_at: at,
                }),
            }
        }
        if !incomplete {
            for (name, before) in &self.last {
                if !current.contains_key(name) {
                    changes.push(RefChange {
                        name: name.clone(),
                        before: Some(before.clone()),
                        after: None,
                        observed_at: at,
                    });
                }
            }
            self.last = current;
        } else {
            self.last.extend(current);
        }
        changes.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(RefPoll {
            changes,
            skipped,
            incomplete,
        })
    }

    fn snapshot(&self) -> Result<Snapshot, WatchError> {
        self.snapshot_with(&mut || {})
    }

    /// Reads `HEAD`, then every loose ref, then `packed-refs`, calling
    /// `between` after the loose refs and before `packed-refs`.
    ///
    /// The order is the point. `git pack-refs` writes the new `packed-refs`
    /// first and prunes the loose files after. Reading `packed-refs` first
    /// and the loose refs second can see the old packed file and the pruned
    /// directory, so a branch that never moved looks deleted and, one poll
    /// later, created. Reading the loose refs first sees them either before
    /// the prune or not at all, and `packed-refs`, read after, then holds
    /// them. `between` is where a test packs refs.
    fn snapshot_with(&self, between: &mut dyn FnMut()) -> Result<Snapshot, WatchError> {
        let mut skipped = Vec::new();
        let mut incomplete = false;

        let head = self.git_dir.join("HEAD");
        let (text, too_long) = match read_plain_file(&head, REF_FILE_BYTES_CAP) {
            Ok(read) => read,
            Err(PlainReadError::Symlink | PlainReadError::NotARegularFile) => {
                return Err(WatchError::RefStorageNotPlain { path: head });
            }
            Err(PlainReadError::Io(source)) => {
                return Err(WatchError::Io { path: head, source });
            }
        };
        if too_long {
            return Err(WatchError::Io {
                path: head,
                source: io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the file is longer than a ref file can be",
                ),
            });
        }
        let head_value = cap_value(text.lines().next().unwrap_or(""));

        let refs_dir = self.git_dir.join("refs");
        match fs::symlink_metadata(&refs_dir) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(WatchError::RefStorageNotPlain { path: refs_dir });
            }
            Ok(_) => {}
            Err(source) => {
                return Err(WatchError::Io {
                    path: refs_dir,
                    source,
                });
            }
        }
        let mut loose = BTreeMap::new();
        let mut stack = vec![(refs_dir, "refs".to_owned(), 0usize)];
        let mut visited = 0usize;
        'walk: while let Some((dir, prefix, depth)) = stack.pop() {
            let listing = fs::read_dir(&dir).map_err(|source| WatchError::Io {
                path: dir.clone(),
                source,
            })?;
            for entry in listing {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        skipped.push(SkippedRef {
                            path: dir.clone(),
                            reason: SkippedRefReason::Unreadable {
                                error: error.to_string(),
                            },
                        });
                        continue;
                    }
                };
                visited += 1;
                if visited > REF_ENTRY_CAP {
                    incomplete = true;
                    break 'walk;
                }
                let path = entry.path();
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    skipped.push(SkippedRef {
                        path,
                        reason: SkippedRefReason::NonUtf8Name,
                    });
                    continue;
                };
                // Git writes `<ref>.lock` while it updates `<ref>`; a ref
                // name cannot end that way.
                if name.ends_with(".lock") {
                    continue;
                }
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => {
                        skipped.push(SkippedRef {
                            path,
                            reason: SkippedRefReason::Unreadable {
                                error: error.to_string(),
                            },
                        });
                        continue;
                    }
                };
                if file_type.is_symlink() {
                    skipped.push(SkippedRef {
                        path,
                        reason: SkippedRefReason::Symlink,
                    });
                } else if file_type.is_dir() {
                    if depth < REF_DEPTH_CAP {
                        stack.push((path, format!("{prefix}/{name}"), depth + 1));
                    } else {
                        incomplete = true;
                    }
                } else if file_type.is_file() {
                    match read_plain_file(&path, REF_FILE_BYTES_CAP) {
                        Ok((text, false)) => {
                            loose.insert(
                                format!("{prefix}/{name}"),
                                cap_value(text.lines().next().unwrap_or("")),
                            );
                        }
                        Ok((_, true)) => skipped.push(SkippedRef {
                            path,
                            reason: SkippedRefReason::TooLarge,
                        }),
                        // Swapped for a link or a pipe since the listing.
                        Err(PlainReadError::Symlink) => skipped.push(SkippedRef {
                            path,
                            reason: SkippedRefReason::Symlink,
                        }),
                        Err(PlainReadError::NotARegularFile) => skipped.push(SkippedRef {
                            path,
                            reason: SkippedRefReason::NotARegularFile,
                        }),
                        // Git removed it between the listing and the read.
                        Err(PlainReadError::Io(error))
                            if error.kind() == io::ErrorKind::NotFound => {}
                        Err(PlainReadError::Io(error)) => skipped.push(SkippedRef {
                            path,
                            reason: SkippedRefReason::Unreadable {
                                error: error.to_string(),
                            },
                        }),
                    }
                } else {
                    skipped.push(SkippedRef {
                        path,
                        reason: SkippedRefReason::NotARegularFile,
                    });
                }
            }
        }

        between();

        let mut refs = BTreeMap::new();
        let packed = self.git_dir.join("packed-refs");
        match read_plain_file(&packed, PACKED_REFS_BYTES_CAP) {
            Ok((text, truncated)) => {
                incomplete |= truncated;
                for line in text.lines() {
                    if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
                        continue;
                    }
                    if let Some((value, name)) = line.split_once(' ') {
                        refs.insert(name.to_owned(), cap_value(value));
                    }
                }
            }
            Err(PlainReadError::Symlink) => skipped.push(SkippedRef {
                path: packed,
                reason: SkippedRefReason::Symlink,
            }),
            Err(PlainReadError::NotARegularFile) => skipped.push(SkippedRef {
                path: packed,
                reason: SkippedRefReason::NotARegularFile,
            }),
            Err(PlainReadError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(PlainReadError::Io(error)) => skipped.push(SkippedRef {
                path: packed,
                reason: SkippedRefReason::Unreadable {
                    error: error.to_string(),
                },
            }),
        }
        // A loose ref is newer than the packed one of the same name.
        refs.extend(loose);
        refs.insert("HEAD".to_owned(), head_value);
        Ok(Snapshot {
            refs,
            skipped,
            incomplete,
        })
    }
}

/// Keeps at most [`REF_VALUE_CAP`] bytes of `value`, on a character boundary.
fn cap_value(value: &str) -> String {
    let value = value.trim();
    if value.len() <= REF_VALUE_CAP {
        return value.to_owned();
    }
    let mut end = REF_VALUE_CAP;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

/// Why [`read_plain_file`] did not read a file: AICD §13.
#[derive(Debug)]
pub(crate) enum PlainReadError {
    /// The path is a symbolic link (or, on Windows, a junction).
    Symlink,
    /// The path is not a regular file: a directory, a pipe, a device.
    NotARegularFile,
    /// The open or the read failed.
    Io(io::Error),
}

/// The raw `open(2)` flag values passed to `custom_flags`, on the platforms
/// `crates/ori-memory/src/code_map.rs` verified them for (see its
/// `raw_open_flags` for the evidence: macOS from the SDK header, Linux x86_64
/// and aarch64 each compiled and run). `libc` is not a dependency of this
/// crate and adding it is an escalation, so these are the platform's own
/// header constants, as that module does. Every other unix opens plainly,
/// after the `symlink_metadata` check in [`read_plain_file`].
#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
))]
mod raw_open_flags {
    #[cfg(target_os = "macos")]
    pub(super) const O_NOFOLLOW: i32 = 0x0100;
    #[cfg(target_os = "macos")]
    pub(super) const O_NONBLOCK: i32 = 0x0004;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(super) const O_NOFOLLOW: i32 = 0o400_000;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(super) const O_NONBLOCK: i32 = 0o4_000;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub(super) const O_NOFOLLOW: i32 = 0o100_000;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub(super) const O_NONBLOCK: i32 = 0o4_000;
}

/// Opens `path` for reading without following a final link and without
/// waiting for a pipe's other end: `open(2)` on a FIFO with no writer blocks
/// until one appears, and `O_NONBLOCK` is what stops that. The caller decides
/// by the handle's own metadata before it reads.
fn open_without_following(path: &Path) -> io::Result<fs::File> {
    #[cfg(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    ))]
    {
        use raw_open_flags::{O_NOFOLLOW, O_NONBLOCK};
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(O_NOFOLLOW | O_NONBLOCK)
            .open(path)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        windows,
    )))]
    {
        // The `symlink_metadata` check before this call has refused a link
        // and a pipe that is already in place; a swap after that check is not
        // covered on a platform whose flags are not verified.
        fs::File::open(path)
    }
}

/// Opens the regular file at `path` for reading, never through a link and
/// never waiting on a pipe, and returns the handle with its own metadata:
/// AICD §13.
///
/// The path is looked at without following (`symlink_metadata`), opened with
/// `O_NOFOLLOW | O_NONBLOCK` where those are verified (see
/// `raw_open_flags`), and the handle's own metadata must say regular file
/// before the caller reads anything. What it does not cover is stated where it
/// matters: a directory in the middle of the path swapped for a link by a live
/// writer.
pub(crate) fn open_plain_file(path: &Path) -> Result<(fs::File, fs::Metadata), PlainReadError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Err(PlainReadError::Symlink),
        Ok(metadata) if !metadata.is_file() => return Err(PlainReadError::NotARegularFile),
        Ok(_) => {}
        Err(error) => return Err(PlainReadError::Io(error)),
    }
    let file = open_without_following(path).map_err(PlainReadError::Io)?;
    let metadata = file.metadata().map_err(PlainReadError::Io)?;
    if metadata.file_type().is_symlink() {
        return Err(PlainReadError::Symlink);
    }
    if !metadata.is_file() {
        return Err(PlainReadError::NotARegularFile);
    }
    Ok((file, metadata))
}

/// Reads at most `limit` bytes of the regular file at `path` as text, through
/// [`open_plain_file`], and says whether more followed: AICD §13.
///
/// It is how everything in `.git` and the hooks directory is read.
pub(crate) fn read_plain_file(path: &Path, limit: u64) -> Result<(String, bool), PlainReadError> {
    let (file, _) = open_plain_file(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(PlainReadError::Io)?;
    let truncated = u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit;
    if truncated {
        bytes.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

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
                "ori-t-0054-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a fresh scratch directory can be created");
            // Resolve the scratch directory once so a test's own paths match
            // the watcher's canonical root.
            let path = fs::canonicalize(&path).expect("the scratch directory resolves");
            Self { path }
        }

        /// A directory inside the scratch directory, created.
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

    /// The most any single wait in a test lasts. Other coders load this
    /// machine; a wait that long is a failure with a message, never a hang.
    const PATIENCE: Duration = Duration::from_secs(10);

    fn quick() -> WatchConfig {
        WatchConfig {
            quiet: Duration::from_millis(60),
            max_hold: Duration::from_millis(400),
            ..WatchConfig::default()
        }
    }

    /// Collects batches until one holds a change `wanted` accepts, or fails
    /// with everything it saw. Returns every change seen on the way, and the
    /// sum of the counters.
    fn collect_until(
        watcher: &mut TreeWatcher,
        what: &str,
        wanted: impl Fn(&Change) -> bool,
    ) -> Seen {
        let deadline = Instant::now() + PATIENCE;
        let mut seen = Seen::default();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out after {PATIENCE:?} waiting for {what}; saw {seen:?}"
            );
            let batch = watcher
                .next_batch(remaining.min(Duration::from_millis(500)))
                .expect("the watcher is still running");
            if let Some(batch) = batch {
                seen.ignored += batch.ignored;
                seen.rejected += batch.rejected;
                seen.transient += batch.transient;
                seen.batches += 1;
                if let Some(missed) = batch.missed {
                    seen.missed.push(missed);
                }
                let found = batch.changes.iter().any(&wanted);
                seen.changes.extend(batch.changes);
                if found {
                    return seen;
                }
            }
        }
    }

    #[derive(Debug, Default)]
    struct Seen {
        changes: Vec<Change>,
        missed: Vec<MissedChanges>,
        ignored: usize,
        rejected: usize,
        transient: usize,
        batches: usize,
    }

    impl Seen {
        fn paths(&self) -> Vec<String> {
            self.changes
                .iter()
                .map(|change| change.path_text().expect("a UTF-8 path"))
                .collect()
        }
    }

    fn at(path: &str, kind: ChangeKind) -> impl Fn(&Change) -> bool {
        let path = PathBuf::from(path);
        move |change: &Change| change.path == path && change.kind == kind
    }

    #[test]
    fn ori_t_0054_an_edit_under_the_root_is_reported_with_its_relative_path_and_kind() {
        let scratch = Scratch::new("edit");
        let root = scratch.dir("root");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("lib.rs"), b"one").unwrap();
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();
        assert_eq!(watcher.root(), root.as_path());

        // Created: the path is relative to the root, never absolute.
        fs::write(root.join("src").join("new.rs"), b"fn main() {}").unwrap();
        let seen = collect_until(&mut watcher, "the creation of src/new.rs", |change| {
            change.path_text().as_deref() == Some("src/new.rs")
        });
        let created = seen
            .changes
            .iter()
            .find(|change| change.path_text().as_deref() == Some("src/new.rs"))
            .unwrap();
        assert_eq!(created.kind, ChangeKind::Created);
        assert!(created.path.is_relative(), "{:?}", created.path);
        assert!(created.observed_at.millis() > 0);

        // Modified.
        fs::write(root.join("src").join("lib.rs"), b"two, longer").unwrap();
        collect_until(
            &mut watcher,
            "the modification of src/lib.rs",
            at("src/lib.rs", ChangeKind::Modified),
        );

        // Removed.
        fs::remove_file(root.join("src").join("lib.rs")).unwrap();
        collect_until(
            &mut watcher,
            "the removal of src/lib.rs",
            at("src/lib.rs", ChangeKind::Removed),
        );
        assert_eq!(watcher.health(), Health::Watching);
    }

    #[test]
    fn ori_t_0054_an_edit_inside_a_linked_worktree_is_reported_relative_to_the_worktree() {
        let scratch = Scratch::new("worktree");
        let repo = scratch.dir("repo");
        let git = |dir: &Path, args: &[&str]| {
            let output = std::process::Command::new("git")
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
                .expect("git can be run");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&repo, &["init", "-q"]);
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src").join("lib.rs"), "one").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "first"]);
        // A session's worktree: a sibling directory whose `.git` is a file.
        let worktree = scratch.path.join("session-worktree");
        git(
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
        assert!(worktree.join(".git").is_file());

        let mut watcher = TreeWatcher::watch_with(&worktree, quick()).unwrap();
        fs::write(
            worktree.join("src").join("lib.rs"),
            "two, edited by the session",
        )
        .unwrap();
        // git's own bookkeeping in the worktree is not an edit.
        git(&worktree, &["add", "src/lib.rs"]);
        let seen = collect_until(
            &mut watcher,
            "src/lib.rs in the worktree",
            at("src/lib.rs", ChangeKind::Modified),
        );
        assert!(
            seen.paths()
                .iter()
                .all(|path| !path.split('/').any(|part| part == ".git")),
            "{:?}",
            seen.paths()
        );
    }

    #[test]
    fn ori_t_0054_a_file_written_into_a_directory_made_a_moment_before_is_not_missed() {
        let scratch = Scratch::new("newdir");
        let root = scratch.dir("root");
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();

        // The backend registers its watch on `a/b/c` after the directories
        // exist; the file below is written straight away.
        fs::create_dir_all(root.join("a").join("b").join("c")).unwrap();
        fs::write(root.join("a").join("b").join("c").join("deep.txt"), b"x").unwrap();
        let seen = collect_until(&mut watcher, "a/b/c/deep.txt", |change| {
            change.path_text().as_deref() == Some("a/b/c/deep.txt")
        });
        assert!(
            seen.changes
                .iter()
                .all(|change| change.path_text().as_deref() != Some("a")),
            "a directory is not itself a change: {:?}",
            seen.paths()
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_symlink_to_a_directory_outside_the_root_is_never_followed_or_reported_as_inside()
     {
        let scratch = Scratch::new("linkdir");
        let root = scratch.dir("root");
        let outside = scratch.dir("outside");
        fs::write(outside.join("secret.txt"), b"before").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();

        // Change the far side of the link, then a real file inside the root.
        // The real edit proves the watcher is alive, so the absence of the
        // other is a finding and not a silence.
        fs::write(outside.join("secret.txt"), b"after, and longer").unwrap();
        fs::write(outside.join("fresh.txt"), b"new").unwrap();
        fs::write(root.join("real.txt"), b"inside").unwrap();
        let seen = collect_until(
            &mut watcher,
            "real.txt",
            at("real.txt", ChangeKind::Created),
        );
        // Give a late event time to arrive, then look again.
        std::thread::sleep(Duration::from_millis(300));
        let later = watcher.next_batch(Duration::from_millis(300)).unwrap();
        let mut paths = seen.paths();
        if let Some(batch) = later {
            paths.extend(
                batch
                    .changes
                    .iter()
                    .map(|change| change.path_text().unwrap()),
            );
        }
        assert!(
            paths
                .iter()
                .all(|path| !path.starts_with("link/") && path != "outside"),
            "a path through the link was reported as inside the root: {paths:?}"
        );
        assert!(
            paths.iter().any(|path| path == "real.txt"),
            "the control edit was not seen: {paths:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_symlink_to_a_file_outside_the_root_is_not_reported_when_its_target_changes() {
        // On Linux this test cannot fail: inotify watches the link's directory,
        // not the file the link points at, so the backend never sends an event
        // for the far side and nothing here has anything to refuse. It stays
        // as a regression guard for backends that do follow a link to a file
        // (kqueue, polling). The refusal that matters, a path through a
        // linked directory, is in the test above and the ancestor check's own.
        let scratch = Scratch::new("linkfile");
        let root = scratch.dir("root");
        let outside = scratch.dir("outside");
        let target = outside.join("target.txt");
        fs::write(&target, b"before").unwrap();
        std::os::unix::fs::symlink(&target, root.join("alias.txt")).unwrap();
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();

        fs::write(&target, b"changed through the far side").unwrap();
        fs::write(root.join("real.txt"), b"inside").unwrap();
        let seen = collect_until(
            &mut watcher,
            "real.txt",
            at("real.txt", ChangeKind::Created),
        );
        std::thread::sleep(Duration::from_millis(300));
        let later = watcher.next_batch(Duration::from_millis(300)).unwrap();
        let mut paths = seen.paths();
        if let Some(batch) = later {
            paths.extend(batch.changes.iter().map(|c| c.path_text().unwrap()));
        }
        assert!(
            !paths.iter().any(|path| path == "alias.txt"),
            "a change to the far side of a link was reported as a change to {paths:?}"
        );
        assert!(paths.iter().any(|path| path == "real.txt"), "{paths:?}");
    }

    #[test]
    fn ori_t_0054_a_path_that_is_not_plainly_inside_the_root_is_refused() {
        let root = PathBuf::from(if cfg!(windows) {
            "C:\\work\\root"
        } else {
            "/work/root"
        });
        let join = |tail: &str| root.join(tail);
        assert_eq!(
            relative_inside(&root, &join("a/b.txt")),
            Some(PathBuf::from("a").join("b.txt"))
        );
        assert_eq!(relative_inside(&root, &root), Some(PathBuf::new()));
        // A `..` in the remainder is refused, not trimmed: it could leave.
        assert_eq!(relative_inside(&root, &join("a/../../elsewhere")), None);
        assert_eq!(relative_inside(&root, &join("..")), None);
        // A sibling whose name extends the root's is not inside it.
        let sibling = PathBuf::from(format!("{}-evil", root.display())).join("x");
        assert_eq!(relative_inside(&root, &sibling), None);
        // Something that is not under the root at all.
        let elsewhere = PathBuf::from(if cfg!(windows) {
            "C:\\other\\x"
        } else {
            "/other/x"
        });
        assert_eq!(relative_inside(&root, &elsewhere), None);
        assert_eq!(relative_inside(&root, Path::new("relative/x")), None);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_path_through_a_symlinked_directory_is_refused_and_a_link_leaf_is_not() {
        let scratch = Scratch::new("ancestors");
        let root = scratch.dir("root");
        let outside = scratch.dir("outside");
        fs::create_dir_all(root.join("real").join("deeper")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("real").join("inner-link")).unwrap();

        assert!(ancestors_are_real_directories(
            &root,
            Path::new("real/deeper/f.txt")
        ));
        // The leaf may be a link: it is the entry, and is not resolved.
        assert!(ancestors_are_real_directories(&root, Path::new("link")));
        assert!(ancestors_are_real_directories(
            &root,
            Path::new("real/inner-link")
        ));
        // A link in the middle, at any depth, is a road out.
        assert!(!ancestors_are_real_directories(
            &root,
            Path::new("link/secret.txt")
        ));
        assert!(!ancestors_are_real_directories(
            &root,
            Path::new("real/inner-link/secret.txt")
        ));
        // A vanished ancestor is not a link.
        assert!(ancestors_are_real_directories(
            &root,
            Path::new("gone/f.txt")
        ));
    }

    #[test]
    fn ori_t_0054_git_internals_are_not_reported_as_edits() {
        let scratch = Scratch::new("gitignore");
        let root = scratch.dir("root");
        fs::create_dir_all(root.join(".git").join("refs").join("heads")).unwrap();
        fs::create_dir_all(root.join("vendor").join("sub").join(".git")).unwrap();
        fs::create_dir_all(root.join(".GIT")).unwrap();
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();

        fs::write(root.join(".git").join("index"), b"i").unwrap();
        fs::write(
            root.join(".git").join("refs").join("heads").join("main"),
            b"abc",
        )
        .unwrap();
        fs::write(root.join(".git").join("COMMIT_EDITMSG"), b"msg").unwrap();
        fs::write(
            root.join("vendor").join("sub").join(".git").join("HEAD"),
            b"h",
        )
        .unwrap();
        fs::write(root.join(".GIT").join("config"), b"c").unwrap();
        // The control: a real edit, so the silence above is a finding.
        fs::write(root.join("real.txt"), b"inside").unwrap();
        let seen = collect_until(
            &mut watcher,
            "real.txt",
            at("real.txt", ChangeKind::Created),
        );
        std::thread::sleep(Duration::from_millis(200));
        let mut paths = seen.paths();
        let mut ignored = seen.ignored;
        if let Some(batch) = watcher.next_batch(Duration::from_millis(300)).unwrap() {
            ignored += batch.ignored;
            paths.extend(batch.changes.iter().map(|c| c.path_text().unwrap()));
        }
        assert!(
            paths.iter().all(|path| !path
                .to_ascii_lowercase()
                .split('/')
                .any(|part| part == ".git")),
            "a .git path was reported as an edit: {paths:?}"
        );
        assert!(
            ignored >= 1,
            "the watcher saw no .git event to ignore, so this test proved nothing: {seen:?}"
        );
    }

    #[test]
    fn ori_t_0054_build_output_is_not_reported_but_a_nested_target_directory_is() {
        let scratch = Scratch::new("buildout");
        let root = scratch.dir("root");
        fs::create_dir_all(root.join("target").join("debug")).unwrap();
        fs::create_dir_all(root.join("ui").join("node_modules").join("left-pad")).unwrap();
        fs::create_dir_all(root.join("java").join("target")).unwrap();
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();

        fs::write(root.join("target").join("debug").join("out.o"), b"o").unwrap();
        fs::write(
            root.join("ui")
                .join("node_modules")
                .join("left-pad")
                .join("index.js"),
            b"j",
        )
        .unwrap();
        fs::write(
            root.join("java").join("target").join("Main.java"),
            b"class Main {}",
        )
        .unwrap();
        let seen = collect_until(&mut watcher, "java/target/Main.java", |change| {
            change.path_text().as_deref() == Some("java/target/Main.java")
        });
        std::thread::sleep(Duration::from_millis(200));
        let mut paths = seen.paths();
        if let Some(batch) = watcher.next_batch(Duration::from_millis(300)).unwrap() {
            paths.extend(batch.changes.iter().map(|c| c.path_text().unwrap()));
        }
        assert!(
            paths
                .iter()
                .all(|p| !p.starts_with("target/") && !p.contains("node_modules")),
            "build output was reported: {paths:?}"
        );
    }

    #[test]
    fn ori_t_0054_a_burst_of_writes_to_one_file_coalesces_to_one_change() {
        let scratch = Scratch::new("burst");
        let root = scratch.dir("root");
        fs::write(root.join("hot.txt"), b"0").unwrap();
        let config = WatchConfig {
            quiet: Duration::from_millis(250),
            max_hold: Duration::from_secs(3),
            ..WatchConfig::default()
        };
        let mut watcher = TreeWatcher::watch_with(&root, config).unwrap();

        let writes = 200usize;
        for round in 0..writes {
            fs::write(root.join("hot.txt"), format!("{round}")).unwrap();
        }
        let seen = collect_until(&mut watcher, "hot.txt", at("hot.txt", ChangeKind::Modified));
        let hot: Vec<&Change> = seen
            .changes
            .iter()
            .filter(|change| change.path == Path::new("hot.txt"))
            .collect();
        // Each batch holds a path once by construction; the burst must not
        // come out as one report per write.
        assert!(
            hot.len() < writes / 10,
            "{writes} writes produced {} reports across {} batches",
            hot.len(),
            seen.batches
        );
        assert!(!hot.is_empty());
    }

    #[test]
    fn ori_t_0054_a_directory_that_appears_is_scanned_for_the_files_already_in_it() {
        let scratch = Scratch::new("scan");
        let root = scratch.dir("root");
        // A directory that already holds files by the time the consumer looks,
        // which is the case the backend's late watch registration creates.
        let fresh = root.join("fresh");
        fs::create_dir_all(fresh.join("nested").join("deeper")).unwrap();
        fs::create_dir_all(fresh.join("node_modules")).unwrap();
        fs::write(fresh.join("top.txt"), b"1").unwrap();
        fs::write(fresh.join("nested").join("mid.txt"), b"2").unwrap();
        fs::write(fresh.join("nested").join("deeper").join("low.txt"), b"3").unwrap();
        fs::write(fresh.join("node_modules").join("skipped.js"), b"4").unwrap();
        #[cfg(unix)]
        {
            let outside = scratch.dir("outside");
            fs::write(outside.join("far.txt"), b"5").unwrap();
            std::os::unix::fs::symlink(&outside, fresh.join("link")).unwrap();
        }

        let rules = WatchConfig::default().ignore;
        let mut accumulator = Accumulator::new(&root, &rules);
        accumulator.push(Raw::Change {
            path: fresh.clone(),
            kind: ChangeKind::Created,
            at: Timestamp::from_millis(7),
        });
        let batch = accumulator.finish();
        let mut paths: Vec<String> = batch
            .changes
            .iter()
            .map(|change| change.path_text().unwrap())
            .collect();
        paths.sort();
        let mut expected = vec![
            "fresh/nested/deeper/low.txt".to_owned(),
            "fresh/nested/mid.txt".to_owned(),
            "fresh/top.txt".to_owned(),
        ];
        // The link is an entry of its own, reported and never walked through.
        #[cfg(unix)]
        expected.push("fresh/link".to_owned());
        expected.sort();
        assert_eq!(paths, expected);
        assert!(batch.changes.iter().all(|c| c.kind == ChangeKind::Created));
        assert!(
            batch.ignored >= 1,
            "node_modules inside the new directory is ignored"
        );
        assert!(batch.missed.is_none());
    }

    #[test]
    fn ori_t_0054_a_scan_that_runs_out_of_budget_says_so_instead_of_stopping_quietly() {
        let scratch = Scratch::new("scanbudget");
        let root = scratch.dir("root");
        let crowded = root.join("crowded");
        fs::create_dir_all(&crowded).unwrap();
        for index in 0..(NEW_DIRECTORY_SCAN_ENTRY_CAP + 50) {
            fs::write(crowded.join(format!("f{index}")), b"").unwrap();
        }
        let rules = WatchConfig::default().ignore;
        let mut accumulator = Accumulator::new(&root, &rules);
        accumulator.push(Raw::Change {
            path: crowded,
            kind: ChangeKind::Created,
            at: Timestamp::from_millis(1),
        });
        let batch = accumulator.finish();
        let missed = batch.missed.expect("an exhausted scan is recorded");
        assert!(matches!(
            missed.reasons.first(),
            Some(MissedReason::ScanBudget { directory }) if directory == Path::new("crowded")
        ));
        assert!(batch.changes.len() <= NEW_DIRECTORY_SCAN_ENTRY_CAP);
    }

    #[test]
    fn ori_t_0054_kinds_fold_the_way_the_module_doc_says() {
        use ChangeKind::{Created, Modified, Removed};
        let table = [
            (Created, Created, Some(Created)),
            (Created, Modified, Some(Created)),
            (Created, Removed, None),
            (Modified, Modified, Some(Modified)),
            (Modified, Created, Some(Modified)),
            (Modified, Removed, Some(Removed)),
            (Removed, Created, Some(Modified)),
            (Removed, Modified, Some(Modified)),
            (Removed, Removed, Some(Removed)),
        ];
        for (previous, next, expected) in table {
            assert_eq!(
                merge_kinds(previous, next),
                expected,
                "{previous:?} then {next:?}"
            );
        }
    }

    #[test]
    fn ori_t_0054_a_path_created_and_removed_in_one_window_is_counted_not_silently_dropped() {
        let root = PathBuf::from(if cfg!(windows) {
            "C:\\work\\root"
        } else {
            "/work/root"
        });
        let rules = WatchConfig::default().ignore;
        let mut accumulator = Accumulator::new(&root, &rules);
        let stamp = Timestamp::from_millis(5);
        // The files never exist on disk; the fold is what is under test.
        accumulator.add(PathBuf::from("tmp.swp"), ChangeKind::Created, stamp);
        accumulator.add(PathBuf::from("tmp.swp"), ChangeKind::Removed, stamp);
        accumulator.add(PathBuf::from("kept.txt"), ChangeKind::Created, stamp);
        let batch = accumulator.finish();
        assert_eq!(batch.transient, 1);
        assert_eq!(batch.changes.len(), 1);
        assert_eq!(batch.changes[0].path, Path::new("kept.txt"));
    }

    #[test]
    fn ori_t_0054_the_latency_from_a_write_to_a_reported_change_is_bounded() {
        let scratch = Scratch::new("latency");
        let root = scratch.dir("root");
        // The defaults: this is the figure the engine will get.
        let mut watcher = TreeWatcher::watch(&root).unwrap();
        let samples = 7usize;
        let mut millis = Vec::new();
        for round in 0..samples {
            let name = format!("file-{round}.txt");
            let started = Instant::now();
            fs::write(root.join(&name), b"edit").unwrap();
            let deadline = started + PATIENCE;
            loop {
                assert!(
                    Instant::now() < deadline,
                    "no report of {name} within {PATIENCE:?}"
                );
                let batch = watcher.next_batch(Duration::from_millis(250)).unwrap();
                if batch.is_some_and(|batch| {
                    batch
                        .changes
                        .iter()
                        .any(|change| change.path == Path::new(&name))
                }) {
                    break;
                }
            }
            millis.push(started.elapsed().as_millis());
        }
        millis.sort_unstable();
        let median = millis[millis.len() / 2];
        let worst = millis[millis.len() - 1];
        eprintln!(
            "ORI-T-0054 latency, write to reported change, defaults (quiet {:?}, max hold {:?}): \
             samples {millis:?} ms, median {median} ms, worst {worst} ms",
            DEFAULT_QUIET, DEFAULT_MAX_HOLD
        );
        // ORI-P1-014 allows two seconds for the whole path from a disk edit
        // to an incident. The watcher's share is half of that for the median
        // (it adds the quiet window, 100 ms, plus the operating system's
        // delivery) and the whole of it for the worst sample, which absorbs a
        // loaded machine's scheduling.
        assert!(
            median < 1_000,
            "median latency {median} ms exceeds half of ORI-P1-014's two seconds: {millis:?}"
        );
        assert!(
            worst < 2_000,
            "worst latency {worst} ms exceeds ORI-P1-014's two seconds: {millis:?}"
        );
    }

    #[test]
    fn ori_t_0054_an_overflow_notice_is_recorded_as_may_have_missed_changes_until_reconciled() {
        let scratch = Scratch::new("overflow");
        let root = scratch.dir("root");
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();
        assert_eq!(watcher.health(), Health::Watching);

        // What notify sends on an inotify queue overflow.
        let notice = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        let raws = translate(
            Ok(notice),
            Timestamp::from_millis(9),
            &mut RenameTracking::default(),
        );
        assert_eq!(
            raws.len(),
            1,
            "an overflow notice becomes exactly one record"
        );
        for raw in raws {
            watcher.inject(raw);
        }
        let batch = watcher
            .next_batch(PATIENCE)
            .unwrap()
            .expect("a recorded overflow is delivered, not swallowed");
        let missed = batch.missed.expect("the batch carries the missed state");
        assert_eq!(missed.reasons, vec![MissedReason::Overflow]);
        assert_eq!(missed.first_observed_at, Timestamp::from_millis(9));
        assert!(batch.changes.is_empty());

        // It stays recorded until the engine reconciles, past an empty wait.
        assert!(
            watcher
                .next_batch(Duration::from_millis(50))
                .unwrap()
                .is_none()
        );
        assert!(matches!(watcher.health(), Health::MayHaveMissedChanges(_)));
        let token = watcher.begin_reconcile();
        assert!(watcher.mark_reconciled(token));
        assert_eq!(watcher.health(), Health::Watching);
    }

    #[test]
    fn ori_t_0054_a_loss_recorded_after_a_reconciliation_began_is_not_cleared_by_it() {
        let scratch = Scratch::new("generation");
        let root = scratch.dir("root");
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();
        watcher.inject(Raw::Missed(
            MissedReason::Overflow,
            Timestamp::from_millis(9),
        ));
        watcher
            .next_batch(PATIENCE)
            .unwrap()
            .expect("the first loss");

        // The engine begins to compare the tree with git.
        let token = watcher.begin_reconcile();
        // While it compares, the backend loses events again.
        watcher.inject(Raw::Missed(
            MissedReason::BackendError {
                message: "later".to_owned(),
            },
            Timestamp::from_millis(10),
        ));
        watcher
            .next_batch(PATIENCE)
            .unwrap()
            .expect("the second loss");

        // The comparison that began before the second loss cannot vouch for it.
        assert!(!watcher.mark_reconciled(token));
        match watcher.health() {
            Health::MayHaveMissedChanges(missed) => assert_eq!(missed.reasons.len(), 2),
            other => panic!("the second loss was cleared: {other:?}"),
        }
        // A comparison that began after it can.
        let token = watcher.begin_reconcile();
        assert!(watcher.mark_reconciled(token));
        assert_eq!(watcher.health(), Health::Watching);
    }

    #[test]
    fn ori_t_0054_a_dead_backend_is_reported_as_stopped_never_as_a_quiet_tree() {
        let scratch = Scratch::new("dead");
        let root = scratch.dir("root");
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();
        // Alive and quiet is a timeout, not an error.
        assert!(
            watcher
                .next_batch(Duration::from_millis(100))
                .unwrap()
                .is_none()
        );
        assert_eq!(watcher.health(), Health::Watching);

        // The backend's thread dies while handling the next event, as it does
        // if notify panics in it.
        watcher.kill_backend_thread();
        fs::write(root.join("trigger.txt"), b"x").unwrap();
        let deadline = Instant::now() + PATIENCE;
        let error = loop {
            assert!(
                Instant::now() < deadline,
                "the backend's thread died and next_batch kept answering as for a quiet tree"
            );
            match watcher.next_batch(Duration::from_millis(200)) {
                Ok(_) => {}
                Err(error) => break error,
            }
        };
        assert!(matches!(error, WatchError::Stopped), "{error}");
        assert_eq!(watcher.health(), Health::Stopped);
        // It stays stopped, and cannot be reconciled back to healthy.
        assert!(matches!(
            watcher.next_batch(Duration::from_millis(50)),
            Err(WatchError::Stopped)
        ));
        let token = watcher.begin_reconcile();
        assert!(!watcher.mark_reconciled(token));

        // notify's own drop unwraps a send to the dead thread and panics; ours
        // must not let that out.
        let dropped = catch_unwind(AssertUnwindSafe(move || drop(watcher)));
        assert!(
            dropped.is_ok(),
            "dropping a watcher whose backend died panicked in the caller"
        );
    }

    #[test]
    fn ori_t_0054_a_backend_error_is_recorded_as_may_have_missed_changes() {
        let error = notify::Error::generic("inotify watch limit reached");
        let raws = translate(
            Err(error),
            Timestamp::from_millis(3),
            &mut RenameTracking::default(),
        );
        assert_eq!(raws.len(), 1);
        match &raws[0] {
            Raw::Missed(MissedReason::BackendError { message }, at) => {
                assert!(message.contains("watch limit"), "{message}");
                assert_eq!(*at, Timestamp::from_millis(3));
            }
            other => panic!("a backend error must become a missed record, got {other:?}"),
        }
    }

    #[test]
    fn ori_t_0054_a_queue_that_filled_is_recorded_as_dropped_events_not_lost_silently() {
        let scratch = Scratch::new("queuefull");
        let root = scratch.dir("root");
        let config = WatchConfig {
            queue_capacity: 2,
            ..quick()
        };
        let mut watcher = TreeWatcher::watch_with(&root, config).unwrap();
        // Produce far more events than the queue holds while nobody drains it.
        for round in 0..60 {
            fs::write(root.join(format!("f{round}.txt")), b"x").unwrap();
        }
        let deadline = Instant::now() + PATIENCE;
        let mut dropped = None;
        while dropped.is_none() {
            assert!(
                Instant::now() < deadline,
                "60 writes into a queue of 2 never produced a QueueFull record"
            );
            if let Some(batch) = watcher.next_batch(Duration::from_millis(300)).unwrap() {
                dropped = batch.missed.and_then(|missed| {
                    missed.reasons.into_iter().find_map(|reason| match reason {
                        MissedReason::QueueFull { dropped } => Some(dropped),
                        _ => None,
                    })
                });
            }
        }
        assert!(dropped.unwrap() > 0);
        assert!(matches!(watcher.health(), Health::MayHaveMissedChanges(_)));
    }

    #[test]
    fn ori_t_0054_the_removal_of_the_root_is_recorded_as_a_missed_state() {
        let scratch = Scratch::new("rootgone");
        let root = scratch.dir("root");
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();
        fs::remove_dir_all(&root).unwrap();
        let deadline = Instant::now() + PATIENCE;
        loop {
            assert!(
                Instant::now() < deadline,
                "removing the watched root was never recorded"
            );
            let batch = watcher.next_batch(Duration::from_millis(300)).unwrap();
            if batch
                .and_then(|batch| batch.missed)
                .is_some_and(|missed| missed.reasons.contains(&MissedReason::RootGone))
            {
                break;
            }
        }
        assert!(matches!(watcher.health(), Health::MayHaveMissedChanges(_)));
    }

    #[test]
    fn ori_t_0054_a_root_that_is_missing_or_not_a_directory_is_refused() {
        let scratch = Scratch::new("badroot");
        let missing = scratch.path.join("nope");
        assert!(matches!(
            TreeWatcher::watch(&missing),
            Err(WatchError::Root { .. })
        ));
        let file = scratch.path.join("file.txt");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(
            TreeWatcher::watch(&file),
            Err(WatchError::RootNotADirectory { .. })
        ));
    }

    #[test]
    fn ori_t_0054_a_root_spelled_with_dotdot_is_canonicalised_before_paths_are_judged() {
        let scratch = Scratch::new("dotdotroot");
        let root = scratch.dir("root");
        fs::create_dir_all(root.join("sub")).unwrap();
        let spelled = root.join("sub").join("..");
        let mut watcher = TreeWatcher::watch_with(&spelled, quick()).unwrap();
        assert_eq!(watcher.root(), root.as_path());
        fs::write(root.join("x.txt"), b"x").unwrap();
        collect_until(&mut watcher, "x.txt", at("x.txt", ChangeKind::Created));
    }

    #[test]
    fn ori_t_0054_the_ignore_rules_match_by_component_not_by_substring() {
        let rules = WatchConfig::default().ignore;
        assert!(is_ignored(Path::new(".git/index"), &rules));
        assert!(is_ignored(Path::new("a/.GIT/config"), &rules));
        assert!(is_ignored(Path::new(".git"), &rules));
        assert!(is_ignored(Path::new("target/debug/x"), &rules));
        assert!(is_ignored(Path::new("a/node_modules/b"), &rules));
        // Names that merely contain an ignored one are real files.
        assert!(!is_ignored(Path::new(".gitignore"), &rules));
        assert!(!is_ignored(Path::new(".github/workflows/ci.yml"), &rules));
        assert!(!is_ignored(Path::new("targets/x"), &rules));
        assert!(!is_ignored(Path::new("a/target/x"), &rules));
        assert!(!is_ignored(Path::new("my.git/x"), &rules));
    }

    // ---- the ref poller ----

    const OID_A: &str = "1111111111111111111111111111111111111111";
    const OID_B: &str = "2222222222222222222222222222222222222222";
    const OID_C: &str = "3333333333333333333333333333333333333333";

    /// A bare-bones `.git` layout, enough for the poller.
    fn fake_repo(scratch: &Scratch) -> PathBuf {
        let repo = scratch.dir("repo");
        fs::create_dir_all(repo.join(".git").join("refs").join("heads")).unwrap();
        fs::create_dir_all(
            repo.join(".git")
                .join("refs")
                .join("remotes")
                .join("origin"),
        )
        .unwrap();
        fs::write(repo.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            repo.join(".git").join("refs").join("heads").join("main"),
            format!("{OID_A}\n"),
        )
        .unwrap();
        repo
    }

    fn head_ref(repo: &Path, name: &str) -> PathBuf {
        repo.join(".git").join("refs").join("heads").join(name)
    }

    #[test]
    fn ori_t_0054_a_ref_poll_reports_a_branch_that_moved_appeared_and_vanished() {
        let scratch = Scratch::new("refs");
        let repo = fake_repo(&scratch);
        let mut poller = RefPoller::open(&repo).unwrap();

        // Nothing changed since the baseline.
        let quiet = poller.poll().unwrap();
        assert!(quiet.changes.is_empty(), "{quiet:?}");

        fs::write(head_ref(&repo, "main"), format!("{OID_B}\n")).unwrap();
        fs::write(head_ref(&repo, "feature"), format!("{OID_C}\n")).unwrap();
        fs::write(
            repo.join(".git")
                .join("refs")
                .join("remotes")
                .join("origin")
                .join("main"),
            format!("{OID_B}\n"),
        )
        .unwrap();
        let poll = poller.poll().unwrap();
        let by_name: BTreeMap<&str, &RefChange> =
            poll.changes.iter().map(|c| (c.name.as_str(), c)).collect();
        let moved = by_name["refs/heads/main"];
        assert_eq!(moved.before.as_deref(), Some(OID_A));
        assert_eq!(moved.after.as_deref(), Some(OID_B));
        assert_eq!(moved.class(), RefClass::Branch);
        let appeared = by_name["refs/heads/feature"];
        assert_eq!(appeared.before, None);
        assert_eq!(appeared.after.as_deref(), Some(OID_C));
        assert_eq!(
            by_name["refs/remotes/origin/main"].class(),
            RefClass::RemoteTracking
        );
        assert_eq!(poll.changes.len(), 3, "{:?}", poll.changes);

        fs::remove_file(head_ref(&repo, "feature")).unwrap();
        let gone = poller.poll().unwrap();
        assert_eq!(gone.changes.len(), 1);
        assert_eq!(gone.changes[0].name, "refs/heads/feature");
        assert_eq!(gone.changes[0].after, None);

        // And it is quiet again.
        assert!(poller.poll().unwrap().changes.is_empty());
    }

    #[test]
    fn ori_t_0054_a_ref_poll_reads_head_and_packed_refs_and_a_loose_ref_overrides_a_packed_one() {
        let scratch = Scratch::new("packed");
        let repo = fake_repo(&scratch);
        fs::write(
            repo.join(".git").join("packed-refs"),
            format!(
                "# pack-refs with: peeled fully-peeled sorted \n{OID_A} refs/heads/packed\n\
                 {OID_B} refs/tags/v1\n^{OID_C}\n{OID_A} refs/heads/main\n"
            ),
        )
        .unwrap();
        let mut poller = RefPoller::open(&repo).unwrap();
        assert!(poller.poll().unwrap().changes.is_empty());

        // The loose `main` (OID_A) and the packed `main` (OID_A) agree; move
        // the loose one and it wins over the packed value.
        fs::write(head_ref(&repo, "main"), format!("{OID_C}\n")).unwrap();
        let poll = poller.poll().unwrap();
        assert_eq!(poll.changes.len(), 1, "{:?}", poll.changes);
        assert_eq!(poll.changes[0].name, "refs/heads/main");
        assert_eq!(poll.changes[0].after.as_deref(), Some(OID_C));

        // HEAD is a ref too: switching branches is visible.
        fs::write(repo.join(".git").join("HEAD"), "ref: refs/heads/packed\n").unwrap();
        let switched = poller.poll().unwrap();
        assert_eq!(switched.changes.len(), 1);
        assert_eq!(switched.changes[0].name, "HEAD");
        assert_eq!(switched.changes[0].class(), RefClass::Head);
        assert_eq!(
            switched.changes[0].after.as_deref(),
            Some("ref: refs/heads/packed")
        );
    }

    #[test]
    fn ori_t_0054_a_ref_poll_ignores_git_lock_files() {
        let scratch = Scratch::new("reflock");
        let repo = fake_repo(&scratch);
        let mut poller = RefPoller::open(&repo).unwrap();
        fs::write(head_ref(&repo, "main.lock"), format!("{OID_B}\n")).unwrap();
        assert!(poller.poll().unwrap().changes.is_empty());
    }

    #[test]
    fn ori_t_0054_a_ref_poll_refuses_a_git_dir_that_is_not_a_plain_directory() {
        let scratch = Scratch::new("gitfile");
        let repo = scratch.dir("repo");
        fs::write(repo.join(".git"), "gitdir: /somewhere/else\n").unwrap();
        let error = RefPoller::open(&repo).unwrap_err();
        assert!(
            matches!(error, WatchError::GitDirNotADirectory { .. }),
            "{error}"
        );
        assert_eq!(error.methodology_ref().unwrap().section, 13);
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_ref_poll_refuses_a_symlinked_git_dir_and_skips_a_symlinked_ref() {
        let scratch = Scratch::new("reflink");
        let elsewhere = scratch.dir("elsewhere");
        let linked = scratch.dir("linked");
        std::os::unix::fs::symlink(&elsewhere, linked.join(".git")).unwrap();
        assert!(matches!(
            RefPoller::open(&linked),
            Err(WatchError::GitDirNotADirectory { .. })
        ));

        let repo = fake_repo(&scratch);
        let secret = scratch.path.join("secret.txt");
        fs::write(&secret, "do not read me\n").unwrap();
        let mut poller = RefPoller::open(&repo).unwrap();
        std::os::unix::fs::symlink(&secret, head_ref(&repo, "trick")).unwrap();
        let poll = poller.poll().unwrap();
        assert!(poll.changes.is_empty(), "{:?}", poll.changes);
        assert_eq!(poll.skipped.len(), 1);
        assert_eq!(poll.skipped[0].reason, SkippedRefReason::Symlink);
    }

    #[test]
    fn ori_t_0054_a_ref_poll_refuses_reftable_storage_rather_than_polling_nothing() {
        let scratch = Scratch::new("reftable");
        let repo = fake_repo(&scratch);
        fs::create_dir_all(repo.join(".git").join("reftable")).unwrap();
        let error = RefPoller::open(&repo).unwrap_err();
        assert!(
            matches!(error, WatchError::UnsupportedRefStorage { .. }),
            "{error}"
        );
        assert_eq!(error.methodology_ref().unwrap().section, 14);
    }

    #[test]
    fn ori_t_0054_a_ref_poll_sees_what_real_git_writes_loose_then_packed() {
        let scratch = Scratch::new("realgit");
        let repo = scratch.dir("repo");
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args([
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "init.defaultBranch=main",
                ])
                .args(args)
                .current_dir(&repo)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("HOME", &scratch.path)
                .env("XDG_CONFIG_HOME", &scratch.path)
                .output()
                .expect("git can be run");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q"]);
        fs::write(repo.join("a.txt"), "a").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "first"]);
        let mut poller = RefPoller::open(&repo).unwrap();

        fs::write(repo.join("a.txt"), "b").unwrap();
        git(&["commit", "-q", "-am", "second"]);
        let loose = poller.poll().unwrap();
        let names: Vec<&str> = loose.changes.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"refs/heads/main"), "{names:?}");
        let main = loose
            .changes
            .iter()
            .find(|c| c.name == "refs/heads/main")
            .unwrap();
        assert_ne!(main.before, main.after);
        assert!(
            matches!(main.after.as_ref().map(String::len), Some(40 | 64)),
            "an object id: {:?}",
            main.after
        );

        // After packing, the loose files are gone and nothing has moved: the
        // poller must read `packed-refs` and report no change at all.
        git(&["pack-refs", "--all", "--prune"]);
        let packed = poller.poll().unwrap();
        assert!(
            packed.changes.is_empty(),
            "packing refs moved nothing, yet the poll reported {:?}",
            packed.changes
        );
    }
    // ---- the review round: links, pipes, renames, ordering ----

    /// Runs `work` on its own thread and fails, rather than hangs, if it does
    /// not finish: a call that blocks for ever must be a failing test.
    fn within<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{what} did not return within 5 s: it blocked"))
    }

    #[cfg(unix)]
    fn mkfifo(path: &Path) {
        let status = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .expect("mkfifo can be run");
        assert!(status.success(), "mkfifo {path:?} failed");
    }

    /// A file with something a user would not want in an event, standing in
    /// for a `.env` line.
    const SECRET: &str = "API_KEY=hunter2";

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_ref_poll_never_reads_through_a_head_that_is_a_link() {
        let scratch = Scratch::new("headlink");
        let repo = fake_repo(&scratch);
        let secret = scratch.path.join("secret.env");
        fs::write(&secret, format!("{SECRET}\nmore\n")).unwrap();
        let head = repo.join(".git").join("HEAD");

        // At open.
        fs::remove_file(&head).unwrap();
        std::os::unix::fs::symlink(&secret, &head).unwrap();
        let error = RefPoller::open(&repo).unwrap_err();
        assert!(
            matches!(error, WatchError::RefStorageNotPlain { .. }),
            "{error}"
        );
        assert_eq!(error.methodology_ref().unwrap().section, 13);
        assert!(!format!("{error:?} {error}").contains("hunter2"));

        // After a good baseline, when HEAD is swapped for a link.
        fs::remove_file(&head).unwrap();
        fs::write(&head, "ref: refs/heads/main\n").unwrap();
        let mut poller = RefPoller::open(&repo).unwrap();
        fs::remove_file(&head).unwrap();
        std::os::unix::fs::symlink(&secret, &head).unwrap();
        let error = poller.poll().unwrap_err();
        assert!(
            matches!(error, WatchError::RefStorageNotPlain { .. }),
            "{error}"
        );
        assert!(!format!("{error:?} {error}").contains("hunter2"));
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_ref_poll_never_walks_a_refs_directory_that_is_a_link() {
        let scratch = Scratch::new("refslink");
        let repo = fake_repo(&scratch);
        let elsewhere = scratch.dir("elsewhere");
        fs::write(elsewhere.join("secret"), format!("{SECRET}\n")).unwrap();
        fs::write(elsewhere.join("other"), "second file\n").unwrap();

        let mut poller = RefPoller::open(&repo).unwrap();
        let refs = repo.join(".git").join("refs");
        fs::remove_dir_all(&refs).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &refs).unwrap();
        let error = poller.poll().unwrap_err();
        assert!(
            matches!(error, WatchError::RefStorageNotPlain { .. }),
            "{error}"
        );
        assert!(!format!("{error:?} {error}").contains("hunter2"));

        // And at open.
        let error = RefPoller::open(&repo).unwrap_err();
        assert!(
            matches!(error, WatchError::RefStorageNotPlain { .. }),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_ref_poll_does_not_block_on_a_pipe_in_the_place_of_a_ref_file() {
        let scratch = Scratch::new("refpipe");
        let repo = fake_repo(&scratch);
        let git_dir = repo.join(".git");

        // HEAD is a pipe: open(2) with no writer would wait for ever.
        let head = git_dir.join("HEAD");
        fs::remove_file(&head).unwrap();
        mkfifo(&head);
        let repo_for_head = repo.clone();
        let error = within("opening a poller over a pipe at HEAD", move || {
            RefPoller::open(&repo_for_head)
        })
        .unwrap_err();
        assert!(
            matches!(error, WatchError::RefStorageNotPlain { .. }),
            "{error}"
        );
        fs::remove_file(&head).unwrap();
        fs::write(&head, "ref: refs/heads/main\n").unwrap();

        // packed-refs and a loose ref are pipes: skipped, said so, not waited on.
        mkfifo(&git_dir.join("packed-refs"));
        mkfifo(&head_ref(&repo, "piped"));
        let poll = within("polling over pipes in the refs", move || {
            RefPoller::open(&repo)?.poll()
        })
        .unwrap();
        let reasons: Vec<&SkippedRefReason> = poll.skipped.iter().map(|s| &s.reason).collect();
        assert_eq!(reasons.len(), 2, "{poll:?}");
        assert!(
            reasons
                .iter()
                .all(|r| **r == SkippedRefReason::NotARegularFile)
        );
    }

    #[test]
    fn ori_t_0054_a_pack_refs_between_the_two_reads_does_not_make_a_branch_vanish() {
        let scratch = Scratch::new("packrace");
        let repo = fake_repo(&scratch);
        fs::write(head_ref(&repo, "stable"), format!("{OID_B}\n")).unwrap();
        let poller = RefPoller::open(&repo).unwrap();

        // `git pack-refs --all --prune` runs in the gap: it writes the new
        // packed-refs, then deletes the loose files.
        let git_dir = repo.join(".git");
        let repo_for_hook = repo.clone();
        let snapshot = poller
            .snapshot_with(&mut || {
                fs::write(
                    git_dir.join("packed-refs"),
                    format!("{OID_A} refs/heads/main\n{OID_B} refs/heads/stable\n"),
                )
                .unwrap();
                fs::remove_file(head_ref(&repo_for_hook, "main")).unwrap();
                fs::remove_file(head_ref(&repo_for_hook, "stable")).unwrap();
            })
            .unwrap();
        assert_eq!(
            snapshot.refs.get("refs/heads/stable").map(String::as_str),
            Some(OID_B),
            "a branch that was only packed mid-poll looked deleted: {:?}",
            snapshot.refs
        );
        assert_eq!(
            snapshot.refs.get("refs/heads/main").map(String::as_str),
            Some(OID_A)
        );
    }

    #[cfg(unix)]
    #[test]
    fn ori_t_0054_a_directory_swapped_for_a_link_before_its_scan_is_not_listed() {
        let scratch = Scratch::new("scanlink");
        let root = scratch.dir("root");
        let outside = scratch.dir("outside");
        fs::write(outside.join("leaked.txt"), b"x").unwrap();
        fs::create_dir_all(outside.join("b")).unwrap();
        fs::write(outside.join("b").join("deep.txt"), b"x").unwrap();
        fs::create_dir_all(root.join("x")).unwrap();
        fs::create_dir_all(root.join("a").join("b")).unwrap();
        let rules = WatchConfig::default().ignore;
        let mut accumulator = Accumulator::new(&root, &rules);
        for path in [root.join("x"), root.join("a").join("b")] {
            accumulator.push(Raw::Change {
                path,
                kind: ChangeKind::Created,
                at: Timestamp::from_millis(1),
            });
        }
        // In the window between the event and the scan: `x` is replaced by a
        // link out of the root, and `a` (an ancestor of `a/b`) likewise.
        fs::remove_dir(root.join("x")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("x")).unwrap();
        fs::remove_dir_all(root.join("a")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("a")).unwrap();

        let batch = accumulator.finish();
        let paths: Vec<String> = batch
            .changes
            .iter()
            .map(|change| change.path_text().unwrap())
            .collect();
        assert!(
            paths
                .iter()
                .all(|path| path != "x/leaked.txt" && !path.starts_with("a/b/")),
            "names from outside the root were listed as inside it: {paths:?}"
        );
        assert_eq!(batch.rejected, 2, "{batch:?}");
    }

    #[test]
    fn ori_t_0054_one_rename_is_counted_once_however_many_events_the_backend_sends() {
        let rename = |mode: RenameMode, paths: &[&str], tracker: usize| {
            let mut event =
                Event::new(EventKind::Modify(ModifyKind::Name(mode))).set_tracker(tracker);
            for path in paths {
                event = event.add_path(PathBuf::from(path));
            }
            event
        };
        let at = Timestamp::from_millis(1);

        // inotify: From, To, then Both, one tracker.
        let mut tracking = RenameTracking::default();
        let mut raws = Vec::new();
        raws.extend(translate(
            Ok(rename(RenameMode::From, &["/r/tmp"], 7)),
            at,
            &mut tracking,
        ));
        raws.extend(translate(
            Ok(rename(RenameMode::To, &["/r/target"], 7)),
            at,
            &mut tracking,
        ));
        raws.extend(translate(
            Ok(rename(RenameMode::Both, &["/r/tmp", "/r/target"], 7)),
            at,
            &mut tracking,
        ));
        assert_eq!(
            raws.len(),
            2,
            "one rename became {} records: {raws:?}",
            raws.len()
        );

        // A backend that sends only the combined event is still heard.
        let mut tracking = RenameTracking::default();
        let alone = translate(
            Ok(rename(RenameMode::Both, &["/r/old", "/r/new"], 9)),
            at,
            &mut tracking,
        );
        assert_eq!(alone.len(), 2);

        // A combined event for a different rename than the halves just seen is
        // not swallowed.
        let mut tracking = RenameTracking::default();
        let _ = translate(
            Ok(rename(RenameMode::From, &["/r/a"], 1)),
            at,
            &mut tracking,
        );
        let _ = translate(Ok(rename(RenameMode::To, &["/r/b"], 1)), at, &mut tracking);
        let other = translate(
            Ok(rename(RenameMode::Both, &["/r/c", "/r/d"], 2)),
            at,
            &mut tracking,
        );
        assert_eq!(other.len(), 2);
    }

    #[test]
    fn ori_t_0054_a_temporary_renamed_over_its_target_is_one_change_to_the_target_and_none_to_the_temporary()
     {
        let scratch = Scratch::new("sedi");
        let root = scratch.dir("root");
        fs::write(root.join("target.txt"), b"old").unwrap();
        let mut watcher = TreeWatcher::watch_with(&root, quick()).unwrap();

        // What `sed -i` and most editors do.
        fs::write(root.join(".target.txt.tmp"), b"new content").unwrap();
        fs::rename(root.join(".target.txt.tmp"), root.join("target.txt")).unwrap();
        let seen = collect_until(&mut watcher, "target.txt", |change| {
            change.path == Path::new("target.txt")
        });
        std::thread::sleep(Duration::from_millis(300));
        let mut paths = seen.paths();
        if let Some(batch) = watcher.next_batch(Duration::from_millis(300)).unwrap() {
            paths.extend(batch.changes.iter().map(|c| c.path_text().unwrap()));
        }
        assert!(
            paths.iter().all(|path| path != ".target.txt.tmp"),
            "the temporary was reported although it never outlived the save: {paths:?}"
        );
        assert!(seen.transient >= 1, "{seen:?}");
    }
}
