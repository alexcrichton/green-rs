// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::c_str::CString;
use std::io;
use libc;
use libc::c_int;

use raw::{Request, Allocated, Loop};
use {UvResult, UvError, uvll};

pub struct Fs {
    handle: *mut uvll::uv_fs_t,
}

impl Fs {
    /// This method is unsafe due to lack of knowledge of whether this handle is
    /// actively being used elsewhere.
    pub unsafe fn cleanup(&mut self) { uvll::uv_fs_req_cleanup(self.handle) }

    pub unsafe fn uv_loop(&self) -> Loop {
        Loop::from_raw(uvll::rust_uv_get_loop_from_fs_req(self.handle))
    }

    pub fn result(&self) -> UvResult<i64> {
        match unsafe { uvll::rust_uv_get_result_from_fs_req(self.handle) } {
            n if n < 0 => Err(UvError(n as i32)),
            n => Ok(n as i64)
        }
    }

    pub fn get_ptr(&self) -> *mut libc::c_void {
        unsafe { uvll::rust_uv_get_ptr_from_fs_req(self.handle) }
    }

    pub fn get_path(&self) -> *const libc::c_char {
        unsafe { uvll::rust_uv_get_path_from_fs_req(self.handle) }
    }

    pub fn uv_stat(&self) -> uvll::uv_stat_t {
        let mut stat = uvll::uv_stat_t::new();
        unsafe { uvll::rust_uv_populate_uv_stat(self.handle, &mut stat); }
        stat
    }

    pub fn io_stat(&self) -> io::FileStat {
        #[cfg(windows)] type Mode = libc::c_int;
        #[cfg(unix)] type Mode = libc::mode_t;

        let stat = self.uv_stat();
        return io::FileStat {
            size: stat.st_size as u64,
            kind: match (stat.st_mode as Mode) & libc::S_IFMT {
                libc::S_IFREG => io::TypeFile,
                libc::S_IFDIR => io::TypeDirectory,
                libc::S_IFIFO => io::TypeNamedPipe,
                libc::S_IFBLK => io::TypeBlockSpecial,
                libc::S_IFLNK => io::TypeSymlink,
                _ => io::TypeUnknown,
            },
            perm: io::FilePermission::from_bits_truncate(stat.st_mode as u32),
            created: to_msec(stat.st_birthtim),
            modified: to_msec(stat.st_mtim),
            accessed: to_msec(stat.st_atim),
            unstable: io::UnstableFileStat {
                device: stat.st_dev as u64,
                inode: stat.st_ino as u64,
                rdev: stat.st_rdev as u64,
                nlink: stat.st_nlink as u64,
                uid: stat.st_uid as u64,
                gid: stat.st_gid as u64,
                blksize: stat.st_blksize as u64,
                blocks: stat.st_blocks as u64,
                flags: stat.st_flags as u64,
                gen: stat.st_gen as u64,
            },
        };
        fn to_msec(stat: uvll::uv_timespec_t) -> u64 {
            // Be sure to cast to u64 first to prevent overflowing if the tv_sec
            // field is a 32-bit integer.
            (stat.tv_sec as u64) * 1000 + (stat.tv_nsec as u64) / 1000000
        }
    }

    pub fn close(&mut self, uv_loop: Loop, file: c_int, cb: uvll::uv_fs_cb)
                 -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_close(uv_loop.raw(), self.handle,
                                         file, cb)));
            Ok(())
        }
    }

    pub fn open(&mut self, uv_loop: Loop, path: CString,
                flags: c_int, mode: c_int,
                cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_open(uv_loop.raw(), self.handle,
                                        path.as_ptr(), flags, mode, cb)));
            Ok(())
        }
    }

    pub fn read(&mut self, uv_loop: Loop, file: c_int,
                buf: &mut [u8], offset: i64, cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            let buf = uvll::uv_buf_t {
                base: buf.as_mut_ptr(),
                len: buf.len() as uvll::uv_buf_len_t,
            };
            try!(call!(uvll::uv_fs_read(uv_loop.raw(), self.handle,
                                        file, &buf, 1, offset, cb)));
            Ok(())
        }
    }

    pub fn unlink(&mut self, uv_loop: Loop, path: CString,
                  cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_unlink(uv_loop.raw(), self.handle,
                                          path.as_ptr(), cb)));
            Ok(())
        }
    }

    pub fn write(&mut self, uv_loop: Loop, file: c_int,
                 buf: &[u8], offset: i64, cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            let buf = uvll::uv_buf_t {
                base: buf.as_ptr() as *mut _,
                len: buf.len() as uvll::uv_buf_len_t,
            };
            try!(call!(uvll::uv_fs_write(uv_loop.raw(), self.handle,
                                         file, &buf, 1, offset, cb)));
            Ok(())
        }
    }

    pub fn mkdir(&mut self, uv_loop: Loop, path: CString, mode: c_int,
                 cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_mkdir(uv_loop.raw(), self.handle,
                                         path.as_ptr(), mode, cb)));
            Ok(())
        }
    }

    pub fn rmdir(&mut self, uv_loop: Loop, path: CString,
                 cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_rmdir(uv_loop.raw(), self.handle,
                                         path.as_ptr(), cb)));
            Ok(())
        }
    }

    pub fn readdir(&mut self, uv_loop: Loop, path: CString, flags: c_int,
                   cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_readdir(uv_loop.raw(), self.handle,
                                           path.as_ptr(), flags, cb)));
            Ok(())
        }
    }

    pub fn stat(&mut self, uv_loop: Loop, path: CString,
                cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_stat(uv_loop.raw(), self.handle,
                                        path.as_ptr(), cb)));
            Ok(())
        }
    }

    pub fn lstat(&mut self, uv_loop: Loop, path: CString,
                 cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_lstat(uv_loop.raw(), self.handle,
                                         path.as_ptr(), cb)));
            Ok(())
        }
    }

    pub fn fstat(&mut self, uv_loop: Loop, file: c_int,
                cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_fstat(uv_loop.raw(), self.handle,
                                         file, cb)));
            Ok(())
        }
    }

    pub fn rename(&mut self, uv_loop: Loop, path: CString, new_path: CString,
                  cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_rename(uv_loop.raw(), self.handle,
                                          path.as_ptr(), new_path.as_ptr(),
                                          cb)));
            Ok(())
        }
    }

    pub fn link(&mut self, uv_loop: Loop, path: CString, new_path: CString,
                cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_link(uv_loop.raw(), self.handle,
                                        path.as_ptr(), new_path.as_ptr(),
                                        cb)));
            Ok(())
        }
    }

    pub fn symlink(&mut self, uv_loop: Loop, path: CString, new_path: CString,
                   flags: c_int, cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_symlink(uv_loop.raw(), self.handle,
                                           path.as_ptr(), new_path.as_ptr(),
                                           flags, cb)));
            Ok(())
        }
    }

    pub fn readlink(&mut self, uv_loop: Loop, path: CString,
                    cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_readlink(uv_loop.raw(), self.handle,
                                            path.as_ptr(), cb)));
            Ok(())
        }
    }

    pub fn chown(&mut self, uv_loop: Loop, path: CString,
                 uid: uvll::uv_uid_t, gid: uvll::uv_gid_t, cb: uvll::uv_fs_cb)
                 -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_chown(uv_loop.raw(), self.handle,
                                         path.as_ptr(), uid, gid, cb)));
            Ok(())
        }
    }

    pub fn fsync(&mut self, uv_loop: Loop, file: c_int,
                 cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_fsync(uv_loop.raw(), self.handle, file, cb)));
            Ok(())
        }
    }

    pub fn fdatasync(&mut self, uv_loop: Loop, file: c_int,
                     cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_fdatasync(uv_loop.raw(), self.handle, file,
                                             cb)));
            Ok(())
        }
    }

    pub fn ftruncate(&mut self, uv_loop: Loop, file: c_int, offset: i64,
                     cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_ftruncate(uv_loop.raw(), self.handle, file,
                                             offset, cb)));
            Ok(())
        }
    }

    pub fn chmod(&mut self, uv_loop: Loop, path: CString, mode: c_int,
                 cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_chmod(uv_loop.raw(), self.handle,
                                         path.as_ptr(), mode, cb)));
            Ok(())
        }
    }

    pub fn utime(&mut self, uv_loop: Loop, path: CString, atime: f64,
                 mtime: f64, cb: uvll::uv_fs_cb) -> UvResult<()> {
        unsafe {
            try!(call!(uvll::uv_fs_utime(uv_loop.raw(), self.handle,
                                         path.as_ptr(), atime, mtime, cb)));
            Ok(())
        }
    }
}

impl Allocated for uvll::uv_fs_t {
    fn size(_self: Option<uvll::uv_fs_t>) -> uint {
        unsafe { uvll::uv_req_size(uvll::UV_FS) as uint }
    }
}

impl Request<uvll::uv_fs_t> for Fs {
    fn raw(&self) -> *mut uvll::uv_fs_t { self.handle }
    fn from_raw(t: *mut uvll::uv_fs_t) -> Fs { Fs { handle: t } }
}

