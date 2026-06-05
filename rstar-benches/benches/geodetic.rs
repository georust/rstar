//! Benchmarks comparing a geodetic R-tree against a naive Euclidean
//! `RTree<[f64; 2]>` over the same longitude/latitude data.
//!
//! Two geodetic designs live on divergent branches and cannot be compiled
//! together, so this benchmark exists in a matching form on each branch: the
//! random data, dataset sizes, query points, and criterion benchmark IDs are
//! identical, and only the geodetic tree type differs. Compare them with
//! criterion baselines:
//!
//! ```text
//! # on the 2D-MBR branch (RTree<GeodeticPoint>)
//! cargo bench --bench geodetic -- --save-baseline geo_lo
//! # on the 3D-embedding branch (GeodeticRTree)
//! cargo bench --bench geodetic -- --baseline geo_lo
//! ```
//!
//! The `euclidean/*` group is identical code on both branches and acts as a
//! stability anchor (its cross-branch delta should be near zero); the `geo/*`
//! group exercises each branch's real geodetic tree.

#[macro_use]
extern crate criterion;

use std::hint::black_box;

use criterion::Criterion;
use rand::{RngExt, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::geodetic::{GeodeticCoord, GeodeticPoint};
use rstar::RTree;

// --- branch-specific: the geodetic tree under test ---
use rstar::geodetic::GeodeticRTree;

// Seeds shared with the other branch so the generated data matches exactly.
const SEED_POINTS: &[u8; 32] = b"Gv0aHMtHkBGsUXNspGU9fLRuCWkZWHZx";
const SEED_QUERIES: &[u8; 32] = b"km7DO4GeaFZfTcDXVpnO7ZJlgUY7hZiS";

const BULK_LOAD_SIZE: usize = 2_000;
const QUERY_TREE_SIZE: usize = 100_000;
const QUERY_COUNT: usize = 100;

/// Radius for the geodetic radius query, in metres (500 km).
const RADIUS_METRES: f64 = 500_000.0;
/// Radius for the Euclidean radius query, in degrees (squared at the call site).
/// Unrelated to `RADIUS_METRES` — a different metric — but identical on both branches.
const EUCLID_RADIUS_DEG: f64 = 5.0;

// --- shared deterministic data (identical on both branches) ---

fn random_coords(n: usize, seed: &[u8; 32]) -> Vec<GeodeticCoord> {
    let mut rng = Hc128Rng::from_seed(*seed);
    (0..n)
        .map(|_| GeodeticCoord {
            lon: rng.random_range(-180.0..180.0),
            lat: rng.random_range(-90.0..90.0),
        })
        .collect()
}

fn geodetic_points(coords: &[GeodeticCoord]) -> Vec<GeodeticPoint> {
    coords
        .iter()
        .map(|c| GeodeticPoint::new(c.lon, c.lat))
        .collect()
}

fn euclidean_points(coords: &[GeodeticCoord]) -> Vec<[f64; 2]> {
    coords.iter().map(|c| [c.lon, c.lat]).collect()
}

// --- naive Euclidean baseline (identical code on both branches) ---

fn euclidean(c: &mut Criterion) {
    let load = euclidean_points(&random_coords(BULK_LOAD_SIZE, SEED_POINTS));
    c.bench_function("euclidean/bulk_load 2000", |b| {
        b.iter(|| RTree::bulk_load(load.clone()));
    });

    let tree = RTree::bulk_load(euclidean_points(&random_coords(
        QUERY_TREE_SIZE,
        SEED_POINTS,
    )));
    let queries = euclidean_points(&random_coords(QUERY_COUNT, SEED_QUERIES));

    c.bench_function("euclidean/nearest_neighbor 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor(*q));
            }
        });
    });

    let threshold = EUCLID_RADIUS_DEG * EUCLID_RADIUS_DEG;
    c.bench_function("euclidean/locate_within_distance 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.locate_within_distance(*q, threshold).count());
            }
        });
    });
}

// --- geodetic tree under test (branch-specific bodies, shared IDs) ---

fn geodetic(c: &mut Criterion) {
    let load = geodetic_points(&random_coords(BULK_LOAD_SIZE, SEED_POINTS));
    c.bench_function("geo/bulk_load 2000", |b| {
        b.iter(|| GeodeticRTree::bulk_load(load.clone()));
    });

    c.bench_function("geo/insert_sequential 2000", |b| {
        b.iter(|| {
            let mut tree = GeodeticRTree::new();
            for p in &load {
                tree.insert(*p);
            }
            tree
        });
    });

    let tree = GeodeticRTree::bulk_load(geodetic_points(&random_coords(
        QUERY_TREE_SIZE,
        SEED_POINTS,
    )));
    let queries = random_coords(QUERY_COUNT, SEED_QUERIES);

    c.bench_function("geo/nearest_neighbor 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor(*q));
            }
        });
    });

    c.bench_function("geo/nearest_neighbor_iter 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor_iter(*q).next());
            }
        });
    });

    c.bench_function("geo/locate_within_distance 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.locate_within_distance(*q, RADIUS_METRES).count());
            }
        });
    });
}

criterion_group!(benches, euclidean, geodetic);
criterion_main!(benches);
