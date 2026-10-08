use serde_json::Value;
use sts2sim::rng::{deterministic_hash, Rng};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/rng.json")).unwrap()
}
fn u(v: &Value) -> u64 {
    v.as_str().unwrap().parse().unwrap()
}

#[test]
fn name_hashes_match() {
    for h in golden()["hashes"].as_array().unwrap() {
        assert_eq!(deterministic_hash(h["name"].as_str().unwrap()), u(&h["hash"]), "{}", h["name"]);
    }
}

#[test]
fn streams_match() {
    for s in golden()["streams"].as_array().unwrap() {
        let (seed, name) = (u(&s["seed"]), s["name"].as_str().unwrap());
        let mut r = Rng::named(seed, name);
        for (i, v) in s["ints"].as_array().unwrap().iter().enumerate() {
            assert_eq!(r.next_int(10) as i64, v.as_i64().unwrap(), "{seed} {name} ints[{i}]");
        }
        for (i, v) in s["ranged"].as_array().unwrap().iter().enumerate() {
            assert_eq!(r.next_int_range(3, 40) as i64, v.as_i64().unwrap(), "{seed} {name} ranged[{i}]");
        }
        for (i, v) in s["doubles"].as_array().unwrap().iter().enumerate() {
            let want: f64 = v.as_str().unwrap().parse().unwrap();
            assert_eq!(r.next_double().to_bits(), want.to_bits(), "{seed} {name} doubles[{i}]");
        }
        for (i, v) in s["float_bits"].as_array().unwrap().iter().enumerate() {
            let want: i32 = v.as_str().unwrap().parse().unwrap();
            assert_eq!(r.next_float().to_bits() as i32, want, "{seed} {name} floats[{i}]");
        }
        for (i, v) in s["bools"].as_array().unwrap().iter().enumerate() {
            assert_eq!(r.next_bool(), v.as_bool().unwrap(), "{seed} {name} bools[{i}]");
        }
        let ul = s["ulongs"].as_array().unwrap();
        for v in &ul[..4] {
            assert_eq!(r.next_u64(), u(v));
        }
        for v in &ul[4..] {
            assert_eq!(r.next_u64_below(1000), u(v));
        }
        assert_eq!(r.counter as i64, s["counter"].as_i64().unwrap(), "{seed} {name} counter");
    }
}

#[test]
fn shuffles_match() {
    for s in golden()["shuffles"].as_array().unwrap() {
        let seed = u(&s["seed"]);
        let n = s["n"].as_u64().unwrap() as usize;
        let mut r = Rng::named(seed, "shuffle");
        let mut v: Vec<i64> = (0..n as i64).collect();
        r.shuffle(&mut v);
        let want: Vec<i64> = s["order"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
        assert_eq!(v, want, "seed {seed} n {n}");
        assert_eq!(r.next_int(1000) as i64, s["next_after"].as_i64().unwrap(), "stream position after shuffle");
    }
}

#[test]
fn weighted_picks_match() {
    for s in golden()["weighted"].as_array().unwrap() {
        let mut r = Rng::named(u(&s["seed"]), "monster_ai");
        let w = [0.3f32, 0.5, 0.2, 1.0];
        for (i, v) in s["picks"].as_array().unwrap().iter().enumerate() {
            assert_eq!(r.weighted_index(&w).unwrap() as i64, v.as_i64().unwrap(), "pick {i}");
        }
    }
}
