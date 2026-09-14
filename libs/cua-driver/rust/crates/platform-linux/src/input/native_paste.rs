//! One checked paste chord on the clipboard transaction's own connection.
use anyhow::{bail, Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use x11rb::{
    connection::Connection,
    protocol::{
        xkb::{ConnectionExt as _, ID},
        xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT},
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
};

struct PressedKeys<'a> {
    conn: &'a RustConnection,
    keys: Vec<u8>,
}

impl Drop for PressedKeys<'_> {
    fn drop(&mut self) {
        // Only release keys this operation attempted to press. Never clear
        // unrelated user modifiers. A broken connection leaves delivery unknown.
        for key in self.keys.iter().rev() {
            if let Ok(cookie) =
                self.conn
                    .xtest_fake_input(KEY_RELEASE_EVENT, *key, 0, x11rb::NONE, 0, 0, 0)
            {
                let _ = cookie.check();
            }
        }
        let _ = self.conn.flush();
    }
}

fn prepare_paste_xtest(conn: &RustConnection, window_id: u32) -> Result<(u8, u8)> {
    let root = conn.query_tree(window_id)?.reply()?.root;
    let active = conn.intern_atom(true, b"_NET_ACTIVE_WINDOW")?.reply()?.atom;
    if !super::x11_target_already_focused(conn, root, active, window_id)? {
        bail!("native paste requires the exact target to own keyboard focus");
    }
    if !conn.xkb_use_extension(1, 0)?.reply()?.supported {
        bail!("native paste requires XKB keyboard-state inspection");
    }
    let minimum = conn.setup().min_keycode;
    let count = conn
        .setup()
        .max_keycode
        .checked_sub(minimum)
        .and_then(|v| v.checked_add(1))
        .context("unsupported keyboard range")?;
    let mapping = conn.get_keyboard_mapping(minimum, count)?.reply()?;
    let width = usize::from(mapping.keysyms_per_keycode);
    if width == 0 {
        bail!("empty keyboard mapping");
    }
    let rows: Vec<_> = mapping.keysyms.chunks_exact(width).collect();
    let v_key = rows
        .iter()
        .position(|row| row[0] == u32::from(b'v'))
        .and_then(|i| u8::try_from(i).ok())
        .and_then(|i| minimum.checked_add(i))
        .context("native paste requires an unshifted v in keyboard group one")?;
    let modifiers = conn.get_modifier_mapping()?.reply()?;
    let per_modifier = usize::from(modifiers.keycodes_per_modifier());
    if per_modifier == 0 {
        bail!("empty modifier mapping");
    }
    let row_for = |key: u8| {
        key.checked_sub(minimum)
            .and_then(|i| rows.get(usize::from(i)))
    };
    let control = modifiers.keycodes[2 * per_modifier..3 * per_modifier]
        .iter()
        .copied()
        .find(|&key| row_for(key).is_some_and(|row| matches!(row[0], 0xffe3 | 0xffe4)))
        .context("no Control key in the server Control modifier map")?;
    let state = conn.xkb_get_state(ID::USE_CORE_KBD.into())?.reply()?;
    if u8::from(state.group) != 0
        || u16::from(state.base_mods) != 0
        || u16::from(state.latched_mods) != 0
    {
        bail!("native paste requires keyboard group one and no held or latched modifiers");
    }
    // Permit only known lock keys; a layout's Mod2 need not mean NumLock.
    for index in 0..8 {
        if u16::from(state.locked_mods) & (1 << index) == 0 {
            continue;
        }
        let keys = &modifiers.keycodes[index * per_modifier..(index + 1) * per_modifier];
        if !keys.iter().any(|&key| key != 0)
            || keys.iter().filter(|&&key| key != 0).any(|&key| {
                !row_for(key).is_some_and(|row| matches!(row[0], 0xffe5 | 0xff7f | 0xff14))
            })
        {
            bail!("native paste cannot interpret the active locked modifier");
        }
    }
    let held = conn.query_keymap()?.reply()?.keys;
    if [control, v_key]
        .iter()
        .any(|&key| held[usize::from(key / 8)] & (1 << (key % 8)) != 0)
    {
        bail!("native paste will not release a key already held by another input source");
    }
    if !super::x11_target_already_focused(conn, root, active, window_id)? {
        bail!("native paste target lost keyboard focus before input");
    }
    Ok((control, v_key))
}

/// Run before claiming the clipboard; input delivery repeats this check.
pub(crate) fn validate_paste_xtest(conn: &RustConnection, window_id: u32) -> Result<()> {
    prepare_paste_xtest(conn, window_id).map(|_| ())
}

pub(crate) fn send_paste_xtest_checked(
    conn: &RustConnection,
    window_id: u32,
    input_submitted: &AtomicBool,
) -> Result<()> {
    let (control, v_key) = prepare_paste_xtest(conn, window_id)?;
    // These checks narrow the same external-focus race as checked XTest typing;
    // they do not establish an exclusive keyboard lease. Mark possible delivery
    // before the first request so an error cannot become a replayable refusal.
    let mut pressed = PressedKeys {
        conn,
        keys: Vec::with_capacity(2),
    };
    input_submitted.store(true, Ordering::SeqCst);
    for key in [control, v_key] {
        pressed.keys.push(key);
        conn.xtest_fake_input(KEY_PRESS_EVENT, key, 0, x11rb::NONE, 0, 0, 0)?
            .check()?;
    }
    while let Some(&key) = pressed.keys.last() {
        conn.xtest_fake_input(KEY_RELEASE_EVENT, key, 0, x11rb::NONE, 0, 0, 0)?
            .check()?;
        pressed.keys.pop();
    }
    conn.get_input_focus()?.reply()?;
    Ok(())
}
