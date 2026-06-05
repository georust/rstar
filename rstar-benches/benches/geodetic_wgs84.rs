//! Benchmarks for the WGS84 ellipsoidal geodesic refine (the rstar `geodetic-wgs84`
//! feature).
//!
//! Gated behind this bench crate's `wgs84` feature (which turns on
//! `rstar/geodetic-wgs84`) so the default `cargo bench` stays free of
//! `geographiclib-rs`. Run it with:
//!
//! ```text
//! cargo bench -p rstar-benches --features wgs84 --bench geodetic_wgs84
//! ```
//!
//! Each pair runs the spherical query method and its WGS84 counterpart over the same
//! tree and queries: the end-to-end cost of choosing the ellipsoidal method, as a
//! caller would see it. The two operations behave very differently. The radius query
//! refines every candidate in a widened spherical shell on the ellipsoid (Karney's
//! method, via `geographiclib-rs`), so it is markedly slower. The nearest-neighbour
//! refine instead rides the nearest-neighbour *iterator* with a geodesic lower-bound
//! early break, so it measures only a handful of candidates and stays close to, or
//! even below, the plain spherical `nearest_neighbor`. The two NN methods therefore
//! take different internal traversals: their delta is the methods as called, not a
//! pure geodesic surcharge.

#[macro_use]
extern crate criterion;

use std::hint::black_box;

use criterion::Criterion;
use rand::{RngExt, SeedableRng};
use rand_hc::Hc128Rng;

use rstar::geodetic::{GeodeticCoord, GeodeticPoint, GeodeticRTree};

// Seeds shared with the point benchmark so the data lines up.
const SEED_POINTS: &[u8; 32] = b"Gv0aHMtHkBGsUXNspGU9fLRuCWkZWHZx";
const SEED_QUERIES: &[u8; 32] = b"km7DO4GeaFZfTcDXVpnO7ZJlgUY7hZiS";

const QUERY_TREE_SIZE: usize = 100_000;
const QUERY_COUNT: usize = 100;

/// Radius for the geodetic radius query, in metres (500 km).
const RADIUS_METRES: f64 = 500_000.0;

fn random_coords(n: usize, seed: &[u8; 32]) -> Vec<GeodeticCoord> {
    let mut rng = Hc128Rng::from_seed(*seed);
    (0..n)
        .map(|_| GeodeticCoord {
            lon: rng.random_range(-180.0..180.0),
            lat: rng.random_range(-90.0..90.0),
        })
        .collect()
}

fn wgs84(c: &mut Criterion) {
    let tree = GeodeticRTree::bulk_load(
        random_coords(QUERY_TREE_SIZE, SEED_POINTS)
            .into_iter()
            .map(|co| GeodeticPoint::new(co.lon, co.lat))
            .collect(),
    );
    let queries = random_coords(QUERY_COUNT, SEED_QUERIES);

    c.bench_function("geo/nearest_neighbor_spherical 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor(*q));
            }
        });
    });

    c.bench_function("geo/nearest_neighbor_wgs84 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.nearest_neighbor_wgs84(*q));
            }
        });
    });

    c.bench_function("geo/locate_within_distance_spherical 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.locate_within_distance(*q, RADIUS_METRES).count());
            }
        });
    });

    c.bench_function("geo/locate_within_distance_wgs84 100k", |b| {
        b.iter(|| {
            for q in &queries {
                black_box(tree.locate_within_distance_wgs84(*q, RADIUS_METRES).count());
            }
        });
    });
}

criterion_group!(benches, wgs84);
criterion_main!(benches);
