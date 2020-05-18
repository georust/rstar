#[macro_use]
extern crate criterion;

extern crate rand;
extern crate rand_hc;
extern crate rstar;
extern crate spade;

use rand::{Rng, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::{RStarInsertionStrategy, RTree, RTreeParams};

use criterion::{Bencher, Criterion, Fun};

const SEED_1: &[u8; 32] = b"Gv0aHMtHkBGsUXNspGU9fLRuCWkZWHZx";
const SEED_2: &[u8; 32] = b"km7DO4GeaFZfTcDXVpnO7ZJlgUY7hZiS";

struct Params;

impl RTreeParams for Params {
    const MIN_SIZE: usize = 2;
    const MAX_SIZE: usize = 40;
    const REINSERTION_COUNT: usize = 1;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}

const DEFAULT_BENCHMARK_TREE_SIZE: usize = 2000;

fn bulk_load_baseline(c: &mut Criterion) {
    c.bench_function("Bulk load baseline", move |b| {
        let points: Vec<_> = create_random_points(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);

        b.iter(|| {
            RTree::<_, Params>::bulk_load_with_params(points.clone());
        });
    });
}

fn bulk_load_comparison(c: &mut Criterion) {
    let rstar_bench = Fun::new("rstar", |b: &mut Bencher, _| {
        let points: Vec<_> = create_random_points(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);
        b.iter(|| RTree::<_, Params>::bulk_load_with_params(points.clone()));
    });
    let spade_bench = Fun::new("spade", |b: &mut Bencher, _| {
        let points: Vec<_> = create_random_points(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);
        b.iter(move || {
            spade::rtree::RTree::bulk_load(points.clone());
        });
    });
    let rstar_sequential = Fun::new("rstar sequential", |b: &mut Bencher, _| {
        let points: Vec<_> = create_random_points(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);
        b.iter(move || {
            let mut rtree = rstar::RTree::new();
            for point in &points {
                rtree.insert(*point);
            }
        });
    });

    c.bench_functions(
        "bulk load comparison",
        vec![rstar_bench, spade_bench, rstar_sequential],
        (),
    );
}

fn tree_creation_quality(c: &mut Criterion) {
    const SIZE: usize = 100_000;
    let points: Vec<_> = create_random_points(SIZE, SEED_1);
    let tree_bulk_loaded = RTree::<_, Params>::bulk_load_with_params(points.clone());
    let mut tree_sequential = RTree::new();
    for point in &points {
        tree_sequential.insert(*point);
    }

    let query_points = create_random_points(100, SEED_2);
    let query_points_cloned_1 = query_points.clone();
    c.bench_function("bulk load quality", move |b| {
        b.iter(|| {
            for query_point in &query_points {
                tree_bulk_loaded.nearest_neighbor(&query_point).unwrap();
            }
        })
    })
    .bench_function("sequential load quality", move |b| {
        b.iter(|| {
            for query_point in &query_points_cloned_1 {
                tree_sequential.nearest_neighbor(&query_point).unwrap();
            }
        });
    });
}

fn locate_successful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point (successful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point).unwrap())
    });
}

fn locate_unsuccessful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = [0.7, 0.7];
    c.bench_function("locate_at_point (unsuccessful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point).is_none())
    });
}

fn locate_successful_internal(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point_int (successful)", move |b| {
        b.iter(|| tree.locate_at_point_int(&query_point).unwrap())
    });
}

fn locate_unsuccessful_internal(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = [0.7, 0.7];
    c.bench_function("locate_at_point_int (unsuccessful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point).is_none())
    });
}

criterion_group!(
    benches,
    bulk_load_baseline,
    bulk_load_comparison,
    tree_creation_quality,
    locate_successful,
    locate_unsuccessful,
    locate_successful_internal,
    locate_unsuccessful_internal
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
