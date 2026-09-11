//! Bounded X11 plaintext paste. The persistent owner outlives the tool call.
//!
//! Completion is a target-client text-property deletion, not application save
//! acknowledgment. INCR and native rich-text paste remain unsupported. A timed
//! out submitted paste keeps its payload available until transfer or owner loss;
//! no input replay or timer-driven clipboard restoration is performed.
use std::{
    collections::HashSet,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};
use x11rb::{
    connection::Connection,
    protocol::{
        res::{ClientIdMask, ClientIdSpec, ConnectionExt as _},
        xfixes::{ConnectionExt as _, SelectionEventMask},
        xproto::{
            Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux,
            EventMask, PropMode, Property, SelectionNotifyEvent, SelectionRequestEvent, Window,
            WindowClass, SELECTION_NOTIFY_EVENT,
        },
        Event,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
    COPY_DEPTH_FROM_PARENT, CURRENT_TIME, NONE,
};

const MAX_TEXT_BYTES: usize = 16 * 1024;
const MAX_FORMATS: usize = 32;
const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;
const SNAPSHOT_WAIT: Duration = Duration::from_secs(5);
const NEGOTIATION_WAIT: Duration = Duration::from_secs(2);
const TRANSFER_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct PasteRequest {
    pub pid: u32,
    pub window_id: u32,
    pub text: String,
    pub format: String,
}
#[derive(Debug)]
pub struct PasteOutcome {
    pub clipboard_restoration: String,
    pub transfer_verified: bool,
}
#[derive(Debug)]
pub struct PasteError {
    pub message: String,
    pub input_submitted: bool,
    pub clipboard_state: String,
}
impl fmt::Display for PasteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for PasteError {}

struct Command {
    request: PasteRequest,
    submitted: Arc<AtomicBool>,
    reply: mpsc::Sender<std::result::Result<PasteOutcome, PasteError>>,
}
static ACTOR: OnceLock<std::result::Result<mpsc::Sender<Command>, String>> = OnceLock::new();

/// The caller holds the core clipboard writer lock around this blocking call.
pub fn paste(request: PasteRequest) -> std::result::Result<PasteOutcome, PasteError> {
    let refusal = |message: String| PasteError {
        message,
        input_submitted: false,
        clipboard_state: "unchanged".into(),
    };
    if request.format != "text" {
        return Err(refusal(
            "Native X11 paste currently supports plain text only".into(),
        ));
    }
    if request.pid == 0 || request.window_id == 0 || request.text.len() > MAX_TEXT_BYTES {
        return Err(refusal(
            "Invalid native paste target or text exceeds 16 KiB".into(),
        ));
    }
    // Do not silently apply an X11 transaction to a Wayland clipboard.
    if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_none() {
        return Err(refusal(
            "Native paste requires an explicitly X11 desktop".into(),
        ));
    }
    let sender = ACTOR
        .get_or_init(|| {
            let (commands, receiver) = mpsc::channel::<Command>();
            let (ready, initialized) = mpsc::sync_channel(1);
            thread::Builder::new()
                .name("native-paste-x11".into())
                .spawn(move || match Actor::new() {
                    Ok(mut actor) => {
                        let _ = ready.send(Ok(()));
                        actor.run(receiver);
                    }
                    Err(error) => {
                        let _ = ready.send(Err(format!("{error:#}")));
                    }
                })
                .map_err(|e| e.to_string())?;
            initialized.recv().map_err(|e| e.to_string())??;
            Ok(commands)
        })
        .as_ref()
        .map_err(|message| refusal(message.clone()))?;
    let submitted = Arc::new(AtomicBool::new(false));
    let (reply, result) = mpsc::channel();
    sender
        .send(Command {
            request,
            submitted: submitted.clone(),
            reply,
        })
        .map_err(|_| refusal("Native paste actor unavailable".into()))?;
    result.recv().map_err(|_| PasteError {
        message: "Native paste actor disconnected; do not replay input".into(),
        input_submitted: submitted.load(Ordering::SeqCst),
        clipboard_state: "unknown".into(),
    })?
}

#[derive(Clone, Debug)]
struct Data {
    target: Atom,
    type_: Atom,
    format: u8,
    bytes: Vec<u8>,
}
#[derive(Clone)]
struct Snapshot {
    owner: Window,
    epoch: u64,
    data: Vec<Data>,
}
struct Offer {
    window: Window,
    timestamp: u32,
    data: Vec<Data>,
}
struct Readback {
    window: Window,
    property: Atom,
    text: bool,
    saw_value: bool,
}
struct Pending {
    original: Snapshot,
    window: Window,
    epoch: u64,
    pid: u32,
    submitted: Arc<AtomicBool>,
    reads: Vec<Readback>,
    negotiated: bool,
    transferred: bool,
}
struct Atoms {
    clipboard: Atom,
    targets: Atom,
    timestamp: Atom,
    multiple: Atom,
    atom_pair: Atom,
    incr: Atom,
    stamp: Atom,
    property: Atom,
    active: Atom,
    text: Vec<Atom>,
}
struct Actor {
    conn: Arc<RustConnection>,
    root: Window,
    control: Window,
    display: Option<std::ffi::OsString>,
    atoms: Atoms,
    epoch: u64,
    offers: Vec<Offer>,
    pending: Option<Pending>,
    per_format_limit: usize,
    maintenance_error: Option<String>,
}

struct ServerGrab {
    conn: Arc<RustConnection>,
    held: bool,
}
impl ServerGrab {
    fn new(conn: Arc<RustConnection>) -> Result<Self> {
        conn.grab_server()?.check()?;
        Ok(Self { conn, held: true })
    }
    fn release(mut self) -> Result<()> {
        self.conn.ungrab_server()?.check()?;
        self.conn.flush()?;
        self.held = false;
        Ok(())
    }
}
impl Drop for ServerGrab {
    fn drop(&mut self) {
        // Also releases on early errors/panics. No application waits occur
        // while this guard is live; only this connection's server roundtrips.
        if self.held {
            if let Ok(cookie) = self.conn.ungrab_server() {
                let _ = cookie.check();
            }
            let _ = self.conn.flush();
        }
    }
}

impl Actor {
    fn new() -> Result<Self> {
        let display = std::env::var_os("DISPLAY");
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let res = conn.res_query_version(1, 2)?.reply()?;
        ensure!(
            (res.server_major, res.server_minor) >= (1, 2),
            "XRes 1.2 is required"
        );
        let fixes = conn.xfixes_query_version(2, 0)?.reply()?;
        ensure!(fixes.major_version >= 2, "XFixes 2 is required");
        let atom =
            |name: &[u8]| -> Result<Atom> { Ok(conn.intern_atom(false, name)?.reply()?.atom) };
        let atoms = Atoms {
            clipboard: atom(b"CLIPBOARD")?,
            targets: atom(b"TARGETS")?,
            timestamp: atom(b"TIMESTAMP")?,
            multiple: atom(b"MULTIPLE")?,
            atom_pair: atom(b"ATOM_PAIR")?,
            incr: atom(b"INCR")?,
            stamp: atom(b"_OPENSKY_PASTE_TIME")?,
            property: atom(b"_OPENSKY_PASTE_READ")?,
            active: atom(b"_NET_ACTIVE_WINDOW")?,
            text: [
                b"UTF8_STRING".as_slice(),
                b"text/plain;charset=utf-8",
                b"text/plain",
            ]
            .iter()
            .map(|n| atom(n))
            .collect::<Result<_>>()?,
        };
        let control = Self::window(&conn, root)?;
        conn.xfixes_select_selection_input(
            control,
            atoms.clipboard,
            SelectionEventMask::SET_SELECTION_OWNER
                | SelectionEventMask::SELECTION_WINDOW_DESTROY
                | SelectionEventMask::SELECTION_CLIENT_CLOSE,
        )?
        .check()?;
        let per_format_limit = MAX_TEXT_BYTES
            .min((usize::from(conn.setup().maximum_request_length) * 4).saturating_sub(32));
        let mut actor = Self {
            conn: Arc::new(conn),
            root,
            control,
            display,
            atoms,
            epoch: 0,
            offers: vec![],
            pending: None,
            per_format_limit,
            maintenance_error: None,
        };
        actor.barrier()?;
        Ok(actor)
    }
    fn window(conn: &RustConnection, root: Window) -> Result<Window> {
        let window = conn.generate_id()?;
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
        Ok(window)
    }
    fn run(&mut self, receiver: mpsc::Receiver<Command>) {
        loop {
            // Any connection/protocol failure stops admission; it must never
            // produce a guessed restoration or repeat a submitted chord.
            if self.drain().is_err() {
                return;
            }
            if self.maintenance_error.is_none() {
                if let Err(error) = self.finish_late() {
                    // Keep serving existing offers after a failed late restore,
                    // but stop new admission and automatic maintenance retries.
                    self.maintenance_error = Some(format!("{error:#}"));
                }
            }
            match receiver.recv_timeout(Duration::from_millis(5)) {
                Ok(command) => {
                    let result = self.execute(command.request, command.submitted);
                    let _ = command.reply.send(result);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }
    fn pid(&self, window: Window) -> Result<u32> {
        let reply = self
            .conn
            .res_query_client_ids(&[ClientIdSpec {
                client: window,
                mask: ClientIdMask::CLIENT_XID | ClientIdMask::LOCAL_CLIENT_PID,
            }])?
            .reply()?;
        let pids: Vec<_> = reply
            .ids
            .iter()
            .filter(|v| (u32::from(v.spec.mask) & u32::from(ClientIdMask::LOCAL_CLIENT_PID)) != 0)
            .filter_map(|v| {
                if v.value.len() == 1 {
                    Some(v.value[0])
                } else {
                    None
                }
            })
            .collect();
        ensure!(
            pids.len() == 1 && pids[0] > 0,
            "XRes did not identify exactly one local target PID"
        );
        Ok(pids[0])
    }
    fn focused(&self, request: &PasteRequest) -> Result<()> {
        ensure!(
            self.display == std::env::var_os("DISPLAY")
                && std::env::var_os("WAYLAND_DISPLAY").is_none(),
            "Clipboard display changed"
        );
        ensure!(
            self.pid(request.window_id)? == request.pid,
            "Native paste target PID differs"
        );
        ensure!(
            crate::input::x11_target_already_focused(
                &self.conn,
                self.root,
                self.atoms.active,
                request.window_id
            )?,
            "Native paste requires the exact focused window"
        );
        Ok(())
    }
    fn barrier(&mut self) -> Result<()> {
        self.conn.get_input_focus()?.reply()?;
        self.drain()
    }
    fn drain(&mut self) -> Result<()> {
        while let Some(event) = self.conn.poll_for_event()? {
            self.event(event)?;
        }
        Ok(())
    }
    fn event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::XfixesSelectionNotify(e) if e.selection == self.atoms.clipboard => {
                self.epoch = self
                    .epoch
                    .checked_add(1)
                    .context("Clipboard generation overflow")?;
            }
            Event::SelectionRequest(e) => {
                // Malformed requests and disappearing requestor windows must not
                // kill a persistent clipboard owner (or discard restored data).
                if let Err(error) = self.serve(e) {
                    tracing::debug!(?error, "Native paste selection request refused");
                    let _ = self.notify(e, NONE);
                }
            }
            Event::PropertyNotify(e) => {
                if let Some(p) = &mut self.pending {
                    for read in &mut p.reads {
                        if read.window == e.window && read.property == e.atom {
                            if e.state == Property::NEW_VALUE {
                                read.saw_value = true;
                            }
                            if e.state == Property::DELETE && read.saw_value {
                                if read.text && p.submitted.load(Ordering::SeqCst) {
                                    p.transferred = true;
                                }
                                if !read.text {
                                    p.negotiated = true;
                                }
                                read.saw_value = false;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn stamp(&mut self) -> Result<u32> {
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.control,
                self.atoms.stamp,
                AtomEnum::INTEGER,
                &[1],
            )?
            .check()?;
        let end = Instant::now() + SNAPSHOT_WAIT;
        while Instant::now() < end {
            if let Some(event) = self.conn.poll_for_event()? {
                if let Event::PropertyNotify(e) = &event {
                    if e.window == self.control
                        && e.atom == self.atoms.stamp
                        && e.state == Property::NEW_VALUE
                    {
                        return Ok(e.time);
                    }
                }
                self.event(event)?;
            } else {
                thread::sleep(Duration::from_millis(2));
            }
        }
        bail!("X11 server timestamp unavailable")
    }
    fn read_conversion(&mut self, window: Window, target: Atom) -> Result<Data> {
        self.conn
            .delete_property(window, self.atoms.property)?
            .check()?;
        self.conn
            .convert_selection(
                window,
                self.atoms.clipboard,
                target,
                self.atoms.property,
                CURRENT_TIME,
            )?
            .check()?;
        let end = Instant::now() + SNAPSHOT_WAIT;
        while Instant::now() < end {
            if let Some(event) = self.conn.poll_for_event()? {
                if let Event::SelectionNotify(e) = &event {
                    if e.requestor == window
                        && e.selection == self.atoms.clipboard
                        && e.target == target
                    {
                        ensure!(
                            e.property == self.atoms.property,
                            "Clipboard conversion unavailable"
                        );
                        let reply = self
                            .conn
                            .get_property(
                                true,
                                window,
                                self.atoms.property,
                                AtomEnum::ANY,
                                0,
                                (self.per_format_limit / 4 + 1) as u32,
                            )?
                            .reply()?;
                        ensure!(
                            reply.type_ != self.atoms.incr,
                            "INCR clipboard snapshots are not supported yet"
                        );
                        ensure!(
                            reply.bytes_after == 0 && reply.value.len() <= self.per_format_limit,
                            "Clipboard format exceeds bounded direct-transfer limit"
                        );
                        ensure!(
                            matches!(reply.format, 8 | 16 | 32) && reply.type_ != NONE,
                            "Unsupported clipboard property encoding"
                        );
                        ensure!(
                            reply.value.len() % (usize::from(reply.format) / 8) == 0,
                            "Misaligned clipboard data"
                        );
                        return Ok(Data {
                            target,
                            type_: reply.type_,
                            format: reply.format,
                            bytes: reply.value,
                        });
                    }
                }
                self.event(event)?;
            } else {
                thread::sleep(Duration::from_millis(2));
            }
        }
        bail!("Clipboard snapshot conversion timed out")
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        self.barrier()?;
        let owner = self
            .conn
            .get_selection_owner(self.atoms.clipboard)?
            .reply()?
            .owner;
        let epoch = self.epoch;
        if owner == NONE {
            return Ok(Snapshot {
                owner,
                epoch,
                data: vec![],
            });
        }
        if let Some(offer) = self.offers.iter().find(|o| o.window == owner) {
            return Ok(Snapshot {
                owner,
                epoch,
                data: offer.data.clone(),
            });
        }
        let window = Self::window(&self.conn, self.root)?;
        let result = (|| {
            let targets = self.read_conversion(window, self.atoms.targets)?;
            ensure!(
                targets.type_ == u32::from(AtomEnum::ATOM) && targets.format == 32,
                "Clipboard TARGETS is not ATOM/32"
            );
            let targets = words(&targets.bytes)?;
            ensure!(
                targets.len() <= MAX_FORMATS + 8,
                "Too many clipboard targets"
            );
            let mut data = vec![];
            let mut seen = HashSet::new();
            for target in targets {
                if !seen.insert(target)
                    || [
                        self.atoms.targets,
                        self.atoms.timestamp,
                        self.atoms.multiple,
                    ]
                    .contains(&target)
                {
                    continue;
                }
                let name = self.conn.get_atom_name(target)?.reply()?.name;
                if [
                    b"SAVE_TARGETS".as_slice(),
                    b"TARGET_SIZES",
                    b"DELETE",
                    b"INSERT_SELECTION",
                    b"INSERT_PROPERTY",
                ]
                .contains(&name.as_slice())
                {
                    // Protocol/side-effect targets are not captured user data.
                    continue;
                }
                ensure!(data.len() < MAX_FORMATS, "Too many clipboard data formats");
                data.push(self.read_conversion(window, target)?);
                ensure!(
                    data.iter().map(|d: &Data| d.bytes.len()).sum::<usize>() <= MAX_SNAPSHOT_BYTES,
                    "Clipboard snapshot too large"
                );
            }
            self.barrier()?;
            ensure!(
                self.conn
                    .get_selection_owner(self.atoms.clipboard)?
                    .reply()?
                    .owner
                    == owner
                    && self.epoch == epoch,
                "Clipboard changed during snapshot"
            );
            Ok(Snapshot { owner, epoch, data })
        })();
        self.destroy(window);
        result
    }
    fn owned(&self) -> Result<bool> {
        let Some(p) = &self.pending else {
            return Ok(false);
        };
        Ok(self
            .conn
            .get_selection_owner(self.atoms.clipboard)?
            .reply()?
            .owner
            == p.window
            && self.epoch == p.epoch)
    }
    fn destroy(&self, window: Window) {
        if let Ok(cookie) = self.conn.destroy_window(window) {
            let _ = cookie.check();
        }
    }
    fn prune_offers(&mut self) -> Result<()> {
        // Called only with no unresolved transaction. The currently owned
        // restored snapshot remains served; retired owner windows are bounded.
        let owner = self
            .conn
            .get_selection_owner(self.atoms.clipboard)?
            .reply()?
            .owner;
        let retired: Vec<_> = self
            .offers
            .iter()
            .filter(|o| o.window != owner)
            .map(|o| o.window)
            .collect();
        self.offers.retain(|o| o.window == owner);
        for window in retired {
            self.destroy(window);
        }
        Ok(())
    }
    fn claim(
        &mut self,
        snapshot: Snapshot,
        request: &PasteRequest,
        submitted: Arc<AtomicBool>,
    ) -> Result<()> {
        let timestamp = self.stamp()?;
        let window = Self::window(&self.conn, self.root)?;
        let data = self
            .atoms
            .text
            .iter()
            .map(|&target| Data {
                target,
                type_: target,
                format: 8,
                bytes: request.text.as_bytes().to_vec(),
            })
            .collect();
        let result = (|| {
            let grab = ServerGrab::new(self.conn.clone())?;
            self.barrier()?;
            ensure!(
                self.epoch == snapshot.epoch
                    && self
                        .conn
                        .get_selection_owner(self.atoms.clipboard)?
                        .reply()?
                        .owner
                        == snapshot.owner,
                "Clipboard changed before ownership claim"
            );
            let epoch = self
                .epoch
                .checked_add(1)
                .context("Clipboard generation overflow")?;
            // Install service state BEFORE submitting ownership. If an X11
            // reply is lost, the actor can still answer requests for this XID.
            self.offers.push(Offer {
                window,
                timestamp,
                data,
            });
            self.pending = Some(Pending {
                original: snapshot,
                window,
                epoch,
                pid: request.pid,
                submitted,
                reads: vec![],
                negotiated: false,
                transferred: false,
            });
            self.conn
                .set_selection_owner(window, self.atoms.clipboard, timestamp)?
                .check()?;
            self.barrier()?;
            ensure!(self.owned()?, "Clipboard ownership claim was not verified");
            grab.release()?;
            Ok(())
        })();
        if self.pending.is_none() {
            self.destroy(window);
        }
        result
    }
    fn restore(&mut self) -> Result<String> {
        let timestamp = self.stamp()?;
        let grab = ServerGrab::new(self.conn.clone())?;
        self.barrier()?;
        if !self.owned()? {
            self.pending = None;
            grab.release()?;
            return Ok("skipped_owner_changed".into());
        }
        let original = self.pending.as_ref().unwrap().original.clone();
        let window = if original.owner == NONE {
            NONE
        } else {
            Self::window(&self.conn, self.root)?
        };
        if window != NONE {
            // Keep the temporary offer too, until ownership verification has
            // completed. A protocol error must not leave a live owner unserved.
            self.offers.push(Offer {
                window,
                timestamp,
                data: original.data,
            });
        }
        self.conn
            .set_selection_owner(window, self.atoms.clipboard, timestamp)?
            .check()?;
        ensure!(
            self.conn
                .get_selection_owner(self.atoms.clipboard)?
                .reply()?
                .owner
                == window,
            "Clipboard restoration ownership was not verified"
        );
        self.pending = None;
        grab.release()?;
        Ok("restored".into())
    }
    fn finish_late(&mut self) -> Result<()> {
        if self.pending.is_none() {
            return Ok(());
        }
        if !self.owned()? {
            self.pending = None;
            return Ok(());
        }
        if self.pending.as_ref().unwrap().transferred {
            self.restore()?;
        }
        Ok(())
    }
    fn execute(
        &mut self,
        request: PasteRequest,
        submitted: Arc<AtomicBool>,
    ) -> std::result::Result<PasteOutcome, PasteError> {
        // A refused second call has no authority to restore a preceding
        // submitted transaction. That transaction remains serviced by the actor.
        if let Some(message) = &self.maintenance_error {
            return Err(PasteError {
                message: format!("Native paste actor requires recovery: {message}"),
                input_submitted: false,
                clipboard_state: "unknown".into(),
            });
        }
        if self.pending.is_some() {
            return Err(PasteError {
                message:
                    "A previous paste still awaits transfer or external clipboard ownership change"
                        .into(),
                input_submitted: false,
                clipboard_state: "previous_transaction_pending".into(),
            });
        }
        let mut claimed = false;
        let result = (|| -> Result<PasteOutcome> {
            self.prune_offers()?;
            self.focused(&request)?;
            crate::input::validate_paste_xtest(&self.conn, request.window_id)?;
            ensure!(
                request.text.len() <= self.per_format_limit,
                "Text exceeds X11 direct-transfer size limit"
            );
            let snapshot = self.snapshot()?;
            self.focused(&request)?;
            // claim installs pending state before its first ownership request.
            let claim = self.claim(snapshot, &request, submitted.clone());
            claimed = self.pending.is_some();
            claim?;
            // Writer probes demonstrated eager TARGETS negotiation before the
            // chord. Wait on actual protocol events, never a restore sleep.
            let end = Instant::now() + NEGOTIATION_WAIT;
            while Instant::now() < end && !self.pending.as_ref().unwrap().negotiated {
                self.drain()?;
                ensure!(
                    self.owned()?,
                    "Clipboard ownership changed before paste input"
                );
                thread::sleep(Duration::from_millis(2));
            }
            ensure!(
                self.pending.as_ref().unwrap().negotiated,
                "Target did not complete supported clipboard negotiation before input"
            );
            // Close the ownership-check/chord submission gap with a short
            // server critical section. All checks use this same connection;
            // release before waiting for any application response.
            {
                let grab = ServerGrab::new(self.conn.clone())?;
                self.barrier()?;
                ensure!(
                    self.owned()?,
                    "Clipboard changed before paste chord submission"
                );
                self.focused(&request)?;
                crate::input::send_paste_xtest_checked(&self.conn, request.window_id, &submitted)?;
                grab.release()?;
            }
            let end = Instant::now() + TRANSFER_WAIT;
            while Instant::now() < end {
                self.drain()?;
                if self.pending.as_ref().unwrap().transferred {
                    return Ok(PasteOutcome {
                        clipboard_restoration: self.restore()?,
                        transfer_verified: true,
                    });
                }
                ensure!(
                    self.owned()?,
                    "Clipboard owner changed before target text transfer completed"
                );
                thread::sleep(Duration::from_millis(2));
            }
            bail!("Paste input submitted; target transfer unverified. Payload remains available; do not replay")
        })();
        result.map_err(|error| {
            let input_submitted = submitted.load(Ordering::SeqCst);
            let clipboard_state = if claimed && !input_submitted && self.pending.is_some() {
                self.restore().unwrap_or_else(|_| "unknown".into())
            } else if claimed && self.pending.is_some() {
                match self.barrier().and_then(|_| self.owned()) {
                    Ok(false) => {
                        self.pending = None;
                        "skipped_owner_changed".into()
                    }
                    Ok(true) => "pending_transfer".into(),
                    Err(_) => "unknown".into(),
                }
            } else if claimed {
                "restoration_unverified".into()
            } else {
                "unchanged".into()
            };
            PasteError {
                message: format!("{error:#}"),
                input_submitted,
                clipboard_state,
            }
        })
    }
    fn put(&self, owner: Window, requestor: Window, property: Atom, target: Atom) -> Result<bool> {
        let Some(offer) = self.offers.iter().find(|o| o.window == owner) else {
            return Ok(false);
        };
        if target == self.atoms.targets {
            let targets: Vec<_> = [
                self.atoms.targets,
                self.atoms.timestamp,
                self.atoms.multiple,
            ]
            .into_iter()
            .chain(offer.data.iter().map(|d| d.target))
            .collect();
            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    requestor,
                    property,
                    AtomEnum::ATOM,
                    &targets,
                )?
                .check()?;
        } else if target == self.atoms.timestamp {
            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    requestor,
                    property,
                    AtomEnum::INTEGER,
                    &[offer.timestamp],
                )?
                .check()?;
        } else if let Some(data) = offer.data.iter().find(|d| d.target == target) {
            self.conn
                .change_property(
                    PropMode::REPLACE,
                    requestor,
                    property,
                    data.type_,
                    data.format,
                    (data.bytes.len() / (usize::from(data.format) / 8)) as u32,
                    &data.bytes,
                )?
                .check()?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
    fn serve(&mut self, request: SelectionRequestEvent) -> Result<()> {
        let mut property = if request.property == NONE {
            request.target
        } else {
            request.property
        };
        let valid = self.offers.iter().any(|o| {
            request.owner == o.window
                && request.selection == self.atoms.clipboard
                && (request.time == CURRENT_TIME
                    || request.time.wrapping_sub(o.timestamp) < (1 << 31))
        });
        if !valid {
            property = NONE;
        } else {
            let target_pid = self
                .pending
                .as_ref()
                .filter(|p| p.window == request.owner)
                .map(|p| p.pid);
            let correlated =
                target_pid.is_some_and(|pid| self.pid(request.requestor).ok() == Some(pid));
            self.conn
                .change_window_attributes(
                    request.requestor,
                    &ChangeWindowAttributesAux::new()
                        .event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY),
                )?
                .check()?;
            if request.target == self.atoms.multiple {
                let reply = self
                    .conn
                    .get_property(
                        false,
                        request.requestor,
                        property,
                        self.atoms.atom_pair,
                        0,
                        (MAX_FORMATS * 2) as u32,
                    )?
                    .reply()?;
                ensure!(
                    reply.type_ == self.atoms.atom_pair
                        && reply.format == 32
                        && reply.bytes_after == 0,
                    "Unsupported MULTIPLE request"
                );
                let mut pairs = words(&reply.value)?;
                ensure!(pairs.len() % 2 == 0, "Invalid MULTIPLE property");
                for pair in pairs.chunks_exact_mut(2) {
                    if pair[1] == NONE
                        || pair[0] == self.atoms.multiple
                        || !self.put(request.owner, request.requestor, pair[1], pair[0])?
                    {
                        pair[1] = NONE;
                    }
                }
                self.conn
                    .change_property32(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        self.atoms.atom_pair,
                        &pairs,
                    )?
                    .check()?;
                // MULTIPLE is served for restoration compatibility, but is not
                // a supported temporary-paste completion oracle yet.
            } else {
                let text = self.atoms.text.contains(&request.target);
                let submitted = self
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.submitted.load(Ordering::SeqCst));
                if self.put(request.owner, request.requestor, property, request.target)? {
                    if correlated && ((text && submitted) || request.target == self.atoms.targets) {
                        let p = self.pending.as_mut().unwrap();
                        p.reads
                            .retain(|r| r.window != request.requestor || r.property != property);
                        ensure!(p.reads.len() < 64, "Too many pending clipboard reads");
                        p.reads.push(Readback {
                            window: request.requestor,
                            property,
                            text,
                            saw_value: false,
                        });
                    }
                } else {
                    property = NONE;
                }
            }
        }
        self.notify(request, property)
    }
    fn notify(&self, request: SelectionRequestEvent, property: Atom) -> Result<()> {
        self.conn
            .send_event(
                false,
                request.requestor,
                EventMask::NO_EVENT,
                SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: request.time,
                    requestor: request.requestor,
                    selection: request.selection,
                    target: request.target,
                    property,
                },
            )?
            .check()?;
        self.conn.flush()?;
        Ok(())
    }
}
fn words(bytes: &[u8]) -> Result<Vec<u32>> {
    ensure!(bytes.len() % 4 == 0, "Unaligned X11 atom data");
    Ok(bytes
        .chunks_exact(4)
        .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atom_payload_keeps_native_property_width() {
        let values = [1_u32, u32::MAX, 0x12345678];
        let bytes: Vec<_> = values.iter().flat_map(|n| n.to_ne_bytes()).collect();
        assert_eq!(words(&bytes).unwrap(), values);
        assert!(words(&bytes[..11]).is_err());
    }
    #[test]
    fn unsupported_format_refuses_before_actor_creation() {
        let error = paste(PasteRequest {
            pid: 1,
            window_id: 1,
            text: "x".into(),
            format: "html".into(),
        })
        .unwrap_err();
        assert!(!error.input_submitted);
        assert_eq!(error.clipboard_state, "unchanged");
    }
}
