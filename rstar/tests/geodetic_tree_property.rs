#![cfg(feature = "geodetic")]

//! Property tests for the geodetic R-tree at the *tree* level, complementing the
//! point-to-MBR oracle in `geodetic_property.rs`.
//!
//! These check the end-to-end consequences of the paper's lower-bound contract:
//! that branch-and-bound queries against the tree return the same answers as a
//! linear scan, and that the underlying envelope arithmetic preserves the
//! invariants the proof needs (monotone under containment, every descendant
//! contained by every ancestor envelope).
//!
//! Each `#[hegel::test]` body is run many times with `tc.draw(...)`-generated
//! inputs and shrinks any failure to a minimal counterexample. See the comment
//! block in `src/geodetic/distance.rs` for how the Hegel property tests work.
//!
//! # Input-domain restriction
//!
//! Coordinates produced by `draw_lon` / `draw_lat` (and so by every test in this
//! file except the MBR bounds in `prop_mbr_distance_monotone_under_containment`,
//! which still use the unrestricted `ordered`) are snapped to nanodegree precision
//! (~11 cm at the equator). These tests therefore do **not** exercise:
//!
//! - the f64 subnormal range, or any coordinate magnitude below `5e-10` deg;
//! - the regime where two distinct points round to the same nanodegree tick;
//! - the haversine-underflow zone, where `(sin(Δθ/2))²` underflows to 0 while the
//!   linear meridian-arc formula used by `point_to_mbr_distance` does not (i.e.
//!   the two distance functions disagree on whether the distance is zero).
//!
//! See the doc comment on `TICKS_PER_DEG` below for why -- in short, the linear
//! arc and haversine diverge in the underflow zone, breaking the
//! `tree == linear-scan` premise of these oracle tests, and the divergence is a
//! property of the two formulas rather than of the tree.
//!
//! Full-f64-range coverage of `point_to_mbr_distance` itself (with no snapping)
//! still happens in `tests/geodetic_property.rs` and in the `prop_*` block of
//! `src/geodetic/distance.rs`; those use a brute-force edge sweep as the oracle
//! rather than cross-comparing against haversine, so they are insensitive to the
//! underflow asymmetry.

use hegel::TestCase;
use hegel::generators;

use rstar::geodetic::distance::{haversine_distance, point_to_mbr_distance};
use rstar::geodetic::{GeodeticCoord, GeodeticPoint};
use rstar::{Envelope, ParentNode, RTree, RTreeNode};

fn coord(lon: f64, lat: f64) -> GeodeticCoord {
    GeodeticCoord { lon, lat }
}

/// Nanodegrees per degree (1 ndeg ~= 11 cm at the equator). All generated coordinates
/// are snapped to this grid: realistic for geographic data, and keeps the smallest
/// non-zero latitude difference well above `f64` subnormals. Without this, Hegel can
/// shrink to subnormal-scale coordinates (e.g. lat = 3.44e-227) where the haversine
/// `(sin(Δθ/2))²` term underflows to 0 while the linear meridian-arc formula used by
/// `point_to_mbr_distance` does not: an actual, but for realistic inputs
/// irrelevant, disagreement that breaks `tree == linear-scan` oracle comparisons.
const TICKS_PER_DEG: f64 = 1.0e9;

fn snap(x: f64) -> f64 {
    (x * TICKS_PER_DEG).round() / TICKS_PER_DEG
}

fn draw_lon(tc: &TestCase) -> f64 {
    snap(
        tc.draw(
            generators::floats::<f64>()
                .min_value(-180.0)
                .max_value(180.0),
        ),
    )
}

fn draw_lat(tc: &TestCase) -> f64 {
    snap(tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)))
}

fn draw_point(tc: &TestCase) -> GeodeticPoint {
    GeodeticPoint::new(draw_lon(tc), draw_lat(tc))
}

fn draw_points(tc: &TestCase, n: usize) -> Vec<GeodeticPoint> {
    (0..n).map(|_| draw_point(tc)).collect()
}

/// Draws an ordered pair `(low, high)` within `[min, max]` so a generated MBR never
/// wraps across the antimeridian (the documented precondition of `point_to_mbr_distance`).
///
/// Uses [`f64::total_cmp`] rather than `<=`: with `<=`, drawing `+0.0` then `-0.0` would
/// produce the pair `(+0.0, -0.0)`, which then panics a downstream
/// `min_value(+0.0).max_value(-0.0)` generator (IEEE bit-order sees `-0.0 < +0.0`, so
/// the range is empty).
fn ordered(tc: &TestCase, min: f64, max: f64) -> (f64, f64) {
    let a = tc.draw(generators::floats::<f64>().min_value(min).max_value(max));
    let b = tc.draw(generators::floats::<f64>().min_value(min).max_value(max));
    if a.total_cmp(&b) == std::cmp::Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    }
}

// ---------------------------------------------------------------------------
// 1. `locate_within_distance` returns exactly the same set as a linear-scan filter.
// ---------------------------------------------------------------------------
//
// Radius queries are one of the two query types Schubert et al. motivate; the
// branch-and-bound pruning uses the same point-to-MBR lower bound as NN, so any
// over-estimate would surface here as missing results.

#[hegel::test(test_cases = 200)]
fn prop_locate_within_distance_matches_linear_scan(tc: TestCase) {
    let points = draw_points(&tc, 50);
    let query = coord(draw_lon(&tc), draw_lat(&tc));
    // Range covers "no hits" through "every point on the sphere" -- the Earth's
    // half-circumference is ~20 015 km, so 25 000 km guarantees the upper end
    // includes everything.
    let radius = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(25_000_000.0),
    );

    let tree = RTree::bulk_load(points.clone());

    let mut from_tree: Vec<GeodeticPoint> = tree
        .locate_within_distance(query, radius)
        .copied()
        .collect();
    let mut from_scan: Vec<GeodeticPoint> = points
        .iter()
        .copied()
        .filter(|p| haversine_distance(p.0, query) <= radius)
        .collect();

    // Compare as sets via a deterministic sort on the raw f64 bits (the tree may
    // return matches in any order; ties on identical coordinates are fine).
    let sort_key = |p: &GeodeticPoint| (p.0.lon.to_bits(), p.0.lat.to_bits());
    from_tree.sort_by_key(sort_key);
    from_scan.sort_by_key(sort_key);

    assert_eq!(
        from_tree, from_scan,
        "locate_within_distance disagrees with linear scan; \
         query=({},{}) radius={radius}",
        query.lon, query.lat
    );
}

// ---------------------------------------------------------------------------
// 2. `nearest_neighbor_iter_with_distance_2` is a permutation of a sorted linear scan.
// ---------------------------------------------------------------------------
//
// Strengthens the rand-driven monotonicity test in `geodetic_rtree.rs`: that test
// would pass even if the iterator yielded the same (wrong) point in increasing
// rank. Comparing the full distance sequence against the sorted brute-force
// catches that class of bug.

#[hegel::test(test_cases = 200)]
fn prop_nn_iter_matches_sorted_linear_scan(tc: TestCase) {
    let points = draw_points(&tc, 30);
    let query = coord(draw_lon(&tc), draw_lat(&tc));

    let tree = RTree::bulk_load(points.clone());
    let tree_dists: Vec<f64> = tree
        .nearest_neighbor_iter_with_distance_2(query)
        .map(|(_, d)| d)
        .collect();

    let mut scan_dists: Vec<f64> = points
        .iter()
        .map(|p| haversine_distance(p.0, query))
        .collect();
    scan_dists.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert_eq!(
        tree_dists.len(),
        scan_dists.len(),
        "iterator length mismatch; query=({},{})",
        query.lon,
        query.lat
    );
    for (rank, (t, s)) in tree_dists.iter().zip(scan_dists.iter()).enumerate() {
        assert!(
            (t - s).abs() <= 1e-6,
            "rank {rank}: tree={t} scan={s}; query=({},{})",
            query.lon,
            query.lat
        );
    }
}

// ---------------------------------------------------------------------------
// 3. `nearest_neighbor` matches a linear-scan minimum.
// ---------------------------------------------------------------------------
//
// The same property as `nearest_neighbor_matches_brute_force` in `geodetic_rtree.rs`
// but Hegel-driven, so failures shrink to a minimal counterexample rather than a
// fixed RNG seed.

#[hegel::test(test_cases = 200)]
fn prop_nearest_neighbor_matches_linear_scan(tc: TestCase) {
    let points = draw_points(&tc, 30);
    let query = coord(draw_lon(&tc), draw_lat(&tc));

    let tree = RTree::bulk_load(points.clone());
    let nn = tree
        .nearest_neighbor(query)
        .expect("non-empty tree must return a nearest neighbour");
    let tree_dist = haversine_distance(nn.0, query);

    let scan_best = points
        .iter()
        .map(|p| haversine_distance(p.0, query))
        .fold(f64::INFINITY, f64::min);

    assert!(
        (tree_dist - scan_best).abs() <= 1e-6,
        "tree NN dist {tree_dist} != linear-scan best {scan_best}; \
         query=({},{})",
        query.lon,
        query.lat
    );
}

// ---------------------------------------------------------------------------
// 4. `point_to_mbr_distance` is monotone under MBR containment.
// ---------------------------------------------------------------------------
//
// If MBR A is contained in MBR B, then dist(q, B) <= dist(q, A) for every query q.
// This is the property the branch-and-bound proof actually relies on: a child
// envelope's distance must never exceed its parent's, otherwise pruning could
// discard a subtree that holds a closer point. The point-to-MBR oracle test checks
// the absolute value; this checks the *relation* the tree's correctness depends on.

#[hegel::test(test_cases = 500)]
fn prop_mbr_distance_monotone_under_containment(tc: TestCase) {
    let (lon_l, lon_h) = ordered(&tc, -180.0, 180.0);
    let (lat_l, lat_h) = ordered(&tc, -90.0, 90.0);
    // Inner rectangle drawn strictly inside the outer (degenerate sub-rectangles allowed).
    let (in_lon_l, in_lon_h) = ordered(&tc, lon_l, lon_h);
    let (in_lat_l, in_lat_h) = ordered(&tc, lat_l, lat_h);
    let q = coord(draw_lon(&tc), draw_lat(&tc));

    let d_outer = point_to_mbr_distance(q, lon_l, lat_l, lon_h, lat_h);
    let d_inner = point_to_mbr_distance(q, in_lon_l, in_lat_l, in_lon_h, in_lat_h);

    assert!(
        d_outer <= d_inner + 1e-3,
        "outer={d_outer} > inner={d_inner}; \
         outer=[{lon_l},{lat_l}]-[{lon_h},{lat_h}], \
         inner=[{in_lon_l},{in_lat_l}]-[{in_lon_h},{in_lat_h}], \
         query=({},{})",
        q.lon,
        q.lat
    );
}

// ---------------------------------------------------------------------------
// 5. Structural invariant: every parent envelope contains all of its descendants.
// ---------------------------------------------------------------------------
//
// Generic to any R-tree, but worth restating because we ship a custom `Envelope`
// impl: a bug in `merge`/`merged`/`contains_*` would corrupt the index silently
// (the lower-bound recursion would catch only some cases).

fn check_envelope_contains_all(node: &ParentNode<GeodeticPoint>) {
    let env = node.envelope();
    for child in node.children() {
        match child {
            RTreeNode::Leaf(p) => {
                assert!(
                    env.contains_point(&p.0),
                    "parent envelope does not contain leaf ({},{}); \
                     envelope=[{},{}]-[{},{}]",
                    p.0.lon,
                    p.0.lat,
                    env.lower().lon,
                    env.lower().lat,
                    env.upper().lon,
                    env.upper().lat
                );
            }
            RTreeNode::Parent(parent) => {
                let child_env = parent.envelope();
                assert!(
                    env.contains_envelope(&child_env),
                    "parent envelope does not contain child envelope; \
                     parent=[{},{}]-[{},{}], child=[{},{}]-[{},{}]",
                    env.lower().lon,
                    env.lower().lat,
                    env.upper().lon,
                    env.upper().lat,
                    child_env.lower().lon,
                    child_env.lower().lat,
                    child_env.upper().lon,
                    child_env.upper().lat,
                );
                check_envelope_contains_all(parent);
            }
        }
    }
}

#[hegel::test(test_cases = 100)]
fn prop_parent_envelope_contains_all_descendants(tc: TestCase) {
    // With DefaultParams (MAX_SIZE = 6), 30 points forces at least one internal level.
    let points = draw_points(&tc, 30);
    let tree = RTree::bulk_load(points);
    check_envelope_contains_all(tree.root());
}
