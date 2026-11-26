// XDamage, only used to skip the full-screen SHM read while nothing on screen changes.
//
// A compositing window manager paints the desktop into a full-screen child of the root (mutter's
// stage, the Composite overlay window) and never into the root itself, while a plain one lets each
// application paint its own window. So the damage objects cover the root and its direct children,
// not the root alone. Mapping or raising a window damages the root, so re-reading the set on every
// report is enough: what a new window painted before the report is part of the capture that
// follows, and the object created at that point covers what it paints next.
//
// dlopen'd like libdrmtap: libxcb-damage0 is not part of a default desktop install, and a hard
// link would keep rustdesk from starting on a machine that does not ship it.

use super::ffi::{
    xcb_connection_t, xcb_flush, xcb_generate_id, xcb_get_geometry_reply,
    xcb_get_geometry_unchecked, xcb_poll_for_event, xcb_query_extension, xcb_query_extension_reply,
    xcb_query_tree_reply, xcb_query_tree_reply_t, xcb_query_tree_unchecked, xcb_window_t,
};
use hbb_common::{
    libc::{self, c_void},
    libloading::Library,
    log,
};
use std::{mem, slice};

type FnCreate = unsafe extern "C" fn(*mut xcb_connection_t, u32, u32, u8);
type FnSubtract = unsafe extern "C" fn(*mut xcb_connection_t, u32, u32, u32);
type FnDestroy = unsafe extern "C" fn(*mut xcb_connection_t, u32);
type FnQueryVersion = unsafe extern "C" fn(*mut xcb_connection_t, u32, u32) -> u32;
// The reply is only checked for being present, so its fields are not declared.
type FnQueryVersionReply =
    unsafe extern "C" fn(*mut xcb_connection_t, u32, *mut *mut c_void) -> *mut c_void;

const XCB_DAMAGE_REPORT_LEVEL_NON_EMPTY: u8 = 3;
// XCB_XFIXES_REGION_NONE: subtracting no explicit region clears the whole damage region.
const XFIXES_REGION_NONE: u32 = 0;
// One full capture per this many silent frames. Without it, a window the damage objects do not
// cover (a plain window manager's deeply nested window, a compositor that paints into a pixmap)
// would freeze the picture for good.
const VERIFY_EVERY: u32 = 60;

pub enum Hint {
    /// The server reported damage, so the screen may have changed.
    Changed,
    /// Nothing changed, the caller may return without reading the framebuffer.
    Idle,
    /// Nothing changed, but the periodic full capture is due, so the server's silence can be
    /// checked against the pixels at least once in a while.
    Verify,
}

pub struct Damage {
    // Kept alive for as long as the fn pointers below may be called.
    _lib: Library,
    create: FnCreate,
    subtract: FnSubtract,
    destroy: FnDestroy,
    first_event: u8,
    root: xcb_window_t,
    /// Damage object per window, covering `root` and its direct children.
    targets: Vec<(xcb_window_t, u32)>,
    idle: u32,
}

impl Damage {
    pub fn new(c: *mut xcb_connection_t, root: xcb_window_t) -> Option<Self> {
        let lib = match unsafe { Library::new("libxcb-damage.so.0") } {
            Ok(lib) => lib,
            Err(err) => {
                log::info!(
                    "no libxcb-damage, unchanged frames still cost a full screen read: {err}"
                );
                return None;
            }
        };
        unsafe {
            let create: FnCreate = *lib.get(b"xcb_damage_create\0").ok()?;
            let subtract: FnSubtract = *lib.get(b"xcb_damage_subtract\0").ok()?;
            let destroy: FnDestroy = *lib.get(b"xcb_damage_destroy\0").ok()?;
            let query_version: FnQueryVersion = *lib.get(b"xcb_damage_query_version\0").ok()?;
            let query_version_reply: FnQueryVersionReply =
                *lib.get(b"xcb_damage_query_version_reply\0").ok()?;
            let cookie = xcb_query_extension(c, 6, b"DAMAGE\0".as_ptr() as *const i8);
            let reply = xcb_query_extension_reply(c, cookie, std::ptr::null_mut());
            if reply.is_null() {
                return None;
            }
            let (present, first_event) = ((*reply).present, (*reply).first_event);
            libc::free(reply as *mut c_void);
            if present == 0 {
                return None;
            }
            // Until the extension's version request has been made on this connection, the server
            // answers DamageCreate with BadRequest, so every create below would be lost.
            let reply = query_version_reply(c, query_version(c, 1, 1), std::ptr::null_mut());
            if reply.is_null() {
                log::info!(
                    "xdamage version request failed, unchanged frames still cost a full screen read"
                );
                return None;
            }
            libc::free(reply);
            let mut damage = Self {
                _lib: lib,
                create,
                subtract,
                destroy,
                first_event,
                root,
                targets: Vec::new(),
                idle: 0,
            };
            damage.sync(c);
            Some(damage)
        }
    }

    /// Makes the damage objects match the root window's direct children.
    fn sync(&mut self, c: *mut xcb_connection_t) {
        unsafe {
            let cookie = xcb_query_tree_unchecked(c, self.root);
            let reply = xcb_query_tree_reply(c, cookie, std::ptr::null_mut());
            if reply.is_null() {
                return;
            }
            let children_ptr = (reply as *const u8).add(mem::size_of::<xcb_query_tree_reply_t>())
                as *const xcb_window_t;
            let children = slice::from_raw_parts(children_ptr, (*reply).children_len as usize);
            let mut i = 0;
            while i < self.targets.len() {
                let window = self.targets[i].0;
                if window != self.root && !children.contains(&window) {
                    (self.destroy)(c, self.targets.swap_remove(i).1);
                } else {
                    i += 1;
                }
            }
            for &window in children {
                if !self.targets.iter().any(|target| target.0 == window) && !input_only(c, window) {
                    let id = xcb_generate_id(c);
                    (self.create)(c, id, window, XCB_DAMAGE_REPORT_LEVEL_NON_EMPTY);
                    self.targets.push((window, id));
                }
            }
            if !self.targets.iter().any(|target| target.0 == self.root) {
                let id = xcb_generate_id(c);
                (self.create)(c, id, self.root, XCB_DAMAGE_REPORT_LEVEL_NON_EMPTY);
                self.targets.push((self.root, id));
            }
            libc::free(reply as *mut c_void);
        }
    }

    /// Clears the damage regions and reports what the server said about them.
    pub fn hint(&mut self, c: *mut xcb_connection_t) -> Hint {
        self.idle += 1;
        let changed = unsafe {
            let subtract = self.subtract;
            for &(_, id) in self.targets.iter() {
                subtract(c, id, XFIXES_REGION_NONE, XFIXES_REGION_NONE);
            }
            xcb_flush(c);
            let mut changed = false;
            loop {
                let event = xcb_poll_for_event(c);
                if event.is_null() {
                    break;
                }
                // Nothing else reads this connection, so whatever sits in the queue was caused by
                // our own requests and is safe to drop.
                changed = changed || ((*event).response_type & 0x7f) == self.first_event;
                libc::free(event as *mut c_void);
            }
            changed
        };
        if changed {
            // Pick up windows that appeared or went away before the caller captures.
            self.sync(c);
            self.idle = 0;
            return Hint::Changed;
        }
        if self.idle >= VERIFY_EVERY {
            self.idle = 0;
            return Hint::Verify;
        }
        Hint::Idle
    }

    pub fn destroy(self, c: *mut xcb_connection_t) {
        unsafe {
            for &(_, id) in self.targets.iter() {
                (self.destroy)(c, id);
            }
        }
    }
}

// An InputOnly window has depth 0 and no pixels, and damage on one is a BadMatch. The usual one
// covers the whole screen and sits above everything, so it must not be re-subtracted every frame.
unsafe fn input_only(c: *mut xcb_connection_t, window: xcb_window_t) -> bool {
    let reply = xcb_get_geometry_reply(
        c,
        xcb_get_geometry_unchecked(c, window),
        std::ptr::null_mut(),
    );
    if reply.is_null() {
        return true;
    }
    let depth = (*reply).depth;
    libc::free(reply as *mut c_void);
    depth == 0
}
