use super::damage::{Damage, Hint};
use super::ffi::*;
use super::Display;
use hbb_common::{libc, log};
use std::{io, ptr, slice};

pub struct Capturer {
    display: Display,
    shmid: i32,
    xcbid: u32,
    buffer: *const u8,

    size: usize,
    last_hash: Option<u64>, // for faster compare, see `crate::frame_hash`
    damage: Option<Damage>,
    damage_misses: u32,
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        // Calculate dimensions.

        let pixel_width = display.pixfmt().bytes_per_pixel();
        let rect = display.rect();
        let size = (rect.w as usize) * (rect.h as usize) * pixel_width;

        // Create a shared memory segment.

        let shmid = unsafe {
            libc::shmget(
                libc::IPC_PRIVATE,
                size,
                // Everyone can do anything.
                libc::IPC_CREAT | 0o777,
            )
        };

        if shmid == -1 {
            return Err(io::Error::last_os_error());
        }

        // Attach the segment to a readable address.

        let buffer = unsafe { libc::shmat(shmid, ptr::null(), libc::SHM_RDONLY) } as *mut u8;

        if buffer as isize == -1 {
            return Err(io::Error::last_os_error());
        }

        // Attach the segment to XCB.

        let server = display.server().raw();
        let xcbid = unsafe { xcb_generate_id(server) };
        unsafe {
            xcb_shm_attach(
                server,
                xcbid,
                shmid as u32,
                0, // False, i.e. not read-only.
            );
        }

        let damage = Damage::new(server, display.root());

        let c = Capturer {
            display,
            shmid,
            xcbid,
            buffer,
            size,
            last_hash: None,
            damage,
            damage_misses: 0,
        };
        Ok(c)
    }

    pub fn display(&self) -> &Display {
        &self.display
    }

    fn get_image(&self) {
        let rect = self.display.rect();
        unsafe {
            let request = xcb_shm_get_image_unchecked(
                self.display.server().raw(),
                self.display.root(),
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                !0,
                XCB_IMAGE_FORMAT_Z_PIXMAP,
                self.xcbid,
                0,
            );
            let response =
                xcb_shm_get_image_reply(self.display.server().raw(), request, ptr::null_mut());
            libc::free(response as *mut _);
        }
    }

    pub fn frame<'b>(&'b mut self) -> std::io::Result<&'b [u8]> {
        let mut verify = false;
        // The first frame is always captured: `last_hash` stays None until one is delivered.
        if self.last_hash.is_some() {
            if let Some(damage) = self.damage.as_mut() {
                match damage.hint(self.display.server().raw()) {
                    Hint::Idle => return Err(io::ErrorKind::WouldBlock.into()),
                    Hint::Changed => {}
                    Hint::Verify => verify = true,
                }
            }
        }
        self.get_image();
        let result = unsafe { slice::from_raw_parts(self.buffer, self.size) };
        let hash = crate::frame_hash(result);
        if verify {
            if self.last_hash == Some(hash) {
                self.damage_misses = 0;
            } else {
                // A single miss can also be a change that landed between the poll and the read,
                // so only two verifies in a row show that damage never reports this window
                // manager's painting: full captures only.
                self.damage_misses += 1;
                if self.damage_misses >= 2 {
                    log::info!(
                        "xdamage missed a screen change, falling back to full screen capture"
                    );
                    if let Some(damage) = self.damage.take() {
                        damage.destroy(self.display.server().raw());
                    }
                }
            }
        }
        if self.last_hash == Some(hash) {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.last_hash = Some(hash);
        Ok(result)
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        if let Some(damage) = self.damage.take() {
            damage.destroy(self.display.server().raw());
        }
        unsafe {
            // Detach segment from XCB.
            xcb_shm_detach(self.display.server().raw(), self.xcbid);
            // Detach segment from our space.
            libc::shmdt(self.buffer as *mut _);
            // Destroy the shared memory segment.
            libc::shmctl(self.shmid, libc::IPC_RMID, ptr::null_mut());
        }
    }
}
