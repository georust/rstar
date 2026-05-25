#![cfg(feature = "geodetic")]

//! End-to-end integration tests for `RTree<GeodeticPoint>`.
//!
//! These tests exercise nearest-neighbour correctness, sorted iterator
//! ordering, the branch-and-bound lower-bound contract, and the known
//! antimeridian limitation (documented in §10.6 of the implementation plan).

use approx::assert_relative_eq;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;

use rstar::geodetic::distance::haversine_distance;
use rstar::geodetic::{GeodeticCoord, GeodeticPoint};
use rstar::{Envelope, ParentNode, PointDistance, RTree, RTreeNode};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn random_points(rng: &mut StdRng, n: usize) -> Vec<GeodeticPoint> {
    (0..n)
        .map(|_| {
            let lon: f64 = rng.random_range(-170.0_f64..170.0_f64);
            let lat: f64 = rng.random_range(-85.0_f64..85.0_f64);
            GeodeticPoint::new(lon, lat)
        })
        .collect()
}

fn random_queries(rng: &mut StdRng, n: usize) -> Vec<GeodeticCoord> {
    (0..n)
        .map(|_| {
            let lon: f64 = rng.random_range(-170.0_f64..170.0_f64);
            let lat: f64 = rng.random_range(-85.0_f64..85.0_f64);
            GeodeticCoord { lon, lat }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Test 1: nearest_neighbor correctness against brute force
// ---------------------------------------------------------------------------

#[test]
fn nearest_neighbor_matches_brute_force() {
    let mut rng = StdRng::seed_from_u64(0x1234_5678_ABCD_EF01);
    let points = random_points(&mut rng, 1000);
    let tree = RTree::bulk_load(points.clone());

    for query in random_queries(&mut rng, 100) {
        let nn = tree.nearest_neighbor(query).unwrap();
        let tree_dist = haversine_distance(nn.0, query);

        let best = points
            .iter()
            .map(|p| haversine_distance(p.0, query))
            .fold(f64::MAX, f64::min);

        assert_relative_eq!(tree_dist, best, epsilon = 1e-6);
    }
}

// ---------------------------------------------------------------------------
// Test 2: nearest_neighbor_iter_with_distance_2 yields non-decreasing distances
// ---------------------------------------------------------------------------

#[test]
fn nearest_neighbor_iter_returns_sorted_distances() {
    let mut rng = StdRng::seed_from_u64(0xFEED_FACE_DEAD_BEEF);
    let points = random_points(&mut rng, 1000);
    let tree = RTree::bulk_load(points.clone());
    let n = points.len();

    for query in random_queries(&mut rng, 10) {
        let distances: Vec<f64> = tree
            .nearest_neighbor_iter_with_distance_2(query)
            .map(|(_, d)| d)
            .collect();

        assert_eq!(
            distances.len(),
            n,
            "iterator should yield exactly {n} items for query ({}, {})",
            query.lon,
            query.lat
        );

        for window in distances.windows(2) {
            let (a, b) = (window[0], window[1]);
            assert!(
                a <= b + 1e-9,
                "distances not non-decreasing: {a} > {b} for query ({}, {})",
                query.lon,
                query.lat
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 3: MBR envelope distance is a lower bound on the nearest leaf below it
// ---------------------------------------------------------------------------

fn min_leaf_distance(node: &ParentNode<GeodeticPoint>, query: GeodeticCoord) -> f64 {
    let min_leaf = node
        .children()
        .iter()
        .map(|child| match child {
            RTreeNode::Leaf(p) => p.distance_2(&query),
            RTreeNode::Parent(parent) => min_leaf_distance(parent, query),
        })
        .fold(f64::MAX, f64::min);
    // The envelope distance must be a lower bound on every leaf below it.
    let env_dist = node.envelope().distance_2(&query);
    assert!(
        env_dist <= min_leaf + 1e-6,
        "envelope distance {env_dist} exceeds nearest leaf {min_leaf} for query ({}, {})",
        query.lon,
        query.lat
    );
    min_leaf
}

#[test]
fn point_to_mbr_lower_bounds_leaf_distance() {
    let mut rng = StdRng::seed_from_u64(0xA1B2_C3D4_E5F6_0718);
    let points = random_points(&mut rng, 1000);
    let tree = RTree::bulk_load(points);

    for query in random_queries(&mut rng, 100) {
        min_leaf_distance(tree.root(), query);
    }
}

// ---------------------------------------------------------------------------
// Test 4: antimeridian limitation (documented behaviour, §10.6)
// ---------------------------------------------------------------------------

#[test]
fn antimeridian_is_not_handled() {
    // Two points at (179.9, 0) and (-179.9, 0) are ~22 km apart on the sphere,
    // but the bulk-loaded MBR spans lon [-179.9, 179.9] (the long way round).
    // The leaf-level haversine still handles the wrap, so nearest_neighbor is
    // correct; only MBR pruning is suboptimal.  This test pins that documented
    // behaviour.
    let tree = RTree::bulk_load(vec![
        GeodeticPoint::new(179.9, 0.0),
        GeodeticPoint::new(-179.9, 0.0),
    ]);
    let query = GeodeticCoord {
        lon: 180.0,
        lat: 0.0,
    };
    let nn = tree.nearest_neighbor(query).unwrap();
    let lon = nn.0.lon;
    assert!(
        (lon - 179.9).abs() < 0.01 || (lon + 179.9).abs() < 0.01,
        "expected one of the two seam points, got lon {lon}"
    );
}
