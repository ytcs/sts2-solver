//! Tiny fixed-capacity containers (no heap, `Copy`/memcpy-clonable) used by the combat state.
//!
//! Storage is `MaybeUninit`: creating an `ArrayVec` is free (no zero-fill of the capacity), which matters because
//! the engine creates small temporary lists (listener snapshots, damage results) on every hook dispatch.
//! Only `[0, len)` is ever read.

use core::mem::MaybeUninit;

/// Ordered fixed-capacity vector. Overflow is a logic error (debug-asserted; silently dropped in release so a
/// pathological fight can never be UB, but capacities are sized so real fights cannot hit it).
pub struct ArrayVec<T: Copy, const N: usize> {
    len: u16,
    items: [MaybeUninit<T>; N],
}

impl<T: Copy, const N: usize> Clone for ArrayVec<T, N> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Copy, const N: usize> Copy for ArrayVec<T, N> {}

impl<T: Copy, const N: usize> Default for ArrayVec<T, N> {
    #[inline(always)]
    fn default() -> Self {
        Self { len: 0, items: [const { MaybeUninit::uninit() }; N] }
    }
}

impl<T: Copy, const N: usize> ArrayVec<T, N> {
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
        debug_assert!((self.len as usize) < N, "ArrayVec overflow");
        if (self.len as usize) < N {
            self.items[self.len as usize] = MaybeUninit::new(v);
            self.len += 1;
        }
    }
    /// `List<T>.Insert(index, v)`.
    pub fn insert(&mut self, index: usize, v: T) {
        debug_assert!((self.len as usize) < N, "ArrayVec overflow");
        let n = self.len as usize;
        if n >= N {
            return;
        }
        let index = index.min(n);
        self.items.copy_within(index..n, index + 1);
        self.items[index] = MaybeUninit::new(v);
        self.len += 1;
    }
    /// `List<T>.RemoveAt(index)` — preserves order.
    pub fn remove(&mut self, index: usize) -> T {
        let n = self.len as usize;
        assert!(index < n);
        let v = unsafe { self.items[index].assume_init() };
        self.items.copy_within(index + 1..n, index);
        self.len -= 1;
        v
    }
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }
    pub fn truncate(&mut self, n: usize) {
        if n < self.len as usize {
            self.len = n as u16;
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

impl<T: Copy + PartialEq, const N: usize> ArrayVec<T, N> {
    pub fn position(&self, v: T) -> Option<usize> {
        self.as_slice().iter().position(|x| *x == v)
    }
    pub fn contains(&self, v: T) -> bool {
        self.position(v).is_some()
    }
    /// Removes the first occurrence of `v` (order-preserving); returns whether it was present.
    pub fn remove_value(&mut self, v: T) -> bool {
        if let Some(i) = self.position(v) {
            self.remove(i);
            true
        } else {
            false
        }
    }
}

impl<T: Copy + core::fmt::Debug, const N: usize> core::fmt::Debug for ArrayVec<T, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Copy, const N: usize> core::ops::Index<usize> for ArrayVec<T, N> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: usize) -> &T {
        &self.as_slice()[i]
    }
}
impl<T: Copy, const N: usize> core::ops::IndexMut<usize> for ArrayVec<T, N> {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.as_mut_slice()[i]
    }
}
