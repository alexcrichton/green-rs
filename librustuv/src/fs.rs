// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::c_str::{mod, CString};
use std::io;
use std::mem;
use std::rt::task::BlockedTask;
use libc;

use {uvll, raw, UvResult, EventLoop, UvError};
use raw::Request;

pub struct File {
    fd: libc::c_int,
    path: Path,
}

struct Fs {
    handle: raw::Fs,
    fired: bool,
}

impl File {
    pub fn open(path: &Path) -> UvResult<File> {
        File::open_mode(path, io::Open, io::Read)
    }

    pub fn create(path: &Path) -> UvResult<File> {
        File::open_mode(path, io::Truncate, io::Write)
    }

    pub fn open_mode(path: &Path,
                     mode: io::FileMode,
                     access: io::FileAccess) -> UvResult<File> {
        let mut eloop = try!(EventLoop::borrow());
        File::open_mode_on(&mut *eloop, path, mode, access)
    }

    pub fn open_mode_on(eloop: &mut EventLoop,
                        path: &Path,
                        mode: io::FileMode,
                        access: io::FileAccess) -> UvResult<File> {
        let flags = match mode {
            io::Open => 0,
            io::Append => libc::O_APPEND,
            io::Truncate => libc::O_TRUNC,
        };
        // Opening with a write permission must silently create the file.
        let (flags, mode) = match access {
            io::Read => (flags | libc::O_RDONLY, 0),
            io::Write => (flags | libc::O_WRONLY | libc::O_CREAT,
                          libc::S_IRUSR | libc::S_IWUSR),
            io::ReadWrite => (flags | libc::O_RDWR | libc::O_CREAT,
                              libc::S_IRUSR | libc::S_IWUSR),
        };

        execute(|req, cb| unsafe {
            req.open(eloop.uv_loop(), path.to_c_str(), flags,
                     mode as libc::c_int, cb)
        }).map(|req| {
            File {
                path: path.clone(),
                fd: req.handle.result().unwrap() as libc::c_int,
            }
        })
    }

    /// Create a new `File` object for the specified descriptor.
    ///
    /// This function is unsafe as there is no knowledge of whether the file
    /// descriptor is a file or whether it is compatible with libuv. The file
    /// descriptor will be closed when the returned object goes out of scope.
    pub unsafe fn wrap(fd: libc::c_int, path: &Path) -> File {
        File { path: path.clone(), fd: fd }
    }

    pub fn path(&self) -> &Path { &self.path }

    pub fn fsync(&self) -> UvResult<()> {
        let eloop = try!(EventLoop::borrow());
        execute_nop(|req, cb| unsafe {
            req.fsync(eloop.uv_loop(), self.fd, cb)
        })
    }

    pub fn datasync(&self) -> UvResult<()> {
        let eloop = try!(EventLoop::borrow());
        execute_nop(|req, cb| unsafe {
            req.fdatasync(eloop.uv_loop(), self.fd, cb)
        })
    }

    pub fn truncate(&self, size: i64) -> UvResult<()> {
        let eloop = try!(EventLoop::borrow());
        execute_nop(|req, cb| unsafe {
            req.ftruncate(eloop.uv_loop(), self.fd, size, cb)
        })
    }

    pub fn stat(&self) -> UvResult<io::FileStat> {
        let eloop = try!(EventLoop::borrow());
        execute(|req, cb| unsafe {
            req.fstat(eloop.uv_loop(), self.fd, cb)
        }).map(|req| req.handle.io_stat())
    }

    /// Read some bytes at `pos`.
    ///
    /// If `pos` is -1, then the data will be read from the current position in
    /// the file.
    pub fn read_at(&mut self, into: &mut [u8], pos: i64) -> UvResult<uint> {
        let eloop = try!(EventLoop::borrow());
        execute(|req, cb| unsafe {
            req.read(eloop.uv_loop(), self.fd, into, pos, cb)
        }).and_then(|req| {
            match req.handle.result().unwrap() as uint {
                0 => Err(UvError(uvll::EOF)),
                n => Ok(n),
            }
        })
    }

    /// Write the contents of `buf` at position `pos`.
    ///
    /// If `pos` is -1, then the data will be written at the current position in
    /// the file.
    pub fn write_at(&mut self, buf: &[u8], pos: i64) -> UvResult<()> {
        let eloop = try!(EventLoop::borrow());
        let mut amt = 0;
        while amt < buf.len() {
            let pos = if pos == -1 {pos} else {pos + amt as i64};
            amt += try!(execute(|req, cb| unsafe {
                req.write(eloop.uv_loop(), self.fd, buf.slice_from(amt), pos, cb)
            }).map(|req| req.handle.result().unwrap() as uint));
        }
        Ok(())
    }

    fn seek_common(&self, pos: i64, whence: libc::c_int) -> io::IoResult<u64> {
        match unsafe { libc::lseek(self.fd, pos as libc::off_t, whence) } {
            -1 => Err(io::IoError::last_error()),
            n => Ok(n as u64)
        }
    }

    /// Gain access to the underlying raw file descriptor.
    ///
    /// This function is unsafe as there is no guarantee that any safe
    /// modifications to the timer handle are actually safe to perform given the
    /// assumptions of this object.
    pub unsafe fn fd(&self) -> libc::c_int { self.fd }
}

impl Reader for File {
    fn read(&mut self, into: &mut [u8]) -> io::IoResult<uint> {
        self.read_at(into, -1).map_err(|e| e.to_io_error())
    }
}

impl Writer for File {
    fn write(&mut self, buf: &[u8]) -> io::IoResult<()> {
        self.write_at(buf, -1).map_err(|e| e.to_io_error())
    }
}

impl Seek for File {
    fn tell(&self) -> io::IoResult<u64> {
        self.seek_common(0, libc::SEEK_CUR)
    }
    fn seek(&mut self, pos: i64, whence: io::SeekStyle) -> io::IoResult<()> {
        let whence = match whence {
            io::SeekSet => libc::SEEK_SET,
            io::SeekCur => libc::SEEK_CUR,
            io::SeekEnd => libc::SEEK_END
        };
        self.seek_common(pos, whence).map(|_| ())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let eloop = EventLoop::borrow().unwrap();
        let _ = execute_nop(|req, cb| unsafe {
            req.close(eloop.uv_loop(), self.fd, cb)
        });
    }
}

macro_rules! f(
    (
        pub fn $name_on:ident($eloop:ident: &mut EventLoop,
                              $($arg:ident: $t:ty),* ) -> $ret:ty $body:block
        as $name:ident
    ) => (
        pub fn $name($($arg: $t),*) -> $ret {
            let mut eloop = try!(EventLoop::borrow());
            $name_on(&mut *eloop $(,$arg)*)
        }

        pub fn $name_on($eloop: &mut EventLoop $(, $arg: $t)*) -> $ret $body
    )
)

f!(pub fn change_file_times_on(eloop: &mut EventLoop,
                               path: &Path,
                               atime: u64,
                               mtime: u64) -> UvResult<()> {
    // libuv takes seconds
    let atime = atime as libc::c_double / 1000.0;
    let mtime = mtime as libc::c_double / 1000.0;
    execute_nop(|req, cb| unsafe {
        req.utime(eloop.uv_loop(), path.to_c_str(), atime, mtime, cb)
    })
} as change_file_times)

f!(pub fn chmod_on(eloop: &mut EventLoop,
                   path: &Path,
                   mode: io::FilePermission) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.chmod(eloop.uv_loop(), path.to_c_str(), mode.bits() as libc::c_int, cb)
    })
} as chmod)

f!(pub fn chown_on(eloop: &mut EventLoop,
                   path: &Path,
                   uid: int,
                   gid: int) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.chown(eloop.uv_loop(), path.to_c_str(),
                  uid as uvll::uv_uid_t, gid as uvll::uv_gid_t, cb)
    })
} as chown)

f!(pub fn symlink_on(eloop: &mut EventLoop,
                     src: &Path,
                     dst: &Path) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.symlink(eloop.uv_loop(), src.to_c_str(), dst.to_c_str(), 0, cb)
    })
} as symlink)

f!(pub fn link_on(eloop: &mut EventLoop,
                  src: &Path,
                  dst: &Path) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.link(eloop.uv_loop(), src.to_c_str(), dst.to_c_str(), cb)
    })
} as link)

f!(pub fn mkdir_on(eloop: &mut EventLoop,
                   dir: &Path,
                   perm: io::FilePermission) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.mkdir(eloop.uv_loop(), dir.to_c_str(),
                  perm.bits() as libc::c_int, cb)
    })
} as mkdir)

f!(pub fn rmdir_on(eloop: &mut EventLoop,
                   dir: &Path) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.rmdir(eloop.uv_loop(), dir.to_c_str(), cb)
    })
} as rmdir)

f!(pub fn lstat_on(eloop: &mut EventLoop, path: &Path) -> UvResult<io::FileStat> {
    execute(|req, cb| unsafe {
        req.lstat(eloop.uv_loop(), path.to_c_str(), cb)
    }).map(|req| req.handle.io_stat())
} as lstat)

f!(pub fn stat_on(eloop: &mut EventLoop, path: &Path) -> UvResult<io::FileStat> {
    execute(|req, cb| unsafe {
        req.stat(eloop.uv_loop(), path.to_c_str(), cb)
    }).map(|req| req.handle.io_stat())
} as stat)

f!(pub fn readlink_on(eloop: &mut EventLoop, path: &Path) -> UvResult<Path> {
    execute(|req, cb| unsafe {
        req.readlink(eloop.uv_loop(), path.to_c_str(), cb)
    }).map(|req| {
        let result = unsafe {
            CString::new(req.handle.get_ptr() as *const libc::c_char, false)
        };
        Path::new(result.as_bytes_no_nul())
    })
} as readlink)

f!(pub fn rename_on(eloop: &mut EventLoop,
                    src: &Path,
                    dst: &Path) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.rename(eloop.uv_loop(), src.to_c_str(), dst.to_c_str(), cb)
    })
} as rename)

f!(pub fn unlink_on(eloop: &mut EventLoop,
                    path: &Path) -> UvResult<()> {
    execute_nop(|req, cb| unsafe {
        req.unlink(eloop.uv_loop(), path.to_c_str(), cb)
    })
} as unlink)

f!(pub fn readdir_on(eloop: &mut EventLoop,
                     path: &Path) -> UvResult<Vec<Path>> {
    execute(|req, cb| unsafe {
        req.readdir(eloop.uv_loop(), path.to_c_str(), 0, cb)
    }).map(|req| unsafe {
        let mut paths = vec!();
        let size = req.handle.result().unwrap() as uint;
        let cstr = req.handle.get_ptr() as *const libc::c_char;
        let _ = c_str::from_c_multistring(cstr, Some(size), |rel| {
            paths.push(path.join(rel.as_bytes_no_nul()));
        });
        paths
    })
} as readdir)

fn execute(f: |&mut raw::Fs, uvll::uv_fs_cb| -> UvResult<()>) -> UvResult<Fs> {
    unsafe {
        let mut raw = Fs { handle: Request::alloc(), fired: false };
        try!(f(&mut raw.handle, fs_cb));
        raw.fired = true;
        let mut slot = None;
        raw.handle.set_data(&mut slot as *mut _ as *mut _);
        ::block(raw.handle.uv_loop(), |task| {
            slot = Some(task);
        });
        return match raw.handle.result() {
            Ok(_) => Ok(raw),
            Err(e) => Err(e),
        }
    }

    extern fn fs_cb(req: *mut uvll::uv_fs_t) {
        unsafe {
            let raw: raw::Fs = Request::from_raw(req);
            let slot: &mut Option<BlockedTask> = mem::transmute(raw.get_data());
            ::wakeup(slot);
        }
    }
}

fn execute_nop(f: |&mut raw::Fs, uvll::uv_fs_cb| -> UvResult<()>) -> UvResult<()> {
    execute(f).map(|_| {})
}

impl Drop for Fs {
    fn drop(&mut self) {
        unsafe {
            if self.fired {
                self.handle.cleanup();
            }
            self.handle.free();
        }
    }
}

pub fn mkdir_recursive(path: &Path, mode: io::FilePermission) -> UvResult<()> {
    // tjc: if directory exists but with different permissions,
    // should we return false?
    match stat(path) {
        Ok(ref s) if s.kind == io::TypeDirectory => return Ok(()),
        _ => {}
    }

    let mut comps = path.components();
    let mut curpath = path.root_path().unwrap_or(Path::new("."));

    for c in comps {
        curpath.push(c);

        let result = mkdir(&curpath, mode);

        match result {
            Err(mkdir_err) => {
                // already exists ?
                if try!(stat(&curpath)).kind != io::TypeDirectory {
                    return Err(mkdir_err);
                }
            }
            Ok(()) => ()
        }
    }

    Ok(())
}

/// Removes a directory at this path, after removing all its contents. Use
/// carefully!
///
/// # Error
///
/// See `file::unlink` and `fs::readdir`
pub fn rmdir_recursive(path: &Path) -> UvResult<()> {
    let mut rm_stack = Vec::new();
    rm_stack.push(path.clone());

    while !rm_stack.is_empty() {
        let children = try!(readdir(rm_stack.last().unwrap()));

        let mut has_child_dir = false;

        // delete all regular files in the way and push subdirs
        // on the stack
        for child in children.into_iter() {
            let child_type = try!(lstat(&child));

            if child_type.kind == io::TypeDirectory {
                rm_stack.push(child);
                has_child_dir = true;
            } else {
                // we can carry on safely if the file is already gone
                // (eg: deleted by someone else since readdir)
                match unlink(&child) {
                    Ok(()) => (),
                    Err(ref e) if e.code() == uvll::ENOENT => (),
                    Err(e) => return Err(e)
                }
            }
        }

        // if no subdir was found, let's pop and delete
        if !has_child_dir {
            let d = rm_stack.pop().unwrap();
            match rmdir(&d) {
                Ok(()) => (),
                Err(ref e) if e.code() == uvll::ENOENT => (),
                Err(e) => return Err(e)
            }
        }
    }

    Ok(())
}

pub fn copy(from: &Path, to: &Path) -> UvResult<()> {
    let s = try!(stat(from));
    if s.kind != io::TypeFile {
        return Err(UvError(uvll::EINVAL))
    }

    let mut reader = try!(File::open(from));
    let mut writer = try!(File::create(to));
    let mut buf = [0, ..128 * 1024];

    loop {
        let amt = match reader.read_at(&mut buf, -1) {
            Ok(n) => n,
            Err(ref e) if e.code() == uvll::EOF => { break }
            Err(e) => return Err(e),
        };
        try!(writer.write_at(buf.slice_to(amt), -1));
    }

    chmod(to, try!(stat(from)).perm)
}
