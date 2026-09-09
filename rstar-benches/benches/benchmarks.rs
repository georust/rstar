#[macro_use]
extern crate criterion;
extern crate rand;
extern crate rand_hc;
extern crate rstar;

use std::f64::consts::PI;

use rand::{RngExt, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::primitives::CachedEnvelope;
use rstar::{RStarInsertionStrategy, RTree, RTreeObject, RTreeParams, AABB};

use criterion::Criterion;
use ordered_float::NotNan;

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

fn bulk_load_comparison_not_nan(c: &mut Criterion) {
    c.bench_function("insert sequential (NotNan)", |b| {
        let points: Vec<_> = create_random_points_not_nan(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);
        b.iter(move || {
            let mut rtree = rstar::RTree::new();
            for point in &points {
                rtree.insert(*point);
            }
        });
    });
}

fn bulk_load_baseline_not_nan(c: &mut Criterion) {
    c.bench_function("bulk load baseline (NotNan)", move |b| {
        let points: Vec<_> = create_random_points_not_nan(DEFAULT_BENCHMARK_TREE_SIZE, SEED_1);

        b.iter(|| {
            RTree::<_, Params>::bulk_load_with_params(points.clone());
        });
    });
}

fn locate_successful_internal_not_nan(c: &mut Criterion) {
    let points: Vec<_> = create_random_points_not_nan(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point_int (successful, NotNan)", move |b| {
        b.iter(|| tree.locate_at_point_int(query_point).is_some())
    });
}

fn locate_successful_not_nan(c: &mut Criterion) {
    let points: Vec<_> = create_random_points_not_nan(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point (successful, NotNan)", move |b| {
        b.iter(|| tree.locate_at_point(query_point).is_some())
    });
}

fn locate_unsuccessful_not_nan(c: &mut Criterion) {
    let points: Vec<_> = create_random_points_not_nan(100_000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = not_nan([0.7, 0.7]);
    c.bench_function("locate_at_point (unsuccessful, NotNan)", move |b| {
        b.iter(|| tree.locate_at_point(query_point).is_none())
    });
}

fn locate_unsuccessful_internal_not_nan(c: &mut Criterion) {
    let points: Vec<_> = create_random_points_not_nan(100_000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = not_nan([0.7, 0.7]);
    c.bench_function("locate_at_point_int (unsuccessful, NotNan)", move |b| {
        b.iter(|| tree.locate_at_point(query_point).is_none())
    });
}

fn bulk_load_complex_geom(c: &mut Criterion) {
    c.bench_function("Bulk load complex geo-types geom", move |b| {
        let polys: Vec<_> =
            create_random_polygons(DEFAULT_BENCHMARK_TREE_SIZE, 4096, SEED_1).collect();

        b.iter(|| {
            RTree::<BenchPolygon, Params>::bulk_load_with_params(polys.clone());
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
                tree_bulk_loaded.nearest_neighbor(*query_point).unwrap();
            }
        })
    })
    .bench_function("sequential load quality", move |b| {
        b.iter(|| {
            for query_point in &query_points_cloned_1 {
                tree_sequential.nearest_neighbor(*query_point).unwrap();
            }
        });
    });
}

fn locate_successful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point (successful)", move |b| {
        b.iter(|| tree.locate_at_point(query_point).is_some())
    });
}

fn locate_unsuccessful(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = [0.7, 0.7];
    c.bench_function("locate_at_point (unsuccessful)", move |b| {
        b.iter(|| tree.locate_at_point(query_point).is_none())
    });
}

fn locate_successful_internal(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let query_point = points[500];
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    c.bench_function("locate_at_point_int (successful)", move |b| {
        b.iter(|| tree.locate_at_point_int(query_point).is_some())
    });
}

fn locate_unsuccessful_internal(c: &mut Criterion) {
    let points: Vec<_> = create_random_points(100_000, SEED_1);
    let tree = RTree::<_, Params>::bulk_load_with_params(points);
    let query_point = [0.7, 0.7];
    c.bench_function("locate_at_point_int (unsuccessful)", move |b| {
        b.iter(|| tree.locate_at_point(query_point).is_none())
    });
}

/// The query-heavy benchmarks, run once per scalar type.
///
/// A point lookup descends one root-to-leaf path and returns at the first hit; these visit many
/// nodes and compare every candidate they reach, which is where `NotNan` can pay off -- it skips
/// the `NaN` test that every `f64` comparison performs.
/// Both arms must do *identical* work for the `(NotNan)` numbers to mean anything, so they share
/// this body rather than being copy-pasted per type.
fn query_benchmarks<P>(
    c: &mut Criterion,
    scalar: &str,
    point: impl Fn([f64; 2]) -> P,
    radius_2: P::Scalar,
) where
    P: rstar::Point
        + rstar::RTreeObject<Envelope = AABB<P>>
        + rstar::PointDistance
        + Copy
        + 'static,
{
    let points: Vec<P> = create_random_points(100_000, SEED_1)
        .into_iter()
        .map(&point)
        .collect();
    let query_points: Vec<P> = create_random_points(100, SEED_2)
        .into_iter()
        .map(&point)
        .collect();
    let envelope = AABB::from_corners(point([0.4, 0.4]), point([0.6, 0.6]));
    let tree = RTree::<P, Params>::bulk_load_with_params(points);

    let nearest_queries = query_points.clone();
    c.bench_function(&format!("nearest_neighbor ({scalar})"), {
        let tree = &tree;
        move |b| {
            b.iter(|| {
                for query_point in &nearest_queries {
                    tree.nearest_neighbor(*query_point).unwrap();
                }
            })
        }
    });

    c.bench_function(&format!("nearest_neighbor_iter, 10 nearest ({scalar})"), {
        let tree = &tree;
        let query_point = query_points[0];
        move |b| b.iter(|| tree.nearest_neighbor_iter(query_point).take(10).count())
    });

    c.bench_function(&format!("locate_in_envelope_intersecting ({scalar})"), {
        let tree = &tree;
        move |b| b.iter(|| tree.locate_in_envelope_intersecting(envelope).count())
    });

    c.bench_function(&format!("locate_within_distance ({scalar})"), {
        let tree = &tree;
        let query_point = query_points[0];
        move |b| b.iter(|| tree.locate_within_distance(query_point, radius_2).count())
    });
}

fn query_benchmarks_f64(c: &mut Criterion) {
    query_benchmarks(c, "f64", |p| p, 0.01);
}

fn query_benchmarks_not_nan(c: &mut Criterion) {
    query_benchmarks(c, "NotNan", not_nan, NotNan::new(0.01).unwrap());
}

criterion_group!(
    benches,
    bulk_load_baseline,
    bulk_load_baseline_not_nan,
    bulk_load_comparison,
    bulk_load_comparison_not_nan,
    bulk_load_complex_geom,
    bulk_load_complex_geom_cached,
    tree_creation_quality,
    locate_successful,
    locate_unsuccessful,
    locate_successful_internal,
    locate_successful_internal_not_nan,
    locate_unsuccessful_internal,
    locate_successful_not_nan,
    locate_unsuccessful_not_nan,
    locate_unsuccessful_internal_not_nan,
    query_benchmarks_f64,
    query_benchmarks_not_nan,
);
criterion_main!(benches);

fn not_nan([x, y]: [f64; 2]) -> [NotNan<f64>; 2] {
    [NotNan::new(x).unwrap(), NotNan::new(y).unwrap()]
}

fn create_random_points_not_nan(num_points: usize, seed: &[u8; 32]) -> Vec<[NotNan<f64>; 2]> {
    create_random_points(num_points, seed)
        .into_iter()
        .map(not_nan)
        .collect()
}

fn create_random_points(num_points: usize, seed: &[u8; 32]) -> Vec<[f64; 2]> {
    let mut rng = Hc128Rng::from_seed(*seed);
    (0..num_points).map(|_| rng.random()).collect()
}

fn create_random_polygons(
    num_points: usize,
    size: usize,
    seed: &[u8; 32],
) -> impl Iterator<Item = BenchPolygon> {
    let mut rng = Hc128Rng::from_seed(*seed);
    let base_polygon = circular_polygon(size);
    (0..num_points).map(move |_| {
        let [scale_x, scale_y]: [f64; 2] = rng.random();
        let [shift_x, shift_y]: [f64; 2] = rng.random();

        let mut shifted_polygon = base_polygon.clone();
        for coord in &mut shifted_polygon.ring {
            coord[0] = (shift_x + coord[0]) * scale_x;
            coord[1] = (shift_y + coord[1]) * scale_y;
        }
        shifted_polygon
    })
}

fn circular_polygon(steps: usize) -> BenchPolygon {
    let delta = 2. * PI / steps as f64;
    let r = 1.0;

    let ring = (0..steps)
        .scan(0.0_f64, |angle, _step| {
            let (sin, cos) = angle.sin_cos();
            *angle += delta;
            Some([r * cos, r * sin])
        })
        .collect();

    BenchPolygon { ring }
}

/// A minimal stand-in for a `geo_types::Polygon` used only to exercise bulk loading
/// of a non-trivial [`RTreeObject`] whose envelope spans many vertices. Its envelope
/// is the axis-aligned bounding box of the exterior ring; no point queries are run
/// against it, so it deliberately implements only [`RTreeObject`].
#[derive(Clone)]
struct BenchPolygon {
    ring: Vec<[f64; 2]>,
}

impl RTreeObject for BenchPolygon {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> AABB<[f64; 2]> {
        AABB::from_points(self.ring.iter())
    }
}
