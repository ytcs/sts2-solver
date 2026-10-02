//! Exact port of .NET `List<T>.Sort()` (`ArraySortHelper<T>.IntrospectiveSort`, .NET 9).
//!
//! The game reshuffles with `StableShuffle`, which sorts the cards (unstable!) by `(ModelId, upgrade level)` and
//! then Fisher-Yates shuffles. Cards that compare equal are *permuted* by the sort, so to reproduce draw
//! order bit-for-bit we must run the same algorithm. Verified in `docs/spec/03-cards-piles.md` §7.3 against the
//! real runtime.

/// Sorts `keys` ascending with `cmp` exactly like .NET's introsort.
pub fn intro_sort<T: Copy>(keys: &mut [T], cmp: impl Fn(&T, &T) -> i32) {
    let n = keys.len();
    if n < 2 {
        return;
    }
    let depth = 2 * (usize::BITS - 1 - (n as u32 as usize).leading_zeros()) as i32 + 2;
    intro(keys, 0, n as isize - 1, depth, &cmp);
}

fn swap_if_greater<T: Copy>(k: &mut [T], i: isize, j: isize, cmp: &impl Fn(&T, &T) -> i32) {
    if i != j && cmp(&k[i as usize], &k[j as usize]) > 0 {
        k.swap(i as usize, j as usize);
    }
}

fn intro<T: Copy>(k: &mut [T], lo: isize, mut hi: isize, mut depth: i32, cmp: &impl Fn(&T, &T) -> i32) {
    while hi > lo {
        let size = hi - lo + 1;
        if size <= 16 {
            if size == 2 {
                swap_if_greater(k, lo, hi, cmp);
                return;
            }
            if size == 3 {
                swap_if_greater(k, lo, hi - 1, cmp);
                swap_if_greater(k, lo, hi, cmp);
                swap_if_greater(k, hi - 1, hi, cmp);
                return;
            }
            insertion_sort(k, lo, hi, cmp);
            return;
        }
        if depth == 0 {
            heap_sort(k, lo, hi, cmp);
            return;
        }
        depth -= 1;
        let p = pick_pivot_and_partition(k, lo, hi, cmp);
        intro(k, p + 1, hi, depth, cmp);
        hi = p - 1;
    }
}

fn insertion_sort<T: Copy>(k: &mut [T], lo: isize, hi: isize, cmp: &impl Fn(&T, &T) -> i32) {
    for i in lo..hi {
        let t = k[(i + 1) as usize];
        let mut j = i;
        while j >= lo && cmp(&t, &k[j as usize]) < 0 {
            k[(j + 1) as usize] = k[j as usize];
            j -= 1;
        }
        k[(j + 1) as usize] = t;
    }
}

fn pick_pivot_and_partition<T: Copy>(k: &mut [T], lo: isize, hi: isize, cmp: &impl Fn(&T, &T) -> i32) -> isize {
    let mid = lo + ((hi - lo) >> 1);
    swap_if_greater(k, lo, mid, cmp);
    swap_if_greater(k, lo, hi, cmp);
    swap_if_greater(k, mid, hi, cmp);
    let pivot = k[mid as usize];
    k.swap(mid as usize, (hi - 1) as usize);
    let mut left = lo;
    let mut right = hi - 1;
    while left < right {
        left += 1;
        while cmp(&k[left as usize], &pivot) < 0 {
            left += 1;
        }
        right -= 1;
        while cmp(&pivot, &k[right as usize]) < 0 {
            right -= 1;
        }
        if left >= right {
            break;
        }
        k.swap(left as usize, right as usize);
    }
    if left != hi - 1 {
        k.swap(left as usize, (hi - 1) as usize);
    }
    left
}

fn heap_sort<T: Copy>(k: &mut [T], lo: isize, hi: isize, cmp: &impl Fn(&T, &T) -> i32) {
    let n = hi - lo + 1;
    let mut i = n / 2;
    while i >= 1 {
        down_heap(k, i, n, lo, cmp);
        i -= 1;
    }
    let mut i = n;
    while i > 1 {
        k.swap(lo as usize, (lo + i - 1) as usize);
        down_heap(k, 1, i - 1, lo, cmp);
        i -= 1;
    }
}

fn down_heap<T: Copy>(k: &mut [T], mut i: isize, n: isize, lo: isize, cmp: &impl Fn(&T, &T) -> i32) {
    let d = k[(lo + i - 1) as usize];
    while i <= n / 2 {
        let mut c = 2 * i;
        if c < n && cmp(&k[(lo + c - 1) as usize], &k[(lo + c) as usize]) < 0 {
            c += 1;
        }
        if !(cmp(&d, &k[(lo + c - 1) as usize]) < 0) {
            break;
        }
        k[(lo + i - 1) as usize] = k[(lo + c - 1) as usize];
        i = c;
    }
    k[(lo + i - 1) as usize] = d;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_like_a_sort() {
        let mut v: Vec<i32> = (0..500).map(|i| (i * 7919 % 101) as i32).collect();
        let mut w = v.clone();
        intro_sort(&mut v, |a, b| (a > b) as i32 - (a < b) as i32);
        w.sort();
        assert_eq!(v, w);
    }

    #[test]
    fn equal_keys_are_permuted_not_stable() {
        // (key, tag): three equal keys. The 3-element network swaps (lo,hi-1) etc. only when strictly greater, so
        // equal items stay put for n<=3; for n>=17 partitioning moves them. Just assert multiset preserved.
        let mut v: Vec<(i32, i32)> = (0..40).map(|i| (i % 3, i)).collect();
        intro_sort(&mut v, |a, b| (a.0 > b.0) as i32 - (a.0 < b.0) as i32);
        assert!(v.windows(2).all(|w| w[0].0 <= w[1].0));
        let mut tags: Vec<i32> = v.iter().map(|x| x.1).collect();
        tags.sort();
        assert_eq!(tags, (0..40).collect::<Vec<_>>());
    }
}
