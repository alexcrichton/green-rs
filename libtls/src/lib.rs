#![feature(macro_rules, unboxed_closures, unsafe_destructor)]

pub use imp::Key;

#[cfg(unix)]
mod imp {
    #![macro_escape]

    use std::cell::UnsafeCell;

    #[macro_export]
    macro_rules! tls(
        ($name:ident: $t:ty) => (
            #[thread_local]
            static mut $name: ::tls::Key<$t> = ::tls::Key {
                ptr: ::std::cell::UnsafeCell { value: 0 as *mut $t },
            };
        )
    )

    pub struct Key<T> {
        pub ptr: UnsafeCell<*mut T>,
    }

    struct Reset<T> {
        key: &'static Key<T>,
        val: *mut T,
    }

    impl<T> Key<T> {
        pub fn set<R>(&'static self, t: &T, cb: || -> R) -> R {
            let prev = unsafe {
                let prev = *self.ptr.get();
                *self.ptr.get() = t as *const _ as *mut T;
                prev
            };
            let _reset = Reset { key: self, val: prev };
            cb()
        }

        pub fn get<R>(&'static self, cb: |Option<&T>| -> R) -> R {
            unsafe {
                let ptr = *self.ptr.get();
                if ptr.is_null() {
                    cb(None)
                } else {
                    cb(Some(&*ptr))
                }
            }
        }
    }

    #[unsafe_destructor]
    impl<T> Drop for Reset<T> {
        fn drop(&mut self) {
            unsafe { *self.key.ptr.get() = self.val; }
        }
    }
}

// "macro hygiene strikes again"
mod tls {
    pub use Key;
}

#[cfg(test)]
mod test {

    #[test]
    fn simple() {
        tls!(foo: uint)

        unsafe {
            foo.get(|val| {
                assert_eq!(val, None);
            });

            foo.set(&1, || {
                foo.get(|val| {
                    assert_eq!(*val.unwrap(), 1);
                });

                let (tx, rx) = channel();
                spawn(proc() {
                    tx.send(foo.get(|val| {
                        assert_eq!(val, None);
                    }));
                });
                rx.recv();
            });
        }

    }
}
