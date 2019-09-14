use criterion::*;
use dashmap::DashMap;
use tendon_resolve as resolve;
use tendon_resolve::Map;
use parking_lot::RwLock;
use failure::_core::cell::RefCell;

fn compare_hash_maps(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare hash maps");

    const N: u64 = 1000;

    group.throughput(Throughput::Elements(N));

    group.bench_function("hashbrown loaded insert + merge", |b| {
        let mut parent = Map::default();
        for i in 0..10000u64 {
            parent.insert(i, i);
        }
        b.iter_batched_ref(
            || (parent.clone(), Map::default()),
            |(parent, map)| {
                for i in 0..N {
                    black_box(map.insert(black_box(i), black_box(i)));
                }
                black_box(parent.extend(black_box(map.drain())));
            },
            BatchSize::LargeInput,
        );
    });
    group.bench_function("dashmap loaded insert", |b| {
        b.iter_batched_ref(
            || {
                let map = DashMap::default();
                for i in 0..10000u64 {
                    map.insert(i, i);
                }
                map
            },
            |map| {
                for i in 0..N {
                    black_box(map.insert(black_box(i), black_box(i)));
                }
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("hashbrown insert + merge", |b| {
        let mut parent = Map::default();
        for i in 0..100u64 {
            parent.insert(i, i);
        }
        b.iter_batched_ref(
            || (parent.clone(), Map::default()),
            |(parent, map)| {
                for i in 0..N {
                    black_box(map.insert(black_box(i), black_box(i)));
                }
                black_box(parent.extend(black_box(map.drain())));
            },
            BatchSize::LargeInput,
        );
    });
    group.bench_function("dashmap insert", |b| {
        b.iter_batched_ref(
            || {
                let map = DashMap::default();
                for i in 0..100u64 {
                    map.insert(i, i);
                }
                map
            },
            |map| {
                for i in 0..N {
                    black_box(map.insert(black_box(i), black_box(i)));
                }
            },
            BatchSize::LargeInput,
        );
    });
}

fn lock_times(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock times");

    group.bench_function("uncontested RwLock read", |b| {
        let lock = RwLock::new(0);
        b.iter(|| black_box(lock.read()));
    });
    group.bench_function("uncontested RwLock write", |b| {
        let lock = RwLock::new(0);
        b.iter(|| black_box(lock.write()));
    });
    group.bench_function("uncontested RefCell read", |b| {
        let lock = RefCell::new(0);
        b.iter(|| black_box(lock.borrow()));
    });
    group.bench_function("uncontested RefCell write", |b| {
        let lock = RefCell::new(0);
        b.iter(|| black_box(lock.borrow_mut()));
    });
}

criterion_group!(benches, compare_hash_maps, lock_times);
criterion_main!(benches);
