#[macro_use]
extern crate criterion;

extern crate rand;
extern crate rstar;
extern crate spade;

use rand::{Rng, SeedableRng, XorShiftRng};
use rstar::RTree;

use criterion::{Bencher, Criterion, ParameterizedBenchmark};

fn bulk_load_baseline(c: &mut Criterion) {
    let sizes: Vec<_> = (1..=3).map(|i| i * i * 1000).collect();
    let mut points: Vec<_> = create_random_points(*sizes.last().unwrap(), *b"|nsu)r3cTI0ni5ts");

    c.bench_function_over_inputs(
        "Bulk load baseline",
        move |b, size| {
            let inner_points = &mut points[..*size];
            b.iter(|| {
                RTree::bulk_load(inner_points);
            });
        },
        sizes,
    );
}

fn bulk_load_comparison(c: &mut Criterion) {
    let sizes: Vec<_> = (1..=20).map(|i| i * 40000).collect();
    let rstar_bench = ParameterizedBenchmark::new(
        "rstar",
        |b: &mut Bencher, size: &usize| {
            let points: Vec<_> = create_random_points(*size, *b"|nsu)r3cTI0ni5ts");

            b.iter(|| {
                RTree::bulk_load(&mut points.clone());
            });
        },
        sizes,
    ).with_function("spade", |b: &mut Bencher, size: &usize| {
        let points: Vec<_> = create_random_points(*size, *b"|nsu)r3cTI0ni5ts");

        b.iter(move || {
            spade::rtree::RTree::bulk_load(points.clone());
        });
    }).with_function("rstar sequential", |b: &mut Bencher, size: &usize| {
        let points: Vec<_> = create_random_points(*size, *b"|nsu)r3cTI0ni5ts");
        b.iter(move || {
            let mut rtree = rstar::RTree::new();
            for point in &points {
                rtree.insert(*point);
            }
        });
    }).with_function("spade sequential", |b: &mut Bencher, size: &usize| {
        let points: Vec<_> = create_random_points(*size, *b"|nsu)r3cTI0ni5ts");
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
    let mut points: Vec<_> = create_random_points(10000, *b"|nsu)r3cTI0ni5ts");
    let tree = RTree::bulk_load(&mut points);
    let query_point = [0.4, -0.4];
    c.bench_function("nearest_neighbor", move |b| {
        b.iter(|| tree.nearest_neighbor(&query_point).is_some())
    });
}

fn locate_successful(c: &mut Criterion) {
    let mut points: Vec<_> = create_random_points(10000, *b"|nsu)r3cTI0ni5ts");
    let tree = RTree::bulk_load(&mut points);
    let query_point = points[500];
    assert!(tree.locate_at_point(&query_point).is_some());
    c.bench_function("locate_at_point (successful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point))
    });
}

fn locate_unsuccessful(c: &mut Criterion) {
    let mut points: Vec<_> = create_random_points(10000, *b"|nsu)r3cTI0ni5ts");
    let tree = RTree::bulk_load(&mut points);
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
    nearest_neighbor,
    locate_successful,
    locate_unsuccessful
);
criterion_main!(benches);

fn create_random_points(num_points: usize, seed: [u8; 16]) -> Vec<[f64; 2]> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = XorShiftRng::from_seed(seed);
    for _ in 0..num_points {
        result.push(rng.gen());
    }
    result
}
