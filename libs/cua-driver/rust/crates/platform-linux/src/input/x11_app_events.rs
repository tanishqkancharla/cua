//! Keep a borrowed keysym live until a participating focused X11 client has
//! processed the preceding input. A server round trip alone is insufficient
//! when the app is stalled. EWMH ping is event-loop evidence, not text readback.

use anyhow::{bail, Result};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::{xproto::*, Event};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub(super) struct AppEventBarrier<'a> {
    conn: &'a RustConnection,
    root: Window,
    client: Window,
    focus: Window,
    protocols: Atom,
    ping: Atom,
    probe: Window,
    timestamp_property: Atom,
}

impl<'a> AppEventBarrier<'a> {
    pub(super) fn focused(conn: &'a RustConnection) -> Result<Option<Self>> {
        let focus = conn.get_input_focus()?.reply()?.focus;
        if focus <= 1 {
            return Ok(None);
        }
        let root = conn.query_tree(focus)?.reply()?.root;
        let protocols = conn.intern_atom(false, b"WM_PROTOCOLS")?.reply()?.atom;
        let ping = conn.intern_atom(false, b"_NET_WM_PING")?.reply()?.atom;
        let mut client = focus;
        let mut supported = false;
        for _ in 0..32 {
            if client == root || client == 0 {
                break;
            }
            let property = conn
                .get_property(false, client, protocols, AtomEnum::ATOM, 0, 1024)?
                .reply()?;
            if property
                .value32()
                .is_some_and(|mut atoms| atoms.any(|atom| atom == ping))
            {
                supported = true;
                break;
            }
            client = conn.query_tree(client)?.reply()?.parent;
        }
        if !supported {
            return Ok(None);
        }
        // Event masks are per connection; Notify does not claim the WM's
        // exclusive SubstructureRedirect selection or alter its subscription.
        conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
        )?
        .check()?;
        let timestamp_property = conn
            .intern_atom(false, b"_OPENSKY_INPUT_ACK_TIME")?
            .reply()?
            .atom;
        let probe = conn.generate_id()?;
        conn.create_window(
            0,
            probe,
            root,
            -1,
            -1,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
        Ok(Some(Self {
            conn,
            root,
            client,
            focus,
            protocols,
            ping,
            probe,
            timestamp_property,
        }))
    }

    fn server_timestamp(&self) -> Result<Timestamp> {
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.probe,
                self.timestamp_property,
                AtomEnum::STRING,
                &[0],
            )?
            .check()?;
        self.conn.flush()?;
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            match self.conn.poll_for_event()? {
                Some(Event::PropertyNotify(event))
                    if event.window == self.probe
                        && event.atom == self.timestamp_property
                        && event.time != 0 =>
                {
                    return Ok(event.time)
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        }
        bail!("X11 input acknowledgement timestamp unavailable; text delivery may be partial, inspect before retrying")
    }

    pub(super) fn acknowledge(&self) -> Result<()> {
        let timestamp = self.server_timestamp()?;
        let request = ClientMessageEvent::new(
            32,
            self.client,
            self.protocols,
            [self.ping, timestamp, self.client, 0, 0],
        );
        self.conn
            .send_event(false, self.client, EventMask::NO_EVENT, request)?
            .check()?;
        self.conn.flush()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.conn.poll_for_event()? {
                Some(Event::ClientMessage(event))
                    if is_reply(
                        &event,
                        self.root,
                        self.protocols,
                        self.ping,
                        timestamp,
                        self.client,
                    ) =>
                {
                    if self.conn.get_input_focus()?.reply()?.focus != self.focus {
                        bail!("X11 input focus changed while awaiting application processing; text delivery may be partial, inspect before retrying");
                    }
                    return Ok(());
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        }
        bail!("X11 application did not acknowledge borrowed-key input within two seconds; delivery is unknown, inspect before retrying")
    }
}

impl Drop for AppEventBarrier<'_> {
    fn drop(&mut self) {
        let failure = match self.conn.destroy_window(self.probe) {
            Ok(cookie) => cookie.check().err().map(|error| error.to_string()),
            Err(error) => Some(error.to_string()),
        };
        if let Some(error) = failure {
            tracing::warn!(%error, "Failed to remove X11 input timestamp probe");
        }
    }
}

fn is_reply(
    event: &ClientMessageEvent,
    root: Window,
    protocols: Atom,
    ping: Atom,
    timestamp: Timestamp,
    client: Window,
) -> bool {
    event.window == root
        && event.type_ == protocols
        && event.format == 32
        && event.data.as_data32() == [ping, timestamp, client, 0, 0]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_exact_client_timestamp_and_protocol_reply_releases_a_borrowed_key() {
        let event = ClientMessageEvent::new(32, 1, 2u32, [3, 4, 5, 0, 0]);
        assert!(is_reply(&event, 1, 2, 3, 4, 5));
        for (root, protocols, ping, timestamp, client) in [
            (9, 2, 3, 4, 5),
            (1, 9, 3, 4, 5),
            (1, 2, 9, 4, 5),
            (1, 2, 3, 9, 5),
            (1, 2, 3, 4, 9),
        ] {
            assert!(!is_reply(&event, root, protocols, ping, timestamp, client));
        }
        let wrong_format = ClientMessageEvent::new(8, 1, 2u32, [3, 4, 5, 0, 0]);
        assert!(!is_reply(&wrong_format, 1, 2, 3, 4, 5));
        let changed_reserved_fields = ClientMessageEvent::new(32, 1, 2u32, [3, 4, 5, 8, 0]);
        assert!(!is_reply(&changed_reserved_fields, 1, 2, 3, 4, 5));
    }
}
