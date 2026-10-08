pub fn xxh64(data: &[u8], seed: u64) -> u64 {
    const P1: u64 = 0x9E3779B185EBCA87;
    const P2: u64 = 0xC2B2AE3D27D4EB4F;
    const P3: u64 = 0x165667B19E3779F9;
    const P4: u64 = 0x85EBCA77C2B2AE63;
    const P5: u64 = 0x27D4EB2F165667C5;
    #[inline]
    fn rd64(b: &[u8]) -> u64 {
        u64::from_le_bytes(b[..8].try_into().unwrap())
    }
    #[inline]
    fn rd32(b: &[u8]) -> u64 {
        u32::from_le_bytes(b[..4].try_into().unwrap()) as u64
    }
    #[inline]
    fn round(acc: u64, input: u64) -> u64 {
        acc.wrapping_add(input.wrapping_mul(P2)).rotate_left(31).wrapping_mul(P1)
    }
    #[inline]
    fn merge(acc: u64, val: u64) -> u64 {
        (acc ^ round(0, val)).wrapping_mul(P1).wrapping_add(P4)
    }
    let len = data.len();
    let mut p = data;
    let mut h: u64;
    if len >= 32 {
        let mut v1 = seed.wrapping_add(P1).wrapping_add(P2);
        let mut v2 = seed.wrapping_add(P2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(P1);
        while p.len() >= 32 {
            v1 = round(v1, rd64(&p[0..]));
            v2 = round(v2, rd64(&p[8..]));
            v3 = round(v3, rd64(&p[16..]));
            v4 = round(v4, rd64(&p[24..]));
            p = &p[32..];
        }
        h = v1.rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        h = merge(h, v1);
        h = merge(h, v2);
        h = merge(h, v3);
        h = merge(h, v4);
    } else {
        h = seed.wrapping_add(P5);
    }
    h = h.wrapping_add(len as u64);
    while p.len() >= 8 {
        h ^= round(0, rd64(p));
        h = h.rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        p = &p[8..];
    }
    if p.len() >= 4 {
        h ^= rd32(p).wrapping_mul(P1);
        h = h.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        p = &p[4..];
    }
    for &b in p {
        h ^= (b as u64).wrapping_mul(P5);
        h = h.rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

#[inline]
pub fn deterministic_hash(s: &str) -> u64 {
    xxh64(s.as_bytes(), 0)
}

#[inline]
fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
    pub counter: i32,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        let s = [splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x)];
        Rng { s, counter: 0 }
    }

    pub fn state(&self) -> [u64; 4] {
        self.s
    }

    pub fn from_state(counter: i32, s: [u64; 4]) -> Self {
        Rng { s, counter }
    }

    pub fn named(seed: u64, name: &str) -> Self {
        Self::new(seed.wrapping_add(deterministic_hash(name)))
    }

    #[inline(always)]
    fn next_u64_inner(&mut self) -> u64 {
        let [mut s0, mut s1, mut s2, mut s3] = self.s;
        let result = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s1 << 17;
        s2 ^= s0;
        s3 ^= s1;
        s1 ^= s2;
        s0 ^= s3;
        s2 ^= t;
        s3 = s3.rotate_left(45);
        self.s = [s0, s1, s2, s3];
        result
    }

    #[inline(always)]
    fn unit_f64(&mut self) -> f64 {
        (self.next_u64_inner() >> 11) as f64 * 1.1102230246251565e-16
    }

    #[inline]
    pub fn next_int(&mut self, max_exclusive: i32) -> i32 {
        debug_assert!(max_exclusive >= 1);
        self.counter = self.counter.wrapping_add(1);
        (self.unit_f64() * max_exclusive as f64) as i32
    }

    #[inline]
    pub fn next_int_range(&mut self, min_inclusive: i32, max_exclusive: i32) -> i32 {
        debug_assert!(min_inclusive < max_exclusive);
        self.counter = self.counter.wrapping_add(1);
        let span = max_exclusive as i64 - min_inclusive as i64;
        if span <= i32::MAX as i64 {
            (self.unit_f64() * span as f64) as i32 + min_inclusive
        } else {
            ((self.unit_f64() * span as f64) as i64 + min_inclusive as i64) as i32
        }
    }

    #[inline]
    pub fn next_bool(&mut self) -> bool {
        self.counter = self.counter.wrapping_add(1);
        (self.unit_f64() * 2.0) as i32 == 0
    }

    #[inline]
    pub fn next_double(&mut self) -> f64 {
        self.counter = self.counter.wrapping_add(1);
        self.unit_f64()
    }

    #[inline]
    pub fn next_float(&mut self) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        (self.unit_f64() * 1.0f64) as f32
    }

    #[inline]
    pub fn next_float_max(&mut self, max: f32) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        (self.unit_f64() * max as f64) as f32
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.counter = self.counter.wrapping_add(1);
        self.next_u64_inner()
    }

    #[inline]
    pub fn next_u64_below(&mut self, max_exclusive: u64) -> u64 {
        if max_exclusive == u64::MAX {
            return self.next_u64();
        }
        self.counter = self.counter.wrapping_add(1);
        (self.unit_f64() * max_exclusive as f64) as u64
    }

    pub fn shuffle<T>(&mut self, list: &mut [T]) {
        let mut i = list.len();
        while i > 1 {
            i -= 1;
            let j = self.next_int(i as i32 + 1) as usize;
            list.swap(i, j);
        }
    }

    pub fn weighted_index(&mut self, weights: &[f32]) -> Option<usize> {
        let r = self.next_float();
        Self::weighted_index_with(r, weights)
    }

    pub fn weighted_index_with(rand_input: f32, weights: &[f32]) -> Option<usize> {
        let mut total = 0f64;
        for &w in weights {
            total += w as f64;
        }
        let mut x = rand_input * total as f32;
        for (i, &w) in weights.iter().enumerate() {
            x -= w;
            if x <= 0.0 {
                return Some(i);
            }
        }
        None
    }
}
