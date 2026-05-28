#![cfg(feature = "geodetic")]

//! Property tests for the geodetic R-tree at the *tree* level, driving the
//! [`Geodetic3DTree`] facade.
//!
//! These check the end-to-end consequences of the pruning lower-bound contract:
//! that branch-and-bound queries return the same answers as a linear scan, and
//! that the underlying envelope arithmetic preserves the invariants the proof
//! needs (the point-to-box value is a lower bound on every contained leaf, the
//! `min_max_dist_2` value is an upper bound on the nearest contained leaf, and
//! every ancestor box contains every descendant).
//!
//! Each `#[hegel::test]` body is run many times with `tc.draw(...)`-generated
//! inputs and shrinks any failure to a minimal counterexample.
//!
//! # Full-domain coverage
//!
//! Unlike the previous 2D design, there is no nanodegree snapping: the leaf metric
//! and the envelope bound are the same Euclidean function on the same vectors, so
//! the two-formula divergence that forced snapping no longer exists. Generators
//! draw `lon ∈ [-180, 180]` and `lat ∈ [-90, 90]` across the full range, including
//! ±180° and the poles.

use hegel::TestCase;
use hegel::generators;

use rstar::geodetic::distance::{
    EARTH_RADIUS_METRES, haversine_distance, metres_to_squared_chord, squared_chord_to_metres,
};
use rstar::geodetic::{Geodetic3DTree, GeodeticCoord, GeodeticPoint, UnitVec};
use rstar::{Envelope, ParentNode, PointDistance, RTreeNode};

const HALF_CIRCUMFERENCE: f64 = std::f64::consts::PI * EARTH_RADIUS_METRES;

fn coord(lon: f64, lat: f64) -> GeodeticCoord {
    GeodeticCoord { lon, lat }
}

fn draw_lon(tc: &TestCase) -> f64 {
    tc.draw(
        generators::floats::<f64>()
            .min_value(-180.0)
            .max_value(180.0),
    )
}

fn draw_lat(tc: &TestCase) -> f64 {
    tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0))
}

fn draw_point(tc: &TestCase) -> GeodeticPoint {
    GeodeticPoint::new(draw_lon(tc), draw_lat(tc))
}

fn draw_points(tc: &TestCase, n: usize) -> Vec<GeodeticPoint> {
    (0..n).map(|_| draw_point(tc)).collect()
}

// ---------------------------------------------------------------------------
// 1. `nearest_neighbor` matches a linear-scan minimum.
// ---------------------------------------------------------------------------

#[hegel::test(test_cases = 200)]
fn prop_nearest_neighbor_matches_linear_scan(tc: TestCase) {
    let points = draw_points(&tc, 30);
    if points.is_empty() {
        return;
    }
    let query = coord(draw_lon(&tc), draw_lat(&tc));

    let tree = Geodetic3DTree::bulk_load(points.clone());
    let nn = tree
        .nearest_neighbor(query)
        .expect("non-empty tree must return a nearest neighbour");
    let tree_dist = haversine_distance(nn.coord(), query);

    let scan_best = points
        .iter()
        .map(|p| haversine_distance(p.coord(), query))
        .fold(f64::INFINITY, f64::min);

    let tol = 1e-3 + scan_best * 1e-9;
    assert!(
        (tree_dist - scan_best).abs() <= tol,
        "tree NN dist {tree_dist} != linear-scan best {scan_best}; query=({},{})",
        query.lon,
        query.lat
    );
}

// ---------------------------------------------------------------------------
// 2. `nearest_neighbor_iter_with_distance` is a sorted linear scan in metres.
// ---------------------------------------------------------------------------

#[hegel::test(test_cases = 200)]
fn prop_nn_iter_matches_sorted_linear_scan(tc: TestCase) {
    let points = draw_points(&tc, 30);
    let query = coord(draw_lon(&tc), draw_lat(&tc));

    let tree = Geodetic3DTree::bulk_load(points.clone());
    let tree_dists: Vec<f64> = tree
        .nearest_neighbor_iter_with_distance(query)
        .map(|(_, d)| d)
        .collect();

    // The metres sequence is non-decreasing.
    for w in tree_dists.windows(2) {
        assert!(
            w[0] <= w[1] + 1e-6,
            "distances not non-decreasing: {} > {}; query=({},{})",
            w[0],
            w[1],
            query.lon,
            query.lat
        );
    }

    let mut scan_dists: Vec<f64> = points
        .iter()
        .map(|p| haversine_distance(p.coord(), query))
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
        let tol = 1e-3 + s * 1e-9;
        assert!(
            (t - s).abs() <= tol,
            "rank {rank}: tree={t} scan={s}; query=({},{})",
            query.lon,
            query.lat
        );
    }
}

// ---------------------------------------------------------------------------
// 3. `locate_within_distance` returns the same set as a linear-scan filter.
// ---------------------------------------------------------------------------
//
// The metre<->chord round trip is not bit-exact, so points within a small band of
// the threshold may tie. Points strictly inside or strictly outside (beyond the
// band) must agree with the linear scan; points inside the band are excluded.

#[hegel::test(test_cases = 200)]
fn prop_locate_within_distance_matches_linear_scan(tc: TestCase) {
    let points = draw_points(&tc, 50);
    let query = coord(draw_lon(&tc), draw_lat(&tc));
    // Cap at the half-circumference (the maximum possible great-circle distance).
    // Above it, `metres_to_squared_chord` clamps to c2 = 4 and every point matches,
    // so the test would degenerate to "return everything" and stop probing the
    // chord-threshold boundary. Capping here keeps the whole range boundary-relevant.
    let radius = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(HALF_CIRCUMFERENCE),
    );

    let tree = Geodetic3DTree::bulk_load(points.clone());

    // Tolerance band around the threshold, in metres.
    const BAND: f64 = 1.0;

    let from_tree: Vec<GeodeticCoord> = tree
        .locate_within_distance(query, radius)
        .map(|p| p.coord())
        .collect();

    // Every returned point must be within radius + band.
    for c in &from_tree {
        let d = haversine_distance(*c, query);
        assert!(
            d <= radius + BAND,
            "returned point at {d} m exceeds radius {radius} m; query=({},{})",
            query.lon,
            query.lat
        );
    }

    // Every point comfortably inside the radius must be returned.
    let key = |c: &GeodeticCoord| (c.lon.to_bits(), c.lat.to_bits());
    let mut tree_set = from_tree.clone();
    tree_set.sort_by_key(key);
    for p in &points {
        let d = haversine_distance(p.coord(), query);
        if d <= radius - BAND {
            let c = p.coord();
            assert!(
                tree_set.binary_search_by_key(&key(&c), key).is_ok(),
                "point at {d} m (radius {radius} m) missing from result; query=({},{})",
                query.lon,
                query.lat
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Envelope distance is a lower bound on every contained leaf (squared chord).
// ---------------------------------------------------------------------------

fn min_leaf_distance(node: &ParentNode<GeodeticPoint>, query: UnitVec) -> f64 {
    let min_leaf = node
        .children()
        .iter()
        .map(|child| match child {
            RTreeNode::Leaf(p) => p.distance_2(&query),
            RTreeNode::Parent(parent) => min_leaf_distance(parent, query),
        })
        .fold(f64::MAX, f64::min);
    let env_dist = node.envelope().distance_2(&query);
    assert!(
        env_dist <= min_leaf + 1e-12,
        "envelope distance {env_dist} exceeds nearest leaf {min_leaf}"
    );
    min_leaf
}

#[hegel::test(test_cases = 100)]
fn prop_envelope_distance_is_lower_bound(tc: TestCase) {
    let points = draw_points(&tc, 30);
    let tree = Geodetic3DTree::bulk_load(points);
    let query = UnitVec::from(coord(draw_lon(&tc), draw_lat(&tc)));
    min_leaf_distance(tree.root(), query);
}

// ---------------------------------------------------------------------------
// 5. `min_max_dist_2` is an upper bound on the nearest contained leaf.
// ---------------------------------------------------------------------------

fn check_min_max_dist(node: &ParentNode<GeodeticPoint>, query: UnitVec) -> f64 {
    let min_leaf = node
        .children()
        .iter()
        .map(|child| match child {
            RTreeNode::Leaf(p) => p.distance_2(&query),
            RTreeNode::Parent(parent) => check_min_max_dist(parent, query),
        })
        .fold(f64::MAX, f64::min);
    let mmd = node.envelope().min_max_dist_2(&query);
    assert!(
        mmd + 1e-9 >= min_leaf,
        "min_max_dist_2 {mmd} below nearest contained leaf {min_leaf}"
    );
    min_leaf
}

#[hegel::test(test_cases = 100)]
fn prop_min_max_dist_2_is_upper_bound(tc: TestCase) {
    let points = draw_points(&tc, 30);
    let tree = Geodetic3DTree::bulk_load(points);
    let query = UnitVec::from(coord(draw_lon(&tc), draw_lat(&tc)));
    check_min_max_dist(tree.root(), query);
}

// ---------------------------------------------------------------------------
// 6. Structural invariant: every parent envelope contains all descendants.
// ---------------------------------------------------------------------------

fn check_envelope_contains_all(node: &ParentNode<GeodeticPoint>) {
    let env = node.envelope();
    for child in node.children() {
        match child {
            RTreeNode::Leaf(p) => {
                assert!(
                    env.contains_point(&p.unit_vec()),
                    "parent envelope does not contain leaf"
                );
            }
            RTreeNode::Parent(parent) => {
                assert!(
                    env.contains_envelope(&parent.envelope()),
                    "parent envelope does not contain child envelope"
                );
                check_envelope_contains_all(parent);
            }
        }
    }
}

#[hegel::test(test_cases = 100)]
fn prop_parent_envelope_contains_descendants(tc: TestCase) {
    let points = draw_points(&tc, 30);
    let tree = Geodetic3DTree::bulk_load(points);
    check_envelope_contains_all(tree.root());
}

// ---------------------------------------------------------------------------
// 7. Ordering consistency: squared chord and haversine induce the same order.
// ---------------------------------------------------------------------------
//
// Pins the monotonicity the whole design rests on: for any q, a, b, the sign of
// (c²(q,a) − c²(q,b)) equals the sign of (haversine(q,a) − haversine(q,b)).

#[hegel::test(test_cases = 500)]
fn prop_ordering_consistency(tc: TestCase) {
    let q = coord(draw_lon(&tc), draw_lat(&tc));
    let a = coord(draw_lon(&tc), draw_lat(&tc));
    let b = coord(draw_lon(&tc), draw_lat(&tc));

    let qv = UnitVec::from(q);
    let chord_a = GeodeticPoint::from(a).distance_2(&qv);
    let chord_b = GeodeticPoint::from(b).distance_2(&qv);
    let hav_a = haversine_distance(q, a);
    let hav_b = haversine_distance(q, b);

    // Compare via the metre conversion of the chord, with a tolerance band: near
    // ties (within ~1 m) are allowed to disagree on sign because both formulas
    // carry rounding error there.
    let chord_a_m = squared_chord_to_metres(chord_a);
    let chord_b_m = squared_chord_to_metres(chord_b);
    let chord_diff = chord_a_m - chord_b_m;
    let hav_diff = hav_a - hav_b;

    if chord_diff.abs() > 1.0 && hav_diff.abs() > 1.0 {
        assert!(
            chord_diff.signum() == hav_diff.signum(),
            "ordering disagreement: chord_diff={chord_diff} hav_diff={hav_diff}; \
             q=({},{}) a=({},{}) b=({},{})",
            q.lon,
            q.lat,
            a.lon,
            a.lat,
            b.lon,
            b.lat
        );
    }
}

// ---------------------------------------------------------------------------
// 8. metres <-> squared-chord round trip.
// ---------------------------------------------------------------------------

#[hegel::test(test_cases = 500)]
fn prop_metres_chord_round_trip(tc: TestCase) {
    let r = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(HALF_CIRCUMFERENCE),
    );
    let back = squared_chord_to_metres(metres_to_squared_chord(r));
    // Near the antipode (r ≈ π·R) the chord->angle inversion via asin is
    // ill-conditioned (sin(d/2) is flat near 1), so allow a tolerance scaling
    // with r (a few mm at half-circumference).
    let tol = 1e-3 + r * 1e-9;
    assert!(
        (back - r).abs() <= tol,
        "round trip diverged: r={r} back={back}"
    );
}

// ---------------------------------------------------------------------------
// 9. `locate_in_rectangle` matches a brute-force longitude/latitude filter.
// ---------------------------------------------------------------------------

/// Independent oracle: longitude/latitude rectangle membership with the same
/// eastward-arc and pole conventions the facade documents, written out here so it
/// shares no code with the implementation under test.
fn point_in_rectangle(lower: GeodeticCoord, upper: GeodeticCoord, p: GeodeticCoord) -> bool {
    if p.lat < lower.lat || p.lat > upper.lat {
        return false;
    }
    if p.lat.abs() == 90.0 {
        return true;
    }
    if lower.lon <= upper.lon {
        lower.lon <= p.lon && p.lon <= upper.lon
    } else {
        p.lon >= lower.lon || p.lon <= upper.lon
    }
}

#[hegel::test(test_cases = 300)]
fn prop_locate_in_rectangle_matches_brute_force(tc: TestCase) {
    let points = draw_points(&tc, 40);
    let tree = Geodetic3DTree::bulk_load(points.clone());

    // A rectangle with a valid latitude band (sort the two drawn latitudes) and an
    // arbitrary eastward longitude arc, which may wrap across the antimeridian.
    let lat_a = draw_lat(&tc);
    let lat_b = draw_lat(&tc);
    let (lat_lo, lat_hi) = if lat_a <= lat_b {
        (lat_a, lat_b)
    } else {
        (lat_b, lat_a)
    };
    let lower = coord(draw_lon(&tc), lat_lo);
    let upper = coord(draw_lon(&tc), lat_hi);

    let mut from_tree: Vec<GeodeticCoord> = tree
        .locate_in_rectangle(lower, upper)
        .map(|p| p.coord())
        .collect();
    let mut from_scan: Vec<GeodeticCoord> = points
        .iter()
        .map(|p| p.coord())
        .filter(|c| point_in_rectangle(lower, upper, *c))
        .collect();

    let key = |c: &GeodeticCoord| (c.lon.to_bits(), c.lat.to_bits());
    from_tree.sort_by_key(key);
    from_scan.sort_by_key(key);
    assert_eq!(from_tree, from_scan);
}
