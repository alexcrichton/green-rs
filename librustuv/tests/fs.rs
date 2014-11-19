use std::str;
use std::os;
use std::rand::{mod, StdRng, Rng};
use std::io::{mod, Open, Read, SeekSet, SeekCur, SeekEnd, ReadWrite};
use std::io::fs::PathExtensions;

use rustuv::fs::{File, rmdir, mkdir, readdir, mkdir_recursive, rmdir_recursive,
                 unlink, stat, symlink, link, copy,
                 readlink, chmod, lstat, change_file_times};

macro_rules! check( ($e:expr) => (
    match $e {
        Ok(t) => t,
        Err(e) => panic!("{} failed with: {}", stringify!($e), e),
    }
) )

macro_rules! error( ($e:expr, $s:expr) => (
    match $e {
        Ok(..) => panic!("Should have been an error"),
        Err(ref err) => assert!(err.to_string().as_slice().contains($s.as_slice()),
                                format!("`{}` did not contain `{}`", err, $s))
    }
) )

pub struct TempDir(Path);

impl TempDir {
    fn join(&self, path: &str) -> Path {
        let TempDir(ref p) = *self;
        p.join(path)
    }

    fn path<'a>(&'a self) -> &'a Path {
        let TempDir(ref p) = *self;
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Gee, seeing how we're testing the fs module I sure hope that we
        // at least implement this correctly!
        let TempDir(ref p) = *self;
        check!(rmdir_recursive(p));
    }
}

pub fn tmpdir() -> TempDir {
    let ret = os::tmpdir().join(format!("rust-{}", rand::random::<u32>()));
    check!(mkdir(&ret, io::USER_RWX));
    TempDir(ret)
}

test!(fn file_test_io_smoke_test() {
    let message = "it's alright. have a good time";
    let tmpdir = tmpdir();
    let filename = &tmpdir.path().join("file_rt_io_file_test.txt");
    {
        let mut write_stream = check!(File::open_mode(filename, Open, ReadWrite));
        check!(write_stream.write(message.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open_mode(filename, Open, Read));
        let mut read_buf = [0, .. 1028];
        let read_str = match check!(read_stream.read(&mut read_buf)) {
            -1|0 => panic!("shouldn't happen"),
            n => str::from_utf8(read_buf.slice_to(n)).unwrap().to_string()
        };
        assert_eq!(read_str.as_slice(), message);
    }
    check!(unlink(filename));
})

test!(fn invalid_path_raises() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_that_does_not_exist.txt");
    let result = File::open_mode(filename, Open, Read);
    assert!(result.is_err());

    if cfg!(unix) {
        error!(result, "no such file or directory");
    }
})

test!(fn file_test_iounlinking_invalid_path_should_raise_condition() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_another_file_that_does_not_exist.txt");

    let result = unlink(filename);
    assert!(result.is_err());

    if cfg!(unix) {
        error!(result, "no such file or directory");
    }
})

test!(fn file_test_io_non_positional_read() {
    let message: &str = "ten-four";
    let mut read_mem = [0, .. 8];
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_positional.txt");
    {
        let mut rw_stream = check!(File::open_mode(filename, Open, ReadWrite));
        check!(rw_stream.write(message.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open_mode(filename, Open, Read));
        {
            let read_buf = read_mem.slice_mut(0, 4);
            check!(read_stream.read(read_buf));
        }
        {
            let read_buf = read_mem.slice_mut(4, 8);
            check!(read_stream.read(read_buf));
        }
    }
    check!(unlink(filename));
    let read_str = str::from_utf8(&read_mem).unwrap();
    assert_eq!(read_str, message);
})

test!(fn file_test_io_seek_and_tell_smoke_test() {
    let message = "ten-four";
    let mut read_mem = [0, .. 4];
    let set_cursor = 4 as u64;
    let mut tell_pos_pre_read;
    let mut tell_pos_post_read;
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_seeking.txt");
    {
        let mut rw_stream = check!(File::open_mode(filename, Open, ReadWrite));
        check!(rw_stream.write(message.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open_mode(filename, Open, Read));
        check!(read_stream.seek(set_cursor as i64, SeekSet));
        tell_pos_pre_read = check!(read_stream.tell());
        check!(read_stream.read(&mut read_mem));
        tell_pos_post_read = check!(read_stream.tell());
    }
    check!(unlink(filename));
    let read_str = str::from_utf8(&read_mem).unwrap();
    assert_eq!(read_str, message.slice(4, 8));
    assert_eq!(tell_pos_pre_read, set_cursor);
    assert_eq!(tell_pos_post_read, message.len() as u64);
})

test!(fn file_test_io_seek_and_write() {
    let initial_msg =   "food-is-yummy";
    let overwrite_msg =    "-the-bar!!";
    let final_msg =     "foo-the-bar!!";
    let seek_idx = 3i;
    let mut read_mem = [0, .. 13];
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_seek_and_write.txt");
    {
        let mut rw_stream = check!(File::open_mode(filename, Open, ReadWrite));
        check!(rw_stream.write(initial_msg.as_bytes()));
        check!(rw_stream.seek(seek_idx as i64, SeekSet));
        check!(rw_stream.write(overwrite_msg.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open_mode(filename, Open, Read));
        check!(read_stream.read(&mut read_mem));
    }
    check!(unlink(filename));
    let read_str = str::from_utf8(&read_mem).unwrap();
    assert!(read_str.as_slice() == final_msg.as_slice());
})

test!(fn file_test_io_seek_shakedown() {
    let initial_msg =   "qwer-asdf-zxcv";
    let chunk_one: &str = "qwer";
    let chunk_two: &str = "asdf";
    let chunk_three: &str = "zxcv";
    let mut read_mem = [0, .. 4];
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_rt_io_file_test_seek_shakedown.txt");
    {
        let mut rw_stream = check!(File::open_mode(filename, Open, ReadWrite));
        check!(rw_stream.write(initial_msg.as_bytes()));
    }
    {
        let mut read_stream = check!(File::open_mode(filename, Open, Read));

        check!(read_stream.seek(-4, SeekEnd));
        check!(read_stream.read(&mut read_mem));
        assert_eq!(str::from_utf8(&read_mem).unwrap(), chunk_three);

        check!(read_stream.seek(-9, SeekCur));
        check!(read_stream.read(&mut read_mem));
        assert_eq!(str::from_utf8(&read_mem).unwrap(), chunk_two);

        check!(read_stream.seek(0, SeekSet));
        check!(read_stream.read(&mut read_mem));
        assert_eq!(str::from_utf8(&read_mem).unwrap(), chunk_one);
    }
    check!(unlink(filename));
})

test!(fn file_test_stat_is_correct_on_is_file() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_stat_correct_on_is_file.txt");
    {
        let mut fs = check!(File::open_mode(filename, Open, ReadWrite));
        let msg = "hw";
        fs.write(msg.as_bytes()).unwrap();

        let fstat_res = check!(fs.stat());
        assert_eq!(fstat_res.kind, io::TypeFile);
    }
    let stat_res_fn = check!(stat(filename));
    assert_eq!(stat_res_fn.kind, io::TypeFile);
    let stat_res_meth = check!(filename.stat());
    assert_eq!(stat_res_meth.kind, io::TypeFile);
    check!(unlink(filename));
})

test!(fn file_test_stat_is_correct_on_is_dir() {
    let tmpdir = tmpdir();
    let filename = &tmpdir.join("file_stat_correct_on_is_dir");
    check!(mkdir(filename, io::USER_RWX));
    let stat_res_fn = check!(stat(filename));
    assert!(stat_res_fn.kind == io::TypeDirectory);
    let stat_res_meth = check!(filename.stat());
    assert!(stat_res_meth.kind == io::TypeDirectory);
    check!(rmdir(filename));
})

test!(fn file_test_fileinfo_false_when_checking_is_file_on_a_directory() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("fileinfo_false_on_dir");
    check!(mkdir(dir, io::USER_RWX));
    assert!(dir.is_file() == false);
    check!(rmdir(dir));
})

test!(fn file_test_fileinfo_check_exists_before_and_after_file_creation() {
    let tmpdir = tmpdir();
    let file = &tmpdir.join("fileinfo_check_exists_b_and_a.txt");
    check!(check!(File::create(file)).write(b"foo"));
    assert!(file.exists());
    check!(unlink(file));
    assert!(!file.exists());
})

test!(fn file_test_directoryinfo_check_exists_before_and_after_mkdir() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("before_and_after_dir");
    assert!(!dir.exists());
    check!(mkdir(dir, io::USER_RWX));
    assert!(dir.exists());
    assert!(dir.is_dir());
    check!(rmdir(dir));
    assert!(!dir.exists());
})

test!(fn file_test_directoryinfo_readdir() {
    let tmpdir = tmpdir();
    let dir = &tmpdir.join("di_readdir");
    check!(mkdir(dir, io::USER_RWX));
    let prefix = "foo";
    for n in range(0i,3) {
        let f = dir.join(format!("{}.txt", n));
        let mut w = check!(File::create(&f));
        let msg_str = format!("{}{}", prefix, n.to_string());
        let msg = msg_str.as_slice().as_bytes();
        check!(w.write(msg));
    }
    let files = check!(readdir(dir));
    let mut mem = [0u8, .. 4];
    for f in files.iter() {
        {
            let n = f.filestem_str();
            check!(check!(File::open(f)).read(&mut mem));
            let read_str = str::from_utf8(&mem).unwrap();
            let expected = match n {
                None|Some("") => panic!("really shouldn't happen.."),
                Some(n) => format!("{}{}", prefix, n),
            };
            assert_eq!(expected.as_slice(), read_str);
        }
        check!(unlink(f));
    }
    check!(rmdir(dir));
})

test!(fn recursive_mkdir() {
    let tmpdir = tmpdir();
    let dir = tmpdir.join("d1/d2");
    check!(mkdir_recursive(&dir, io::USER_RWX));
    assert!(dir.is_dir())
})

test!(fn recursive_mkdir_failure() {
    let tmpdir = tmpdir();
    let dir = tmpdir.join("d1");
    let file = dir.join("f1");

    check!(mkdir_recursive(&dir, io::USER_RWX));
    check!(File::create(&file));

    let result = mkdir_recursive(&file, io::USER_RWX);
    assert!(result.is_err());
})

test!(fn recursive_mkdir_slash() {
    check!(mkdir_recursive(&Path::new("/"), io::USER_RWX));
})

test!(fn recursive_rmdir() {
    let tmpdir = tmpdir();
    let d1 = tmpdir.join("d1");
    let dt = d1.join("t");
    let dtt = dt.join("t");
    let d2 = tmpdir.join("d2");
    let canary = d2.join("do_not_delete");
    check!(mkdir_recursive(&dtt, io::USER_RWX));
    check!(mkdir_recursive(&d2, io::USER_RWX));
    check!(check!(File::create(&canary)).write(b"foo"));
    check!(symlink(&d2, &dt.join("d2")));
    check!(rmdir_recursive(&d1));

    assert!(!d1.is_dir());
    assert!(canary.exists());
})

test!(fn unicode_path_is_dir() {
    assert!(Path::new(".").is_dir());
    assert!(!Path::new("test/stdtest/fs.rs").is_dir());

    let tmpdir = tmpdir();

    let mut dirpath = tmpdir.path().clone();
    dirpath.push(format!("test-가一ー你好"));
    check!(mkdir(&dirpath, io::USER_RWX));
    assert!(dirpath.is_dir());

    let mut filepath = dirpath;
    filepath.push("unicode-file-\uac00\u4e00\u30fc\u4f60\u597d.rs");
    check!(File::create(&filepath)); // ignore return; touch only
    assert!(!filepath.is_dir());
    assert!(filepath.exists());
})

test!(fn unicode_path_exists() {
    assert!(Path::new(".").exists());
    assert!(!Path::new("test/nonexistent-bogus-path").exists());

    let tmpdir = tmpdir();
    let unicode = tmpdir.path();
    let unicode = unicode.join(format!("test-각丁ー再见"));
    check!(mkdir(&unicode, io::USER_RWX));
    assert!(unicode.exists());
    assert!(!Path::new("test/unicode-bogus-path-각丁ー再见").exists());
})

test!(fn copy_file_does_not_exist() {
    let from = Path::new("test/nonexistent-bogus-path");
    let to = Path::new("test/other-bogus-path");

    match copy(&from, &to) {
        Ok(..) => panic!(),
        Err(..) => {
            assert!(!from.exists());
            assert!(!to.exists());
        }
    }
})

test!(fn copy_file_ok() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(check!(File::create(&input)).write(b"hello"));
    check!(copy(&input, &out));
    let contents = check!(check!(File::open(&out)).read_to_end());
    assert_eq!(contents.as_slice(), b"hello");

    assert_eq!(check!(input.stat()).perm, check!(out.stat()).perm);
})

test!(fn copy_file_dst_dir() {
    let tmpdir = tmpdir();
    let out = tmpdir.join("out");

    check!(File::create(&out));
    match copy(&out, tmpdir.path()) {
        Ok(..) => panic!(), Err(..) => {}
    }
})

test!(fn copy_file_dst_exists() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in");
    let output = tmpdir.join("out");

    check!(check!(File::create(&input)).write("foo".as_bytes()));
    check!(check!(File::create(&output)).write("bar".as_bytes()));
    check!(copy(&input, &output));

    assert_eq!(check!(check!(File::open(&output)).read_to_end()),
               (b"foo".to_vec()));
})

test!(fn copy_file_src_dir() {
    let tmpdir = tmpdir();
    let out = tmpdir.join("out");

    match copy(tmpdir.path(), &out) {
        Ok(..) => panic!(), Err(..) => {}
    }
    assert!(!out.exists());
})

test!(fn copy_file_preserves_perm_bits() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(File::create(&input));
    check!(chmod(&input, io::USER_READ));
    check!(copy(&input, &out));
    assert!(!check!(out.stat()).perm.intersects(io::USER_WRITE));

    check!(chmod(&input, io::USER_FILE));
    check!(chmod(&out, io::USER_FILE));
})

#[cfg(not(windows))] // FIXME(#10264) operation not permitted?
test!(fn symlinks_work() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(check!(File::create(&input)).write("foobar".as_bytes()));
    check!(symlink(&input, &out));
    if cfg!(not(windows)) {
        assert_eq!(check!(lstat(&out)).kind, io::TypeSymlink);
        assert_eq!(check!(out.lstat()).kind, io::TypeSymlink);
    }
    assert_eq!(check!(stat(&out)).size, check!(stat(&input)).size);
    assert_eq!(check!(check!(File::open(&out)).read_to_end()),
               (b"foobar".to_vec()));
})

#[cfg(not(windows))] // apparently windows doesn't like symlinks
test!(fn symlink_noexist() {
    let tmpdir = tmpdir();
    // symlinks can point to things that don't exist
    check!(symlink(&tmpdir.join("foo"), &tmpdir.join("bar")));
    assert!(check!(readlink(&tmpdir.join("bar"))) == tmpdir.join("foo"));
})

test!(fn readlink_not_symlink() {
    let tmpdir = tmpdir();
    match readlink(tmpdir.path()) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
})

test!(fn links_work() {
    let tmpdir = tmpdir();
    let input = tmpdir.join("in.txt");
    let out = tmpdir.join("out.txt");

    check!(check!(File::create(&input)).write("foobar".as_bytes()));
    check!(link(&input, &out));
    if cfg!(not(windows)) {
        assert_eq!(check!(lstat(&out)).kind, io::TypeFile);
        assert_eq!(check!(out.lstat()).kind, io::TypeFile);
        assert_eq!(check!(stat(&out)).unstable.nlink, 2);
        assert_eq!(check!(out.stat()).unstable.nlink, 2);
    }
    assert_eq!(check!(stat(&out)).size, check!(stat(&input)).size);
    assert_eq!(check!(stat(&out)).size, check!(input.stat()).size);
    assert_eq!(check!(check!(File::open(&out)).read_to_end()),
               (b"foobar".to_vec()));

    // can't link to yourself
    match link(&input, &input) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
    // can't link to something that doesn't exist
    match link(&tmpdir.join("foo"), &tmpdir.join("bar")) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }
})

test!(fn chmod_works() {
    let tmpdir = tmpdir();
    let file = tmpdir.join("in.txt");

    check!(File::create(&file));
    assert!(check!(stat(&file)).perm.contains(io::USER_WRITE));
    check!(chmod(&file, io::USER_READ));
    assert!(!check!(stat(&file)).perm.contains(io::USER_WRITE));

    match chmod(&tmpdir.join("foo"), io::USER_RWX) {
        Ok(..) => panic!("wanted a failure"),
        Err(..) => {}
    }

    check!(chmod(&file, io::USER_FILE));
})

test!(fn sync_doesnt_kill_anything() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("in.txt");

    let mut file = check!(File::open_mode(&path, io::Open, io::ReadWrite));
    check!(file.fsync());
    check!(file.datasync());
    check!(file.write(b"foo"));
    check!(file.fsync());
    check!(file.datasync());
    drop(file);
})

test!(fn truncate_works() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("in.txt");

    let mut file = check!(File::open_mode(&path, io::Open, io::ReadWrite));
    check!(file.write(b"foo"));
    check!(file.fsync());

    // Do some simple things with truncation
    assert_eq!(check!(file.stat()).size, 3);
    check!(file.truncate(10));
    assert_eq!(check!(file.stat()).size, 10);
    check!(file.write(b"bar"));
    check!(file.fsync());
    assert_eq!(check!(file.stat()).size, 10);
    assert_eq!(check!(check!(File::open(&path)).read_to_end()),
               (b"foobar\0\0\0\0".to_vec()));

    // Truncate to a smaller length, don't seek, and then write something.
    // Ensure that the intermediate zeroes are all filled in (we're seeked
    // past the end of the file).
    check!(file.truncate(2));
    assert_eq!(check!(file.stat()).size, 2);
    check!(file.write(b"wut"));
    check!(file.fsync());
    assert_eq!(check!(file.stat()).size, 9);
    assert_eq!(check!(check!(File::open(&path)).read_to_end()),
               (b"fo\0\0\0\0wut".to_vec()));
    drop(file);
})

test!(fn open_flavors() {
    let tmpdir = tmpdir();

    match File::open_mode(&tmpdir.join("a"), io::Open, io::Read) {
        Ok(..) => panic!(), Err(..) => {}
    }

    // Perform each one twice to make sure that it succeeds the second time
    // (where the file exists)
    check!(File::open_mode(&tmpdir.join("b"), io::Open, io::Write));
    assert!(tmpdir.join("b").exists());
    check!(File::open_mode(&tmpdir.join("b"), io::Open, io::Write));

    check!(File::open_mode(&tmpdir.join("c"), io::Open, io::ReadWrite));
    assert!(tmpdir.join("c").exists());
    check!(File::open_mode(&tmpdir.join("c"), io::Open, io::ReadWrite));

    check!(File::open_mode(&tmpdir.join("d"), io::Append, io::Write));
    assert!(tmpdir.join("d").exists());
    check!(File::open_mode(&tmpdir.join("d"), io::Append, io::Write));

    check!(File::open_mode(&tmpdir.join("e"), io::Append, io::ReadWrite));
    assert!(tmpdir.join("e").exists());
    check!(File::open_mode(&tmpdir.join("e"), io::Append, io::ReadWrite));

    check!(File::open_mode(&tmpdir.join("f"), io::Truncate, io::Write));
    assert!(tmpdir.join("f").exists());
    check!(File::open_mode(&tmpdir.join("f"), io::Truncate, io::Write));

    check!(File::open_mode(&tmpdir.join("g"), io::Truncate, io::ReadWrite));
    assert!(tmpdir.join("g").exists());
    check!(File::open_mode(&tmpdir.join("g"), io::Truncate, io::ReadWrite));

    check!(check!(File::create(&tmpdir.join("h"))).write("foo".as_bytes()));
    check!(File::open_mode(&tmpdir.join("h"), io::Open, io::Read));
    {
        let mut f = check!(File::open_mode(&tmpdir.join("h"), io::Open,
                                           io::Read));
        match f.write("wut".as_bytes()) {
            Ok(..) => panic!(), Err(..) => {}
        }
    }
    assert!(check!(stat(&tmpdir.join("h"))).size == 3,
            "write/stat failed");
    {
        let mut f = check!(File::open_mode(&tmpdir.join("h"), io::Append,
                                           io::Write));
        check!(f.write("bar".as_bytes()));
    }
    assert!(check!(stat(&tmpdir.join("h"))).size == 6,
            "append didn't append");
    {
        let mut f = check!(File::open_mode(&tmpdir.join("h"), io::Truncate,
                                           io::Write));
        check!(f.write("bar".as_bytes()));
    }
    assert!(check!(stat(&tmpdir.join("h"))).size == 3,
            "truncate didn't truncate");
})

test!(fn utime() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("a");
    check!(File::create(&path));
    // These numbers have to be bigger than the time in the day to account for timezones
    // Windows in particular will fail in certain timezones with small enough values
    check!(change_file_times(&path, 100000, 200000));
    assert_eq!(check!(path.stat()).accessed, 100000);
    assert_eq!(check!(path.stat()).modified, 200000);
})

test!(fn utime_noexist() {
    let tmpdir = tmpdir();

    match change_file_times(&tmpdir.join("a"), 100, 200) {
        Ok(..) => panic!(),
        Err(..) => {}
    }
})

test!(fn binary_file() {
    let mut bytes = [0, ..1024];
    StdRng::new().ok().unwrap().fill_bytes(&mut bytes);

    let tmpdir = tmpdir();

    check!(check!(File::create(&tmpdir.join("test"))).write(&bytes));
    let actual = check!(check!(File::open(&tmpdir.join("test"))).read_to_end());
    assert!(actual.as_slice() == &bytes);
})

test!(fn unlink_readonly() {
    let tmpdir = tmpdir();
    let path = tmpdir.join("file");
    check!(File::create(&path));
    check!(chmod(&path, io::USER_READ));
    check!(unlink(&path));
})

