#[macro_use]
extern crate criterion;

extern crate rand;
extern crate rand_hc;
extern crate rstar;
extern crate spade;

use rand::{Rng, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::{RStarInsertionStrategy, RTree, RTreeParams};

use criterion::{Bencher, Criterion, ParameterizedBenchmark};

const SEED_1: &[u8; 32] = b"Gv0aHMtHkBGsUXNspGU9fLRuCWkZWHZx";
const SEED_2: &[u8; 32] = b"km7DO4GeaFZfTcDXVpnO7ZJlgUY7hZiS";

struct Params;

impl RTreeParams for Params {
    const MIN_SIZE: usize = 2;
    const MAX_SIZE: usize = 40;
    const REINSERTION_COUNT: usize = 1;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}

fn bulk_load_baseline(c: &mut Criterion) {
    let sizes: Vec<_> = (1..=3).map(|i| i * i * 1000).collect();

    c.bench_function_over_inputs(
        "Bulk load baseline",
        move |b, size| {
            let points: Vec<_> = create_random_points(*size, SEED_1);

            b.iter(|| {
                RTree::<_, Params>::bulk_load_with_params(points.clone());
            });
        },
        sizes,
    );
}

fn bulk_load_comparison(c: &mut Criterion) {
    let sizes: Vec<_> = (1..=3).map(|i| i * 1000).collect();
    let rstar_bench = ParameterizedBenchmark::new(
        "rstar",
        |b: &mut Bencher, size: &usize| {
            let points: Vec<_> = create_random_points(*size, SEED_1);
            b.iter(|| RTree::<_, Params>::bulk_load_with_params(points.clone()));
        },
        sizes,
    )
    .with_function("spade", |b: &mut Bencher, size: &usize| {
        let points: Vec<_> = create_random_points(*size, SEED_1);

        b.iter(move || {
            spade::rtree::RTree::bulk_load(points.clone());
        });
    })
    .with_function("rstar sequential", |b: &mut Bencher, size: &usize| {
        let points: Vec<_> = create_random_points(*size, SEED_1);
        b.iter(move || {
            let mut rtree = rstar::RTree::new();
            for point in &points {
                rtree.insert(*point);
            }
        });
    })
    .with_function("spade sequential", |b: &mut Bencher, size: &usize| {
        let points: Vec<_> = create_random_points(*size, SEED_1);
        b.iter(move || {
            let mut rtree = spade::rtree::RTree::new();
            for point in &points {
                rtree.insert(*point);
            }
        });
    });

    c.bench("bulk load comparison", rstar_bench);
}

fn nearest_neighbor(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(10000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = [0.4, -0.4];
    c.bench_function("nearest_neighbor", move |b| {
        b.iter(|| tree.nearest_neighbor(&query_point).is_some())
    });
}

fn bulk_load_query_quality(c: &mut Criterion) {
    const SIZE: usize = 100_000;
    let points: Vec<_> = create_random_points(SIZE, SEED_1);
    let tree_bulk_loaded = RTree::<_, Params>::bulk_load_with_params(points.clone());
    let mut tree_sequential = RTree::new();
    for point in &points {
        tree_sequential.insert(*point);
    }

    let query_points = create_random_points(25, SEED_2);
    let query_points_cloned_1 = query_points.clone();
    c.bench_function("bulk load queries", move |b| {
        b.iter(|| {
            for query_point in &query_points {
                tree_bulk_loaded.nearest_neighbor(&query_point).is_some();
            }
        })
    })
    .bench_function("bulk load queries (sequential)", move |b| {
        b.iter(|| {
            for query_point in &query_points_cloned_1 {
                tree_sequential.nearest_neighbor(&query_point).is_some();
            }
        });
    });
}

fn locate_successful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(10000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    assert!(tree.locate_at_point(&query_point).is_some());
    c.bench_function("locate_at_point (successful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point))
    });
}

fn locate_unsuccessful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(10000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = [0.0, 0.0];
    assert!(tree.locate_at_point(&query_point).is_none());
    c.bench_function("locate_at_point (unsuccessful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point))
    });
}

criterion_group!(
    benches,
    bulk_load_baseline,
    bulk_load_comparison,
    bulk_load_query_quality,
    nearest_neighbor,
    locate_successful,
    locate_unsuccessful
);
criterion_main!(benches);

fn create_random_points(num_points: usize, seed: &[u8; 32]) -> Vec<[f64; 2]> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = Hc128Rng::from_seed(*seed);
    for _ in 0..num_points {
        result.push(rng.gen());
    }
    result
}
