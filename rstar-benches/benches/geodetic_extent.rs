//! Benchmarks for the geodetic extent leaves: `GeodeticLineString` and
//! `GeodeticPolygon`.
//!
//! Unlike the point benchmark (`geodetic.rs`), extent geometry has no equivalent on
//! the 2D-MBR branch, so there is no cross-branch baseline here. A planar
//! `euclidean/*_bulk_load` anchor (a plain axis-aligned box over the same vertices,
//! the comparison the standard `benchmarks.rs` polygon bench makes) is kept for the
//! index-build figures; the query operations (nearest-neighbour, radius, membership)
//! have no standard planar counterpart and are reported geo-only.
//!
//! Cost-model note: both leaves precompute and cache their envelope at construction --
//! the linestring's arc-aware box in `try_from_lonlat` (O(edges)), the polygon's in
//! `try_new` (O(edges^2)) -- so the `*_bulk_load` benches read a cached box, and the
//! distinctive build cost of each is measured separately in `geo/ls_construct` and
//! `geo/poly_construct`.

#[macro_use]
extern crate criterion;

use std::f64::consts::TAU;
use std::hint::black_box;

use criterion::Criterion;
use rand::{RngExt, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::geodetic::{
    GeodeticCoord, GeodeticLineString, GeodeticPolygon, GeodeticRTree, GeodeticRing, UnitVec,
};
use rstar::{RTree, RTreeObject, AABB};

// Seeds shared with the point benchmark so the centres line up.
const SEED_SHAPES: &[u8; 32] = b"Gv0aHMtHkBGsUXNspGU9fLRuCWkZWHZx";
const SEED_QUERIES: &[u8; 32] = b"km7DO4GeaFZfTcDXVpnO7ZJlgUY7hZiS";

const BULK_LOAD_SIZE: usize = 2_000;
const QUERY_TREE_SIZE: usize = 20_000;
const QUERY_COUNT: usize = 100;

/// Vertices per generated linestring / polygon ring.
const LS_VERTICES: usize = 16;
const POLY_VERTICES: usize = 16;
/// A denser ring for the membership microbenchmark, where the ray-cast cost scales
/// with the edge count.
const MEMBERSHIP_VERTICES: usize = 64;

/// Angular size of a generated shape, in degrees.
const SHAPE_RADIUS_DEG: f64 = 0.5;

/// Radius for the geodetic radius query, in metres (500 km).
const RADIUS_METRES: f64 = 500_000.0;

// --- shared deterministic data ---

/// Mid-latitude centres held a degree clear of the lon/lat bounds (`|lon| <= 179`,
/// `|lat| <= 60`) so that adding the 0.5-degree shape radius keeps every generated
/// vertex in range, and every edge spans far less than 180 degrees.
fn random_centres(n: usize, seed: &[u8; 32]) -> Vec<GeodeticCoord> {
    let mut rng = Hc128Rng::from_seed(*seed);
    (0..n)
        .map(|_| GeodeticCoord {
            lon: rng.random_range(-179.0..179.0),
            lat: rng.random_range(-60.0..60.0),
        })
        .collect()
}

/// `k` `(lon, lat)` vertices evenly spaced on a small circle about `centre`. Open (no
/// closing vertex): used directly as a polyline, or closed by [`closed`] for a ring.
fn ring_lonlat(centre: GeodeticCoord, k: usize) -> Vec<(f64, f64)> {
    (0..k)
        .map(|i| {
            let a = TAU * i as f64 / k as f64;
            (
                centre.lon + SHAPE_RADIUS_DEG * a.cos(),
                centre.lat + SHAPE_RADIUS_DEG * a.sin(),
            )
        })
        .collect()
}

/// Repeats the first vertex at the end, closing the ring (counter-clockwise as given).
fn closed(mut lonlat: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if let Some(&first) = lonlat.first() {
        lonlat.push(first);
    }
    lonlat
}

fn linestring(centre: GeodeticCoord, k: usize) -> GeodeticLineString {
    GeodeticLineString::try_from_lonlat(ring_lonlat(centre, k)).expect("valid linestring")
}

fn polygon(centre: GeodeticCoord, k: usize) -> GeodeticPolygon {
    let exterior =
        GeodeticRing::try_from_lonlat(closed(ring_lonlat(centre, k))).expect("valid ring");
    GeodeticPolygon::try_new(exterior, Vec::new()).expect("valid polygon")
}

/// A planar stand-in indexed by the axis-aligned box of its vertices: the same kind of
/// extent leaf the standard polygon benchmark uses, recomputing the box on each
/// `envelope()` call (so it is comparable to the linestring's arc box).
#[derive(Clone)]
struct BenchExtent {
    ring: Vec<[f64; 2]>,
}

impl RTreeObject for BenchExtent {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> AABB<[f64; 2]> {
        AABB::from_points(self.ring.iter())
    }
}

fn bench_extent(centre: GeodeticCoord, k: usize) -> BenchExtent {
    BenchExtent {
        ring: ring_lonlat(centre, k)
            .into_iter()
            .map(|(lon, lat)| [lon, lat])
            .collect(),
    }
}

// --- linestrings ---

fn linestrings(c: &mut Criterion) {
    let centres = random_centres(BULK_LOAD_SIZE, SEED_SHAPES);

    // Construction from coordinates: the per-edge arc-box build, done once per
    // linestring in `try_from_lonlat` and then cached.
    let lonlats: Vec<Vec<(f64, f64)>> = centres
        .iter()
        .map(|&ce| ring_lonlat(ce, LS_VERTICES))
        .collect();
    c.bench_function("geo/ls_construct 2000", |b| {
        b.iter(|| {
            lonlats
                .iter()
                .map(|r| GeodeticLineString::try_from_lonlat(r.iter().copied()).unwrap())
                .collect::<Vec<_>>()
        });
    });

    let planar: Vec<BenchExtent> = centres
        .iter()
        .map(|&ce| bench_extent(ce, LS_VERTICES))
        .collect();
    c.bench_function("euclidean/ls_bulk_load 2000", |b| {
        b.iter(|| RTree::bulk_load(planar.clone()));
    });

    let lines: Vec<GeodeticLineString> = centres
        .iter()
        .map(|&ce| linestring(ce, LS_VERTICES))
        .collect();
    c.bench_function("geo/ls_bulk_load 2000", |b| {
        b.iter(|| GeodeticRTree::bulk_load(lines.clone()));
    });

    let tree = GeodeticRTree::bulk_load(
        random_centres(QUERY_TREE_SIZE, SEED_SHAPES)
            .into_iter()
            .map(|ce| linestring(ce, LS_VERTICES))
            .collect(),
    );
    let queries = random_centres(QUERY_COUNT, SEED_QUERIES);

    c.bench_function("geo/ls_nearest_neighbor 20k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor(*q));
            }
        });
    });

    c.bench_function("geo/ls_locate_within_distance 20k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.locate_within_distance(*q, RADIUS_METRES).count());
            }
        });
    });
}

// --- polygons ---

fn polygons(c: &mut Criterion) {
    let centres = random_centres(BULK_LOAD_SIZE, SEED_SHAPES);

    // Construction from coordinates: the O(edges^2) envelope build, done once per
    // polygon in `try_new` and then cached.
    let rings: Vec<Vec<(f64, f64)>> = centres
        .iter()
        .map(|&ce| closed(ring_lonlat(ce, POLY_VERTICES)))
        .collect();
    c.bench_function("geo/poly_construct 2000", |b| {
        b.iter(|| {
            rings
                .iter()
                .map(|r| {
                    let ext = GeodeticRing::try_from_lonlat(r.iter().copied()).unwrap();
                    GeodeticPolygon::try_new(ext, Vec::new()).unwrap()
                })
                .collect::<Vec<_>>()
        });
    });

    let planar: Vec<BenchExtent> = centres
        .iter()
        .map(|&ce| bench_extent(ce, POLY_VERTICES))
        .collect();
    c.bench_function("euclidean/poly_bulk_load 2000", |b| {
        b.iter(|| RTree::bulk_load(planar.clone()));
    });

    let polys: Vec<GeodeticPolygon> = centres
        .iter()
        .map(|&ce| polygon(ce, POLY_VERTICES))
        .collect();
    c.bench_function("geo/poly_bulk_load 2000", |b| {
        b.iter(|| GeodeticRTree::bulk_load(polys.clone()));
    });

    let tree = GeodeticRTree::bulk_load(
        random_centres(QUERY_TREE_SIZE, SEED_SHAPES)
            .into_iter()
            .map(|ce| polygon(ce, POLY_VERTICES))
            .collect(),
    );
    let queries = random_centres(QUERY_COUNT, SEED_QUERIES);

    c.bench_function("geo/poly_nearest_neighbor 20k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor(*q));
            }
        });
    });

    c.bench_function("geo/poly_locate_within_distance 20k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.locate_within_distance(*q, RADIUS_METRES).count());
            }
        });
    });

    // Point-in-polygon membership: the ray-cast cost scales with the edge count, so
    // measure it against a denser ring than the indexed polygons.
    let membership = polygon(GeodeticCoord { lon: 0.0, lat: 0.0 }, MEMBERSHIP_VERTICES);
    let probes: Vec<UnitVec> = queries.iter().map(|&q| UnitVec::from(q)).collect();
    c.bench_function("geo/poly_contains_point 64-gon", |b| {
        b.iter(|| {
            for p in &probes {
                black_box(membership.contains_point(*p));
            }
        });
    });
}

criterion_group!(benches, linestrings, polygons);
criterion_main!(benches);
