use criterion::*;
use dashmap::DashMap;
use transgress_resolve::Map;

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

criterion_group!(benches, compare_hash_maps);
criterion_main!(benches);