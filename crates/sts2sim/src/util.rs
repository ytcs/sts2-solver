use core::cell::Cell;
use core::mem::MaybeUninit;

pub const OV_CONTAINER: u32 = 1;

thread_local! {
    static OVERFLOW: Cell<u32> = const { Cell::new(0) };
}

#[cold]
#[inline(never)]
pub fn raise_overflow(bit: u32) {
    if QUIET.with(|q| q.get()) {
        return;
    }
    if std::env::var("STS2_OVERFLOW_PANIC").is_ok() {
        panic!("capacity overflow raised (bit {bit})");
    }
    OVERFLOW.with(|c| c.set(c.get() | bit));
}

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

pub fn quiet<R>(f: impl FnOnce() -> R) -> R {
    let was = QUIET.with(|q| q.replace(true));
    let r = f();
    QUIET.with(|q| q.set(was));
    r
}

#[inline]
pub fn take_overflow() -> u32 {
    OVERFLOW.with(|c| c.replace(0))
}

macro_rules! fixed_vec {
    ($(#[$doc:meta])* $name:ident, $len:ty, $max:expr) => {
        $(#[$doc])*
        pub struct $name<T: Copy, const N: usize> {
            len: $len,
            items: [MaybeUninit<T>; N],
        }

        impl<T: Copy, const N: usize> Clone for $name<T, N> {
            #[inline(always)]
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<T: Copy, const N: usize> Copy for $name<T, N> {}

        impl<T: Copy, const N: usize> Default for $name<T, N> {
            #[inline(always)]
            fn default() -> Self {
                const { assert!(N <= $max, "capacity does not fit the length type") };
                Self { len: 0, items: [const { MaybeUninit::uninit() }; N] }
            }
        }

        impl<T: Copy, const N: usize> $name<T, N> {
            #[inline(always)]
            pub fn new() -> Self {
                Self::default()
            }
            #[inline(always)]
            pub fn len(&self) -> usize {
                self.len as usize
            }
            #[inline(always)]
            pub fn is_empty(&self) -> bool {
                self.len == 0
            }
            #[inline(always)]
            pub fn as_slice(&self) -> &[T] {
                // SAFETY: items[..len] are initialized (invariant maintained by every mutator).
                unsafe { core::slice::from_raw_parts(self.items.as_ptr() as *const T, self.len as usize) }
            }
            #[inline(always)]
            pub fn as_mut_slice(&mut self) -> &mut [T] {
                // SAFETY: as above.
                unsafe { core::slice::from_raw_parts_mut(self.items.as_mut_ptr() as *mut T, self.len as usize) }
            }
            #[inline(always)]
            pub fn push(&mut self, v: T) {
                if (self.len as usize) < N {
                    self.items[self.len as usize] = MaybeUninit::new(v);
                    self.len += 1;
                } else {
                    raise_overflow(OV_CONTAINER);
                }
            }
            pub fn insert(&mut self, index: usize, v: T) {
                let n = self.len as usize;
                if n >= N {
                    raise_overflow(OV_CONTAINER);
                    return;
                }
                let index = index.min(n);
                if index < n {
                    // SAFETY: index < n < N: source [index, n) and destination [index+1, n+1) are inside `items`.
                    unsafe {
                        let p = self.items.as_mut_ptr().add(index);
                        core::ptr::copy(p, p.add(1), n - index);
                    }
                }
                self.items[index] = MaybeUninit::new(v);
                self.len += 1;
            }
            pub fn remove(&mut self, index: usize) -> T {
                let n = self.len as usize;
                assert!(index < n);
                // SAFETY: index < n, so items[index] is initialized.
                let v = unsafe { self.items[index].assume_init() };
                if index + 1 < n {
                    // SAFETY: source [index+1, n) and destination [index, n-1) are inside `items`.
                    unsafe {
                        let p = self.items.as_mut_ptr().add(index);
                        core::ptr::copy(p.add(1), p, n - index - 1);
                    }
                }
                self.len -= 1;
                v
            }
            pub fn pop(&mut self) -> Option<T> {
                if self.len == 0 {
                    return None;
                }
                self.len -= 1;
                Some(unsafe { self.items[self.len as usize].assume_init() })
            }
            #[inline(always)]
            pub fn clear(&mut self) {
                self.len = 0;
            }
            #[inline(always)]
            pub fn copy_from(&mut self, src: &Self) {
                let n = src.len as usize;
                self.items[..n].copy_from_slice(&src.items[..n]);
                self.len = src.len;
            }
            pub fn truncate(&mut self, n: usize) {
                if n < self.len as usize {
                    self.len = n as $len;
                }
            }
            #[inline(always)]
            pub fn get(&self, i: usize) -> Option<T> {
                if i < self.len as usize { Some(self.as_slice()[i]) } else { None }
            }
            #[inline(always)]
            pub fn first(&self) -> Option<T> {
                self.get(0)
            }
            #[inline(always)]
            pub fn last(&self) -> Option<T> {
                if self.len == 0 { None } else { self.get(self.len as usize - 1) }
            }
            #[inline(always)]
            pub fn iter(&self) -> core::slice::Iter<'_, T> {
                self.as_slice().iter()
            }
        }

        impl<T: Copy + PartialEq, const N: usize> $name<T, N> {
            pub fn position(&self, v: T) -> Option<usize> {
                self.as_slice().iter().position(|x| *x == v)
            }
            pub fn contains(&self, v: T) -> bool {
                self.position(v).is_some()
            }
            pub fn remove_value(&mut self, v: T) -> bool {
                if let Some(i) = self.position(v) {
                    self.remove(i);
                    true
                } else {
                    false
                }
            }
        }

        impl<T: Copy + core::fmt::Debug, const N: usize> core::fmt::Debug for $name<T, N> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_list().entries(self.iter()).finish()
            }
        }

        impl<T: Copy, const N: usize> core::ops::Index<usize> for $name<T, N> {
            type Output = T;
            #[inline(always)]
            fn index(&self, i: usize) -> &T {
                &self.as_slice()[i]
            }
        }
        impl<T: Copy, const N: usize> core::ops::IndexMut<usize> for $name<T, N> {
            #[inline(always)]
            fn index_mut(&mut self, i: usize) -> &mut T {
                &mut self.as_mut_slice()[i]
            }
        }
    };
}

fixed_vec!(
    ArrayVec, u16, u16::MAX as usize
);

fixed_vec!(
    SmallVec, u8, u8::MAX as usize
);
