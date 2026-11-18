use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use kqueue::{EventData, EventFilter, FilterFlag, Ident, Vnode};
use notify::event::{DataChange, Flag, ModifyKind, RemoveKind};
use notify::{Error, ErrorKind, Event, EventHandler, EventKind, Result};

const FILTER: EventFilter = EventFilter::EVFILT_VNODE;
const VNODE_READY: mio::Token = mio::Token(0);
const COMMAND_READY: mio::Token = mio::Token(1);
const EVENTS_PER_DRAIN: usize = 128;
// Check the channel even if the poller cannot be woken during shutdown.
const POLL_TIMEOUT: Duration = Duration::from_secs(1);

type Registration = Vec<(PathBuf, Result<bool>)>;
type WatchResults = Result<Registration>;
type WatchSnapshot = (HashSet<PathBuf>, Vec<PathBuf>);

enum Command {
    Add(Vec<PathBuf>, bool, SyncSender<WatchResults>),
    Remove(PathBuf, SyncSender<()>),
    Snapshot(SyncSender<WatchSnapshot>),
}

pub(crate) struct FreeBsdWatcher {
    commands: Option<SyncSender<Command>>,
    wake: mio::Waker,
    worker: Option<JoinHandle<()>>,
}

impl FreeBsdWatcher {
    pub(super) fn new(
        handler: impl EventHandler,
        max_watches: usize,
        reserve: usize,
    ) -> Result<Self> {
        let queue = kqueue::Watcher::new()?;
        let poll = mio::Poll::new()?;
        let fd = queue.as_raw_fd();
        poll.registry().register(
            &mut mio::unix::SourceFd(&fd),
            VNODE_READY,
            mio::Interest::READABLE,
        )?;
        let wake = mio::Waker::new(poll.registry(), COMMAND_READY)?;
        let (send, receive) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("tokensave-kqueue".into())
            .spawn(move || {
                run(
                    queue,
                    poll,
                    &receive,
                    handler,
                    Watches::new(max_watches, reserve),
                )
            })?;
        Ok(Self {
            commands: Some(send),
            wake,
            worker: Some(worker),
        })
    }

    pub(super) fn watch_explicit(
        &mut self,
        paths: Vec<PathBuf>,
        directories: bool,
    ) -> WatchResults {
        self.request(|reply| Command::Add(paths, directories, reply))?
    }

    pub(super) fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.request(|reply| Command::Remove(path.to_path_buf(), reply))
    }

    pub(super) fn snapshot(&mut self) -> Result<WatchSnapshot> {
        self.request(Command::Snapshot)
    }

    fn request<T>(&mut self, command: impl FnOnce(SyncSender<T>) -> Command) -> Result<T> {
        let (send, receive) = mpsc::sync_channel(1);
        let channel = self
            .commands
            .as_ref()
            .ok_or_else(|| Error::generic("FreeBSD watcher stopped"))?;
        channel
            .try_send(command(send))
            .map_err(|error| Error::generic(&error.to_string()))?;
        let _ = self.wake.wake();
        receive
            .recv()
            .map_err(|error| Error::generic(&error.to_string()))
    }
}

impl Drop for FreeBsdWatcher {
    fn drop(&mut self) {
        drop(self.commands.take());
        let _ = self.wake.wake();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    mut queue: kqueue::Watcher,
    mut poll: mio::Poll,
    commands: &Receiver<Command>,
    mut handler: impl EventHandler,
    mut watches: Watches,
) {
    let mut events = mio::Events::with_capacity(2);
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Add(paths, directories, reply)) => {
                    match watches.add(&mut queue, paths, directories) {
                        Ok((registered, changed_during_registration)) => {
                            let _ = reply.send(Ok(registered));
                            if changed_during_registration {
                                handler.handle_event(Ok(
                                    Event::new(EventKind::Other).set_flag(Flag::Rescan)
                                ));
                            }
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error));
                        }
                    }
                }
                Ok(Command::Remove(path, reply)) => {
                    watches.remove(&mut queue, &path, true);
                    let _ = reply.send(());
                }
                Ok(Command::Snapshot(reply)) => {
                    let _ = reply.send(watches.snapshot(&mut queue));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        let mut drained = 0;
        while drained < EVENTS_PER_DRAIN {
            let Some(event) = queue.poll(None) else { break };
            drained += 1;
            if let Some(event) = watches.translate(&mut queue, event) {
                handler.handle_event(Ok(event));
            }
        }
        if drained == EVENTS_PER_DRAIN {
            continue;
        }
        if let Err(error) = poll.poll(&mut events, Some(POLL_TIMEOUT)) {
            if error.kind() != io::ErrorKind::Interrupted {
                handler.handle_event(Err(Error::io(error)));
                return;
            }
        }
    }
}
struct Watches {
    entries: BTreeMap<PathBuf, WatchEntry>,
    paths: HashMap<RawFd, PathBuf>,
    file_changes: HashSet<PathBuf>,
    file_changes_full: bool,
    max_watches: usize,
    reserve: usize,
}

struct WatchEntry {
    file: File,
    directory: bool,
}

impl Watches {
    fn new(max_watches: usize, reserve: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            paths: HashMap::new(),
            file_changes: HashSet::new(),
            file_changes_full: false,
            max_watches,
            reserve,
        }
    }

    fn add(
        &mut self,
        queue: &mut kqueue::Watcher,
        mut paths: Vec<PathBuf>,
        directories: bool,
    ) -> Result<(Registration, bool)> {
        paths.sort_unstable();
        paths.dedup();
        let mut available = available_fds(self.reserve)?;
        let mut added = Vec::new();
        let mut results = Vec::with_capacity(paths.len());
        for path in paths {
            let result = self.add_one(queue, &path, directories, &mut available);
            if matches!(result, Ok(true)) {
                added.push(results.len());
            }
            let result = result.map_err(|error| error.add_path(path.clone()));
            results.push((path, result));
        }

        if added.is_empty() {
            return Ok((results, false));
        }
        let armed = queue.watch();
        let mut changed = false;
        for index in added {
            let (path, result) = &mut results[index];
            let error = match &armed {
                Err(error) => Error::io(error.raw_os_error().map_or_else(
                    || io::Error::new(error.kind(), error.to_string()),
                    io::Error::from_raw_os_error,
                )),
                Ok(())
                    if self
                        .entries
                        .get(path)
                        .is_some_and(|entry| entry.is_current(path)) =>
                {
                    continue;
                }
                Ok(()) => {
                    changed = true;
                    if !directories {
                        self.record_file_change(path.parent());
                    }
                    Error::path_not_found()
                }
            };
            self.remove(queue, path, directories);
            *result = Err(error.add_path(path.clone()));
        }
        Ok((results, changed))
    }

    fn add_one(
        &mut self,
        queue: &mut kqueue::Watcher,
        path: &Path,
        directories: bool,
        available: &mut usize,
    ) -> Result<bool> {
        let expected = std::fs::metadata(path).map_err(Error::io_watch)?;
        if !expected_type(&expected, directories) {
            return Err(Error::path_not_found());
        }
        if let Some(old) = self.entries.get(path) {
            if old
                .file
                .metadata()
                .is_ok_and(|current| same_vnode(&current, &expected))
            {
                return Ok(false);
            }
            self.remove(queue, path, false);
            *available = available_fds(self.reserve)?;
        }
        if *available == 0 || self.entries.len() >= self.max_watches {
            return Err(Error::new(ErrorKind::MaxFilesWatch));
        }

        let flags = libc::O_NONBLOCK | if directories { libc::O_DIRECTORY } else { 0 };
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(path)
            .map_err(|error| {
                if matches!(error.raw_os_error(), Some(libc::EMFILE | libc::ENFILE)) {
                    *available = 0;
                }
                Error::io_watch(error)
            })?;
        let current = file.metadata().map_err(Error::io_watch)?;
        if !expected_type(&current, directories) || !same_vnode(&current, &expected) {
            return Err(Error::path_not_found());
        }
        let flags = FilterFlag::NOTE_DELETE
            | FilterFlag::NOTE_WRITE
            | FilterFlag::NOTE_EXTEND
            | FilterFlag::NOTE_ATTRIB
            | FilterFlag::NOTE_LINK
            | FilterFlag::NOTE_RENAME
            | FilterFlag::NOTE_REVOKE;
        queue.add_file(&file, FILTER, flags).map_err(Error::io)?;
        self.paths.insert(file.as_raw_fd(), path.to_path_buf());
        self.entries.insert(
            path.to_path_buf(),
            WatchEntry {
                file,
                directory: directories,
            },
        );
        *available -= 1;
        Ok(true)
    }

    fn remove(&mut self, queue: &mut kqueue::Watcher, path: &Path, subtree: bool) {
        if subtree {
            let children: Vec<_> = self
                .entries
                .range(path.to_path_buf()..)
                .take_while(|(child, _)| child.starts_with(path))
                .map(|(child, _)| child.clone())
                .collect();
            for child in children {
                self.remove(queue, &child, false);
            }
        } else if let Some(entry) = self.entries.remove(path) {
            self.paths.remove(&entry.file.as_raw_fd());
            // Closing the owned file also drops its filter if EV_DELETE failed.
            let _ = queue.remove_file(&entry.file, FILTER);
        }
    }

    fn record_file_change(&mut self, dir: Option<&Path>) {
        if let Some(dir) = dir {
            if self.file_changes.len() < self.max_watches {
                self.file_changes.insert(dir.to_path_buf());
            } else if !self.file_changes.contains(dir) {
                self.file_changes_full = true;
            }
        }
    }

    fn snapshot(&mut self, queue: &mut kqueue::Watcher) -> WatchSnapshot {
        let mut live = HashSet::new();
        let mut stale = Vec::new();
        for (path, entry) in self.entries.iter().filter(|(_, entry)| entry.directory) {
            if entry.is_current(path) {
                live.insert(path.clone());
            } else {
                stale.push(path.clone());
            }
        }
        for path in stale {
            self.remove(queue, &path, true);
        }
        live.retain(|path| self.entries.contains_key(path));
        let affected = if std::mem::take(&mut self.file_changes_full) {
            self.file_changes.clear();
            live.iter().cloned().collect()
        } else {
            self.file_changes
                .drain()
                .filter(|dir| live.contains(dir))
                .collect()
        };
        (live, affected)
    }

    fn translate(&mut self, queue: &mut kqueue::Watcher, event: kqueue::Event) -> Option<Event> {
        let kqueue::Event {
            ident: Ident::Fd(fd),
            data: EventData::Vnode(vnode),
        } = event
        else {
            return None;
        };
        let path = self.paths.get(&fd)?.clone();
        let directory = self.entries.get(&path)?.directory;
        let kind = match vnode {
            Vnode::Delete | Vnode::Rename | Vnode::Revoke => {
                if !directory {
                    self.record_file_change(path.parent());
                }
                self.remove(queue, &path, true);
                EventKind::Remove(RemoveKind::Any)
            }
            _ => {
                if directory {
                    self.record_file_change(Some(&path));
                }
                EventKind::Modify(ModifyKind::Data(DataChange::Any))
            }
        };
        Some(Event::new(kind).add_path(path))
    }
}

impl WatchEntry {
    fn is_current(&self, path: &Path) -> bool {
        let current = std::fs::metadata(path);
        let opened = self.file.metadata();
        matches!((current, opened), (Ok(current), Ok(opened)) if same_vnode(&current, &opened))
    }
}

fn expected_type(metadata: &Metadata, directory: bool) -> bool {
    if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    }
}

fn same_vnode(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn available_fds(reserve: usize) -> Result<usize> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: limit is writable storage for getrlimit's entire result.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == -1 {
        return Err(Error::io(io::Error::last_os_error()));
    }

    let mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_NFDS, 0];
    let mut count: libc::c_int = 0;
    let mut size = std::mem::size_of_val(&count);
    // SAFETY: the MIB is read-only and count is writable for size bytes. No input value is set.
    let status = unsafe {
        libc::sysctl(
            mib.as_ptr(),
            4,
            std::ptr::from_mut(&mut count).cast(),
            &mut size,
            std::ptr::null(),
            0,
        )
    };
    if status == -1 {
        return Err(Error::io(io::Error::last_os_error()));
    }
    if size != std::mem::size_of_val(&count) || count < 0 {
        return Err(Error::generic("unexpected FreeBSD open descriptor count"));
    }
    let limit = usize::try_from(limit.rlim_cur).unwrap_or(usize::MAX);
    let count = usize::try_from(count)
        .map_err(|_| Error::generic("invalid FreeBSD open descriptor count"))?;
    Ok(limit.saturating_sub(count).saturating_sub(reserve))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::sync::mpsc;
    use std::time::Duration;

    use notify::ErrorKind;

    use super::FreeBsdWatcher;

    #[test]
    fn files_and_directories_share_the_watch_budget() -> notify::Result<()> {
        let project = tempfile::tempdir()?;
        let root = project.path().to_path_buf();
        let subtree = root.join("watched");
        fs::create_dir(&subtree)?;
        let first = subtree.join("first.rs");
        let second = root.join("second.rs");
        fs::write(&first, "")?;
        fs::write(&second, "")?;

        let mut watcher = FreeBsdWatcher::new(|_| {}, 3, 0)?;
        for (_, result) in watcher.watch_explicit(vec![root, subtree.clone()], true)? {
            result?;
        }
        for (_, result) in watcher.watch_explicit(vec![first], false)? {
            result?;
        }
        let refused = watcher.watch_explicit(vec![second.clone()], false)?;
        assert!(matches!(
            &refused[..],
            [(_, Err(error))] if matches!(error.kind, ErrorKind::MaxFilesWatch)
        ));

        watcher.unwatch(&subtree)?;
        for (_, result) in watcher.watch_explicit(vec![second], false)? {
            result?;
        }
        Ok(())
    }

    #[test]
    fn directory_snapshot_drops_recreated_vnodes_without_a_callback() -> notify::Result<()> {
        let project = tempfile::tempdir()?;
        let subtree = project.path().join("watched");
        fs::create_dir(&subtree)?;
        let mut watcher = FreeBsdWatcher::new(|_| {}, 1, 0)?;
        for (_, result) in watcher.watch_explicit(vec![subtree.clone()], true)? {
            result?;
        }

        fs::remove_dir(&subtree)?;
        fs::create_dir(&subtree)?;
        assert!(watcher.snapshot()?.0.is_empty());
        for (_, result) in watcher.watch_explicit(vec![subtree.clone()], true)? {
            result?;
        }
        assert!(watcher.snapshot()?.0.contains(&subtree));
        Ok(())
    }

    #[test]
    fn watched_symlink_reports_writes_at_the_link_path() -> notify::Result<()> {
        let project = tempfile::tempdir()?;
        let target = project.path().join("target.rs");
        let link = project.path().join("link.rs");
        fs::write(&target, "before")?;
        symlink(&target, &link)?;
        let (send, receive) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = FreeBsdWatcher::new(
            move |event| {
                let _ = send.send(event);
            },
            1,
            0,
        )?;
        for (_, result) in watcher.watch_explicit(vec![link.clone()], false)? {
            result?;
        }
        fs::write(&target, "edited")?;
        let event = receive
            .recv_timeout(Duration::from_secs(3))
            .map_err(|error| notify::Error::generic(&error.to_string()))??;
        assert_eq!(event.paths, [link]);
        assert!(event.kind.is_modify());
        Ok(())
    }

    #[test]
    fn snapshot_remembers_parent_of_removed_file() -> notify::Result<()> {
        let project = tempfile::tempdir()?;
        let root = project.path().to_path_buf();
        let file = root.join("lib.rs");
        fs::write(&file, "")?;
        let (send, receive) = mpsc::channel::<()>();
        let mut watcher = FreeBsdWatcher::new(
            move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|event| event.kind.is_remove()) {
                    let _ = send.send(());
                }
            },
            2,
            0,
        )?;
        for (_, result) in watcher.watch_explicit(vec![root.clone()], true)? {
            result?;
        }
        for (_, result) in watcher.watch_explicit(vec![file.clone()], false)? {
            result?;
        }
        fs::remove_file(&file)?;
        receive
            .recv_timeout(Duration::from_secs(3))
            .map_err(|error| notify::Error::generic(&error.to_string()))?;
        fs::write(&file, "replacement")?;
        let (directories, affected) = watcher.snapshot()?;
        assert!(directories.contains(&root));
        assert_eq!(affected, [root]);
        for (_, result) in watcher.watch_explicit(vec![file], false)? {
            result?;
        }
        Ok(())
    }
}
