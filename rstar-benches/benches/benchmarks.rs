#[macro_use]
extern crate criterion;
extern crate geo;
extern crate geo_types;
extern crate rand;
extern crate rand_hc;
extern crate rstar;

use std::f64::consts::PI;

use geo::{Coord, LineString, MapCoords, Polygon};
use rand::{Rng, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::primitives::CachedEnvelope;
use rstar::{RStarInsertionStrategy, RTree, RTreeParams};

use criterion::Criterion;

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
    c.bench_function("bulk load baseline", move |b| {
        let points: Vec<_> = create_random_points(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);

        b.iter(|| {
            RTree::<_, Params>::bulk_load_with_params(points.clone());
        });
    });
}

fn bulk_load_comparison(c: &mut Criterion) {
    c.bench_function("insert sequential", |b| {
        let points: Vec<_> = create_random_points(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);
        b.iter(move || {
            let mut rtree = rstar::RTree::new();
            for point in &points {
                rtree.insert(*point);
            }
        });
    });
}

fn bulk_load_complex_geom(c: &mut Criterion) {
    c.bench_function("Bulk load complex geo-types geom", move |b| {
        let polys: Vec<_> =
            create_random_polygons(DEFAULT_BENCHMARK_TREE_SIZE, 4096, SEED_1).collect();

        b.iter(|| {
            RTree::<Polygon<f64>, Params>::bulk_load_with_params(polys.clone());
        });
    });
}

fn bulk_load_complex_geom_cached(c: &mut Criterion) {
    c.bench_function(
        "Bulk load complex geo-types geom with cached envelope",
        move |b| {
            let cached: Vec<_> = create_random_polygons(DEFAULT_BENCHMARK_TREE_SIZE, 4096, SEED_1)
                .map(CachedEnvelope::new)
                .collect();
            b.iter(|| {
                RTree::<CachedEnvelope<_>, Params>::bulk_load_with_params(cached.clone());
            });
        },
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
                tree_bulk_loaded.nearest_neighbor(query_point).unwrap();
            }
        })
    })
    .bench_function("sequential load quality", move |b| {
        b.iter(|| {
            for query_point in &query_points_cloned_1 {
                tree_sequential.nearest_neighbor(query_point).unwrap();
            }
        });
    });
}

fn locate_successful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point (successful)", move |b| {
        b.iter(|| tree.locate_at_point(&query_point).is_some())
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
        b.iter(|| tree.locate_at_point_int(&query_point).is_some())
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
    bulk_load_complex_geom,
    bulk_load_complex_geom_cached,
    tree_creation_quality,
    locate_successful,
    locate_unsuccessful,
    locate_successful_internal,
    locate_unsuccessful_internal,
);
criterion_main!(benches);

fn create_random_points(num_points: usize, seed: &[u8; 32]) -> Vec<[f64; 2]> {
    let mut rng = Hc128Rng::from_seed(*seed);
    (0..num_points).map(|_| rng.gen()).collect()
}

fn create_random_polygons(
    num_points: usize,
    size: usize,
    seed: &[u8; 32],
) -> impl Iterator<Item = Polygon<f64>> {
    let mut rng = Hc128Rng::from_seed(*seed);
    let base_polygon = circular_polygon(size);

    (0..num_points).map(move |_| {
        let [scale_x, scale_y]: [f64; 2] = rng.gen();
        let [shift_x, shift_y]: [f64; 2] = rng.gen();
        base_polygon.clone().map_coords(|c| Coord {
            x: (shift_x + c.x) * scale_x,
            y: (shift_y + c.y) * scale_y,
        })
    })
}

fn circular_polygon(steps: usize) -> Polygon<f64> {
    let delta = 2. * PI / steps as f64;
    let r = 1.0;

    let ring = (0..steps)
        .scan(0.0_f64, |angle, _step| {
            let (sin, cos) = angle.sin_cos();
            *angle += delta;
            Some((r * cos, r * sin).into())
        })
        .collect();

    Polygon::new(LineString(ring), Vec::new())
}
