use criterion::*;
use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tendon_api::Map;

fn compare_hash_maps(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare hash maps");

    const N: u64 = 1000;

    group.throughput(Throughput::Elements(N));

    // Database representation: DashMap
    group.bench_function("vec dashmap lookup", |b| {
        b.iter_batched_ref(
            || {
                let map = DashMap::<Vec<u64>, u64>::default();
                for i in 0..100u64 {
                    for j in 0..10u64 {
                        map.insert(vec![i, j], 0u64);
                    }
                }
                map
            },
            |map| {
                let k = vec![0, 0];
                for _ in 0..N {
                    black_box(map.get(black_box(&k)));
                }
            },
            BatchSize::LargeInput,
        )
    });

    // Database representation: pre-built Map with OnceCells
    group.bench_function("vec nested hashbrown + oncecell lookup", |b| {
        b.iter_batched_ref(
            || {
                let mut map = Map::<u64, OnceCell<Map<Vec<u64>, u64>>>::default();
                for i in 0..100u64 {
                    let mut child = Map::default();
                    for j in 0..10u64 {
                        child.insert(vec![j], 0u64);
                    }
                    let cell = OnceCell::new();
                    cell.set(child);
                    map.insert(i, cell);
                }
                map
            },
            |map| {
                let k = vec![0, 0];
                for _ in 0..N {
                    black_box(
                        map.get(black_box(&k[0]))
                            .unwrap()
                            .get()
                            .unwrap()
                            .get(black_box(&k[1..])),
                    );
                }
            },
            BatchSize::LargeInput,
        )
    });

    // database representation: pre-built Map with Arcs (collect from OnceCells for each crate)
    group.bench_function("vec nested arc lookup", |b| {
        b.iter_batched_ref(
            || {
                let mut map = Map::<u64, Arc<Map<Vec<u64>, u64>>>::default();
                for i in 0..100u64 {
                    let mut child = Map::default();
                    for j in 0..10u64 {
                        child.insert(vec![j], 0u64);
                    }
                    map.insert(i, Arc::new(child));
                }
                map
            },
            |map| {
                let k = vec![0, 0];
                for _ in 0..N {
                    black_box(map.get(black_box(&k[0])).unwrap().get(black_box(&k[1..])));
                }
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, compare_hash_maps);
criterion_main!(benches);
