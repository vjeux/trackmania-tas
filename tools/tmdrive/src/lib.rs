//! `tmdrive` — the ONLY way to drive the Trackmania instance on the render box.
//!
//! # Why this crate exists
//!
//! The render box runs ONE Trackmania. Its screen, its input queue and its
//! plugin command channel are all singletons. Two drivers at once do not fail
//! — they both *succeed*, on each other's game: one session's `/playmap` lands
//! in another's capture, a jump impulse arrives during someone else's render,
//! and every check passes because no check is looking. That collision happened
//! on 2026-09-23 between a map-publishing session and a physics test.
//!
//! A lock existed (`shootctl lock`) and three drivers ignored it, because it
//! was *cooperative*: nothing made a caller take it. It was machine-scoped and
//! pid-based, so a driver that died left the whole box locked — one dead
//! session held it for over an hour while two live ones drove the game anyway.
//!
//! # The four properties that fix that, permanently
//!
//! 1. **Holding it is not optional.** Every mutating operation is a method on
//!    [`GameLock`]; the guard is the only way to spell the call. Forgetting to
//!    lock is a compile error, not a corrupted run found days later. The
//!    contract test `no_raw_game_access.rs` stops a caller side-stepping the
//!    crate by shelling out, and the in-game [`plugin`] token gate refuses
//!    anything that gets through anyway — including a hand-run `curl`.
//!
//! 2. **The owner is a SESSION, and a session can be talked to.** The record
//!    carries the agentcloud session id, title and purpose, so a blocked driver
//!    names the holder, messages it, and waits — instead of guessing a timeout
//!    or stealing.
//!
//! 3. **It cannot wedge.** The lock is reclaimable the moment ANY of four
//!    independent conditions holds ([`Reclaim`]): it is ours; the game it
//!    protects is gone; its lease expired unrenewed; or the owning session is
//!    no longer running. A holder that dies, hangs, or wanders off frees the
//!    box on its own — no human, no `--force`.
//!
//! 4. **Renewal is automatic.** Acquiring spawns a renewer thread that beats
//!    for as long as the guard lives, so a caller cannot forget to renew and
//!    lose the box mid-render. Callers do nothing.
//!
//! # Where the lock lives, and why there
//!
//! Under the game's own `PluginStorage`, because that path is visible from
//! BOTH sides: WSL drivers see `/mnt/c/Users/.../TmDriveLock`, and the
//! in-game plugin (a Windows process) sees the same directory through
//! `IO::FromDataFolder`. One record, read by everyone, so the plugin validates
//! against the live lock rather than a copy that can go stale. The first
//! version kept the record on the WSL side and mirrored a token file to the
//! plugin; the mirror outlived its lock, and a stale token still opened the
//! game.
//!
//! # What is and is not locked
//!
//! Locked: launch, kill, OS input, DLL injection, plugin commands, and writes
//! into the game folder. Not locked: reading logs, state files, screenshots,
//! and anything else side-effect free. A lock covering reads would be held
//! permanently by everyone, which is the same as no lock at all.

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub mod host;
pub mod ops;
pub mod plugin;

pub use host::Host;

/// The lock directory, as WSL sees it.
///
/// Under `PluginStorage` so the in-game plugin can read the same bytes — see
/// the module docs.
pub const LOCK_DIR: &str = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/TmDriveLock";

/// How long a lock survives without a renewal. The guard renews every third of
/// this, so an unrenewed lease means the holder's process is gone, wedged, or
/// has stopped caring — all of which should free the box.
pub const LEASE_S: u64 = 120;

/// A lock taken before its game exists is honoured this long, so a driver can
/// hold the box across the launch it is about to perform.
pub const LAUNCH_GRACE_S: u64 = 240;

/// The token of the lock this process holds, if any. Lets [`plugin::get`]
/// stamp in-game commands without threading the guard through every layer.
static HELD_TOKEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn held_token() -> Option<String> {
    HELD_TOKEN.lock().ok().and_then(|g| g.clone())
}

fn set_held_token(t: Option<String>) {
    if let Ok(mut g) = HELD_TOKEN.lock() {
        *g = t;
    }
}

#[derive(Debug)]
pub enum Error {
    /// A live session holds the box. Carries the holder so the caller can name
    /// it, inspect it, and message it.
    Busy(Holder),
    Lock(String),
    Op(String),
    /// The caller did not identify itself. A lock nobody can be traced back to
    /// is the failure this crate exists to prevent, so this is refused rather
    /// than defaulted.
    NoSession,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Busy(h) => write!(
                f,
                "the game is BUSY.\n  {}\nOne game, one driver. Either wait:\n  \
                 tmdrive wait --timeout 600\nor ask that session for it:\n  \
                 agentcloudctl send-message --to {} --body 'may I have the render box?'",
                h.summary(),
                h.session_id
            ),
            Error::Lock(m) => write!(f, "lock: {m}"),
            Error::Op(m) => write!(f, "game: {m}"),
            Error::NoSession => write!(
                f,
                "no session id: set TM_SESSION (or AGENTCLOUD_SESSION_ID) so the lock can \
                 name its holder and other drivers can reach you"
            ),
        }
    }
}

impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Why a held lock may be taken. Every reclaim is announced, because each one
/// means something went wrong in the previous holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reclaim {
    /// Already ours: renew, say nothing.
    Ours,
    /// The lease ran out with no renewal: the holder is dead or wedged.
    LeaseExpired,
    /// The owning agentcloud session is no longer running.
    SessionSettled,
}

impl Reclaim {
    fn describe(self, h: &Holder) -> Option<String> {
        match self {
            Reclaim::Ours => None,
            Reclaim::LeaseExpired => Some(format!(
                "tmdrive: taking an EXPIRED lock from session {} ('{}') — no renewal for {}s \
                 (lease {}s), so that driver is gone or wedged.",
                h.session_id, h.purpose, h.since_renewed_s, LEASE_S
            )),
            Reclaim::SessionSettled => Some(format!(
                "tmdrive: taking an ABANDONED lock from session {} ('{}') — that session is no \
                 longer running.",
                h.session_id, h.purpose
            )),
        }
    }
}

/// Who holds the box, and how to reach them.
#[derive(Debug, Clone)]
pub struct Holder {
    pub session_id: String,
    pub title: String,
    pub purpose: String,
    pub game_pid: Option<u32>,
    /// No game process right now. INFORMATIONAL ONLY — never a reason to take
    /// the lock: a holder restarting the game still owns the box.
    pub game_gone: bool,
    pub age_s: u64,
    pub since_renewed_s: u64,
    /// `None` when the lock is live; `Some(reason)` when it may be taken.
    pub reclaimable: Option<Reclaim>,
}

impl Holder {
    pub fn summary(&self) -> String {
        format!(
            "{} session={} title='{}' purpose='{}' pid={} held={}s renewed={}s ago",
            match self.reclaimable {
                None => "HELD",
                Some(Reclaim::Ours) => "OURS",
                Some(Reclaim::LeaseExpired) => "DEAD(lease expired)",
                Some(Reclaim::SessionSettled) => "DEAD(session settled)",
            },
            self.session_id,
            self.title,
            self.purpose,
            self.game_pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
            self.age_s,
            self.since_renewed_s
        ) + if self.game_gone { "  [no game process right now]" } else { "" }
    }
}

/// This driver's identity: the agentcloud session that answers if another
/// driver needs the box.
#[derive(Debug, Clone)]
pub struct Identity {
    pub session_id: String,
    pub title: String,
}

impl Identity {
    pub fn from_env() -> Result<Identity> {
        let session_id = std::env::var("TM_SESSION")
            .or_else(|_| std::env::var("AGENTCLOUD_SESSION_ID"))
            .map_err(|_| Error::NoSession)?;
        if session_id.trim().is_empty() {
            return Err(Error::NoSession);
        }
        Ok(Identity {
            session_id: session_id.trim().to_string(),
            title: std::env::var("TM_SESSION_TITLE").unwrap_or_else(|_| "untitled".into()),
        })
    }
}

pub fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn drive_c() -> &'static str {
    if PathBuf::from("/mnt/c/Windows").exists() {
        "/mnt/c"
    } else {
        "C:"
    }
}

pub fn system32(exe: &str) -> String {
    format!("{}/Windows/System32/{}", drive_c(), exe)
}


/// Refuse to drive a box that has grown a second game install.
///
/// Cheap (one `test -e` per location) and called before every launch: a
/// duplicate install is silent, and silence is what made it cost a day. See
/// [`ops::GAME_EXE_WIN`] for the history.
pub fn assert_single_install(host: &Host) -> Result<()> {
    let checks: Vec<String> = ops::FORBIDDEN_INSTALL_DIRS
        .iter()
        .map(|d| format!("test -e '{d}/Trackmania.exe' && echo '{d}'"))
        .collect();
    if let Ok(found) = host.read_cmd(&format!("{{ {} ; }} 2>/dev/null || true", checks.join("; "))) {
        let found: Vec<&str> = found.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if !found.is_empty() {
            return Err(Error::Op(format!(
                "TWO GAME INSTALLS. A second Trackmania lives at:\n  {}\n\
                 Each install carries its own Openplanet, they drift apart, and whichever tool \
                 launches decides which one you get — that is the 2026-09-23 failure (50 s \
                 startup scans, every plugin broken). Remove the extra copy, or update \
                 tmdrive::ops::GAME_EXE_WIN if the canonical install genuinely moved.",
                found.join("\n  ")
            )));
        }
    }
    Ok(())
}

/// A map path the GAME can resolve, or a refusal.
///
/// The title API accepts anything and silently loads nothing: given
/// `/mnt/c/Users/...` — the WSL spelling of a path that is perfectly real on
/// this side of the bridge — it answers ok, reports ready, keeps rendering,
/// and no playground ever appears. That is indistinguishable from a map the
/// game cannot load, and it has now cost two separate evenings: once in the
/// render pipeline, and again on 2026-09-23 with backslashes in the jump
/// harness. Every successful load in the logs reads `C:/Users/...`.
///
/// So the conversion happens HERE, once, for every driver — and anything
/// still unresolvable is REFUSED rather than handed over, because a wiring
/// error must not be able to come back as a fact about a map.
pub fn game_path(p: &str) -> std::result::Result<String, String> {
    // /mnt/<drive>/rest -> <DRIVE>:/rest
    if let Some(rest) = p.strip_prefix("/mnt/") {
        let mut it = rest.splitn(2, '/');
        if let (Some(d), Some(tail)) = (it.next(), it.next()) {
            if d.len() == 1 && d.chars().next().unwrap().is_ascii_alphabetic() {
                return Ok(format!("{}:/{}", d.to_ascii_uppercase(), tail));
            }
        }
        return Err(format!("{p}: looks like a WSL path but names no drive"));
    }
    let b = p.as_bytes();
    if b.len() >= 3
        && (b[0] as char).is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'/' || b[2] == b'\\')
    {
        // Backslashes reach the game as-is and it loads nothing; normalise.
        return Ok(p.replace('\\', "/"));
    }
    Err(format!(
        "{p}: not a path the game can resolve. The title API accepts anything and silently \
         loads nothing, so this is refused here. Give a Windows path (C:/Users/...) or a WSL \
         path under /mnt/<drive>/."
    ))
}

/// Is the game running, and as which pid?
pub fn game_pid(host: &Host) -> Option<u32> {
    let out = host
        .read_cmd(&format!(
            "{} /FI \"IMAGENAME eq Trackmania.exe\" /NH /FO CSV 2>/dev/null || true",
            system32("tasklist.exe")
        ))
        .ok()?;
    for line in out.lines() {
        if !line.contains("Trackmania.exe") {
            continue;
        }
        if let Some(pid) = line.split(',').nth(1).and_then(|p| p.trim().trim_matches('"').parse().ok())
        {
            return Some(pid);
        }
    }
    None
}

/// Is an agentcloud session still running?
///
/// `None` means "cannot tell" — no CLI, a timeout, an unexpected answer — and
/// an unknown answer must never be read as "dead": stealing a live session's
/// box is the worse error, so [`Reclaim::SessionSettled`] only fires on a
/// definite negative.
fn session_running(session_id: &str) -> Option<bool> {
    let out = std::process::Command::new("agentcloudctl")
        .args(["describe", "-s", session_id, "--output", "json"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    for key in ["\"running\"", "\"status\"", "\"state\""] {
        if let Some(i) = text.find(key) {
            let tail = &text[i + key.len()..];
            let head: String = tail.chars().take(40).collect();
            if head.contains("true") || head.contains("running") || head.contains("active") {
                return Some(true);
            }
            if head.contains("false")
                || head.contains("settled")
                || head.contains("complete")
                || head.contains("failed")
                || head.contains("cancelled")
            {
                return Some(false);
            }
        }
    }
    None
}

fn lock_file(name: &str) -> String {
    format!("{LOCK_DIR}/{name}")
}

/// Read the current holder, if any, and decide whether it may be taken.
pub fn holder(host: &Host) -> Result<Option<Holder>> {
    let out = host
        .read_cmd(&format!(
            "L={LOCK_DIR}; if [ ! -d \"$L\" ]; then echo NONE; exit 0; fi; \
             printf '%s\\n' \"$(cat \"$L/session\" 2>/dev/null)\" \
                 \"$(cat \"$L/title\" 2>/dev/null)\" \
                 \"$(cat \"$L/purpose\" 2>/dev/null)\" \
                 \"$(cat \"$L/game_pid\" 2>/dev/null)\" \
                 \"$(cat \"$L/acquired_at\" 2>/dev/null)\" \
                 \"$(cat \"$L/renewed_at\" 2>/dev/null)\""
        ))
        .map_err(Error::Lock)?;
    if out.trim() == "NONE" {
        return Ok(None);
    }
    let mut it = out.lines();
    let session_id = it.next().unwrap_or("").trim().to_string();
    let title = it.next().unwrap_or("").trim().to_string();
    let purpose = it.next().unwrap_or("").trim().to_string();
    let locked_pid: Option<u32> = it.next().and_then(|s| s.trim().parse().ok());
    let acquired_at: u64 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let renewed_at: u64 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(acquired_at);

    let now = now_s();
    let age_s = now.saturating_sub(acquired_at);
    let since_renewed_s = now.saturating_sub(renewed_at);

    // A half-written record (mkdir won the race, the fields are not in yet) is
    // a HELD lock, not a broken one -- see `acquire`.
    if session_id.is_empty() {
        return Ok(Some(Holder {
            session_id: "(being written)".into(),
            title,
            purpose,
            game_pid: locked_pid,
            game_gone: game_pid(host).is_none(),
            age_s,
            since_renewed_s,
            reclaimable: if since_renewed_s > LEASE_S { Some(Reclaim::LeaseExpired) } else { None },
        }));
    }

    // R2 WAS "the game is gone" AND THAT WAS WRONG.
    //
    // A holder that restarts the game as part of its own work (a bisect, a
    // relaunch after a settings change) has no game process for a while. The
    // old rule handed the box to somebody else in that gap, mid-run — the
    // MK64 session hit exactly this on 2026-09-23 and lost its hold every
    // time its script restarted the game.
    //
    // An actively renewing holder IS using the box, whatever the game process
    // is doing. A holder that has genuinely died stops renewing, and the
    // lease catches it within LEASE_S; a settled session is caught sooner.
    // So game-gone is reported, never acted on.
    let live_pid = game_pid(host);
    let game_gone = live_pid.is_none();

    // Keep the record pointing at the live instance, so `status` stays
    // truthful across the startup handoff and across a holder's own restart.
    if let (Some(live), Some(locked)) = (live_pid, locked_pid) {
        if live != locked {
            let _ = host.read_cmd(&format!("printf '%s' '{live}' > '{}'", lock_file("game_pid")));
        }
    }

    let reclaimable = if since_renewed_s > LEASE_S {
        Some(Reclaim::LeaseExpired)
    } else if session_running(&session_id) == Some(false) {
        Some(Reclaim::SessionSettled)
    } else {
        None
    };

    Ok(Some(Holder {
        session_id,
        title,
        purpose,
        game_pid: locked_pid,
        game_gone,
        age_s,
        since_renewed_s,
        reclaimable,
    }))
}

/// The exclusive right to drive the game, held for as long as this value lives
/// and renewed automatically in the background. Released on drop — including
/// on panic and on every early return.
pub struct GameLock {
    pub(crate) host: Host,
    pub(crate) identity: Identity,
    pub(crate) token: String,
    released: Arc<AtomicBool>,
    /// Did THIS guard create the lock, or join one its session already held?
    ///
    /// A nested acquire must not end the outer hold. `tmdrive kill` inside a
    /// `tmdrive run ... -- script` joined the script's lock and then deleted
    /// it on exit, taking the token with it and making kill+launch inside one
    /// hold impossible (MK64 session, 2026-09-23). Only the creator releases.
    owns: bool,
}

impl GameLock {
    pub fn session_id(&self) -> &str {
        &self.identity.session_id
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Record the game instance this lock protects, so the lock dies with it.
    pub(crate) fn bind_game(&self, pid: u32) -> Result<()> {
        self.host
            .read_cmd(&format!("printf '%s' '{pid}' > '{}'", lock_file("game_pid")))
            .map_err(Error::Lock)?;
        Ok(())
    }

    /// Push the lease out. Called by the renewer thread and by every operation.
    pub(crate) fn renew(&self) {
        let _ = self
            .host
            .read_cmd(&format!("printf '%s' '{}' > '{}'", now_s(), lock_file("renewed_at")));
    }

    /// Give up the box early. Dropping does this too; calling it explicitly
    /// turns a release failure into something the caller can report.
    pub fn release(mut self) -> Result<()> {
        self.release_inner()
    }

    fn release_inner(&mut self) -> Result<()> {
        if self.released.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        if !self.owns {
            // Re-entrant guard: the outer hold owns the lifetime.
            return Ok(());
        }
        set_held_token(None);
        std::env::remove_var("TM_LOCK_TOKEN");
        let me = &self.identity.session_id;
        // Removing the directory removes the token with it, so a released lock
        // cannot leave a key behind that still opens the game.
        let out = self.host.read_cmd(&format!(
            "L={LOCK_DIR}; if [ ! -d \"$L\" ]; then echo not-locked; exit 0; fi; \
             o=$(cat \"$L/session\" 2>/dev/null); \
             if [ \"$o\" != '{me}' ]; then echo \"refused:$o\"; exit 0; fi; \
             rm -rf \"$L\"; echo released"
        ));
        match out {
            Ok(s) if s.trim().starts_with("refused:") => Err(Error::Lock(format!(
                "refusing to release a lock now held by {}",
                s.trim().trim_start_matches("refused:")
            ))),
            Ok(_) => Ok(()),
            Err(e) => Err(Error::Lock(e)),
        }
    }
}

impl Drop for GameLock {
    fn drop(&mut self) {
        let _ = self.release_inner();
    }
}

/// Take the box, or fail naming who has it.
///
/// Never blocks: waiting is [`wait_until_free`], so the system keeps one
/// polling loop rather than one per tool.
pub fn acquire(host: Host, purpose: &str) -> Result<GameLock> {
    acquire_as(host, Identity::from_env()?, purpose)
}

pub fn acquire_as(host: Host, identity: Identity, purpose: &str) -> Result<GameLock> {
    if let Some(h) = holder(&host)? {
        let ours = h.session_id == identity.session_id;
        match h.reclaimable {
            _ if ours => {}
            None => return Err(Error::Busy(h)),
            Some(reason) => {
                if let Some(msg) = reason.describe(&h) {
                    eprintln!("{msg}");
                }
                host.read_cmd(&format!("rm -rf {LOCK_DIR}")).map_err(Error::Lock)?;
            }
        }
    }

    let pid = game_pid(&host);
    let token = format!("{}:{:x}", identity.session_id, now_s() ^ ((std::process::id() as u64) << 20));
    let esc = |s: &str| s.replace('\'', "").replace('\n', " ");
    let now = now_s();

    // mkdir is the atomic primitive: two racers cannot both create it. The
    // fields are written after, so there is a brief window where the lock is
    // held by an unnamed owner -- `holder` treats that as HELD, which is the
    // safe reading (the reverse order would let a second driver create the
    // directory while the first was still naming itself).
    let script = format!(
        "L={LOCK_DIR}; \
         if mkdir -p \"$(dirname \"$L\")\" 2>/dev/null && mkdir \"$L\" 2>/dev/null; then \
           printf '%s' '{session}' > \"$L/session\"; \
           printf '%s' '{title}'   > \"$L/title\"; \
           printf '%s' '{purpose}' > \"$L/purpose\"; \
           printf '%s' '{pid}'     > \"$L/game_pid\"; \
           printf '%s' '{now}'     > \"$L/acquired_at\"; \
           printf '%s' '{now}'     > \"$L/renewed_at\"; \
           printf '%s' '{token}'   > \"$L/token\"; \
           echo ok; \
         else \
           o=$(cat \"$L/session\" 2>/dev/null); \
           if [ \"$o\" = '{session}' ]; then \
             printf '%s' '{purpose}' > \"$L/purpose\"; \
             printf '%s' '{token}'   > \"$L/token\"; \
             printf '%s' '{now}'     > \"$L/renewed_at\"; \
             echo ours; \
           else echo \"busy:$o\"; fi; \
         fi",
        session = esc(&identity.session_id),
        title = esc(&identity.title),
        purpose = esc(purpose),
        pid = pid.map(|p| p.to_string()).unwrap_or_default(),
        now = now,
        token = esc(&token),
    );

    let out = host.read_cmd(&script).map_err(Error::Lock)?;
    let out = out.trim();
    if out.starts_with("busy:") {
        return match holder(&host)? {
            Some(h) => Err(Error::Busy(h)),
            None => Err(Error::Lock("lost the acquire race, then the lock vanished".into())),
        };
    }
    if out != "ok" && out != "ours" {
        return Err(Error::Lock(format!("unexpected acquire result: {out}")));
    }

    set_held_token(Some(token.clone()));
    // Children inherit the token, so a driver that shells out to another tool
    // (tinyctl -> shootctl, a .sh wrapper) carries the lock with it.
    std::env::set_var("TM_LOCK_TOKEN", &token);

    let lock = GameLock {
        host,
        identity,
        token,
        released: Arc::new(AtomicBool::new(false)),
        owns: out == "ok",
    };
    // Only the creator needs a renewer; a nested guard rides the outer one's.
    if lock.owns {
        lock.spawn_renewer();
    }
    Ok(lock)
}

impl GameLock {
    /// Beat the lease for as long as the guard lives.
    ///
    /// Renewal belongs to the guard, not to callers: a ten-minute render must
    /// not lose the box because the caller forgot a keepalive, and no caller
    /// should have to remember one.
    fn spawn_renewer(&self) {
        let host = self.host.clone();
        let released = Arc::clone(&self.released);
        let me = self.identity.session_id.clone();
        std::thread::spawn(move || {
            let every = std::time::Duration::from_secs((LEASE_S / 3).max(5));
            loop {
                std::thread::sleep(every);
                if released.load(Ordering::SeqCst) {
                    return;
                }
                // Only renew while the record is still ours: if we were
                // reclaimed, stop rather than stamping someone else's lock.
                let _ = host.read_cmd(&format!(
                    "L={LOCK_DIR}; [ -d \"$L\" ] || exit 0; \
                     [ \"$(cat \"$L/session\" 2>/dev/null)\" = '{me}' ] || exit 0; \
                     printf '%s' '{}' > \"$L/renewed_at\"",
                    now_s()
                ));
            }
        });
    }
}

/// Is the box free for this session right now?
pub fn is_free(host: &Host, session_id: &str) -> bool {
    match holder(host) {
        Ok(None) => true,
        Ok(Some(h)) => h.session_id == session_id || h.reclaimable.is_some(),
        Err(_) => false,
    }
}

/// Block until the box is free, or the deadline passes.
///
/// The one polling loop in the crate. Prints the holder while it waits, so a
/// long wait says who it is waiting for rather than looking hung.
pub fn wait_until_free(host: &Host, session_id: &str, timeout_s: u64) -> Result<()> {
    let start = now_s();
    let mut last_report = 0u64;
    loop {
        if is_free(host, session_id) {
            return Ok(());
        }
        let waited = now_s().saturating_sub(start);
        if waited >= timeout_s {
            return match holder(host)? {
                Some(h) => Err(Error::Busy(h)),
                None => Ok(()),
            };
        }
        if waited.saturating_sub(last_report) >= 15 {
            last_report = waited;
            if let Ok(Some(h)) = holder(host) {
                println!("  ... waiting {waited}s for the box — {}", h.summary());
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
