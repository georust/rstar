#![cfg(feature = "geodetic")]

//! Tree-level property tests for extent leaves (currently [`GeodeticLineString`]),
//! driving a `GeodeticRTree<GeodeticLineString>`.
//!
//! The point-to-geometry distance is checked against the externally-anchored
//! `point_to_linestring_c2` oracle (the minimum over edges of `nearest_arc_c2`, which
//! `ARC_GOLDEN` validates per edge), and the branch-and-bound queries are checked
//! against a linear scan using that oracle. The MINDIST / MINMAXDIST / containment
//! invariants are walked over every node.

mod geodetic_common;
use geodetic_common::*;

use std::collections::HashSet;

use hegel::generators;
use hegel::TestCase;

use rstar::geodetic::{
    squared_chord_to_metres, GeodeticCoord, GeodeticError, GeodeticLineString, GeodeticPolygon,
    GeodeticRTree, GeodeticRing, UnitVec, EARTH_RADIUS_METRES,
};
use rstar::{Envelope, PointDistance, RTreeObject};

const HALF_CIRCUMFERENCE: f64 = std::f64::consts::PI * EARTH_RADIUS_METRES;

fn line(verts: &[GeodeticCoord]) -> GeodeticLineString {
    GeodeticLineString::try_from_lonlat(verts.iter().copied()).expect("valid generated linestring")
}

fn oracle_metres(query: GeodeticCoord, verts: &[GeodeticCoord]) -> f64 {
    squared_chord_to_metres(point_to_linestring_c2(query, verts))
}

fn key(verts: &[GeodeticCoord]) -> Vec<(u64, u64)> {
    verts
        .iter()
        .map(|c| (c.lon.to_bits(), c.lat.to_bits()))
        .collect()
}

fn draw_lines(tc: &TestCase, n: usize) -> Vec<Vec<GeodeticCoord>> {
    (0..n).map(|_| draw_linestring(tc)).collect()
}

// ---------------------------------------------------------------------------
// Leaf distance against the external oracle (squared chord).
// ---------------------------------------------------------------------------

#[hegel::test(test_cases = 5000)]
fn prop_point_to_linestring_matches_oracle(tc: TestCase) {
    let verts = draw_linestring(&tc);
    let query = draw_coord(&tc);
    let ls = line(&verts);
    let implementation = ls.distance_2(&query.to_unit_vector());
    let oracle = point_to_linestring_c2(query, &verts);
    // Squared chord (well conditioned at the antipode, unlike metres). The oracle is the
    // minimum over edges of nearest_arc_c2, which ARC_GOLDEN anchors per edge.
    let tol = 1e-7 + oracle * 1e-9;
    assert!(
        (implementation - oracle).abs() <= tol,
        "linestring c2 {implementation} != oracle {oracle}; query=({},{})",
        query.lon,
        query.lat
    );
}

// ---------------------------------------------------------------------------
// Pruning invariants, walked over every node.
// ---------------------------------------------------------------------------

#[hegel::test(test_cases = 100)]
fn prop_envelope_distance_is_lower_bound(tc: TestCase) {
    let tree = GeodeticRTree::bulk_load(draw_lines(&tc, 30).iter().map(|v| line(v)).collect());
    let query = draw_coord(&tc).to_unit_vector();
    min_leaf_distance(tree.root(), query);
}

#[hegel::test(test_cases = 100)]
fn prop_min_max_dist_2_is_upper_bound(tc: TestCase) {
    let tree = GeodeticRTree::bulk_load(draw_lines(&tc, 30).iter().map(|v| line(v)).collect());
    let query = draw_coord(&tc).to_unit_vector();
    check_min_max_dist(tree.root(), query);
}

#[hegel::test(test_cases = 100)]
fn prop_parent_envelope_contains_descendants(tc: TestCase) {
    let tree = GeodeticRTree::bulk_load(draw_lines(&tc, 30).iter().map(|v| line(v)).collect());
    check_envelope_contains_all(tree.root());
}

// ---------------------------------------------------------------------------
// Branch-and-bound queries against a linear scan using the external oracle.
// ---------------------------------------------------------------------------

#[hegel::test(test_cases = 200)]
fn prop_nn_matches_linear_scan(tc: TestCase) {
    let verts_list = draw_lines(&tc, 30);
    let query = draw_coord(&tc);
    let tree = GeodeticRTree::bulk_load(verts_list.iter().map(|v| line(v)).collect());

    let (_, tree_metres) = tree
        .nearest_neighbor_with_distance(query)
        .expect("non-empty tree");
    let scan_best = verts_list
        .iter()
        .map(|v| oracle_metres(query, v))
        .fold(f64::INFINITY, f64::min);

    let tol = 1e-3 + scan_best * 1e-9;
    assert!(
        (tree_metres - scan_best).abs() <= tol,
        "tree NN {tree_metres} != linear-scan best {scan_best}; query=({},{})",
        query.lon,
        query.lat
    );
}

#[hegel::test(test_cases = 200)]
fn prop_nn_iter_matches_sorted_linear_scan(tc: TestCase) {
    let verts_list = draw_lines(&tc, 20);
    let query = draw_coord(&tc);
    let tree = GeodeticRTree::bulk_load(verts_list.iter().map(|v| line(v)).collect());

    let tree_dists: Vec<f64> = tree
        .nearest_neighbor_iter_with_distance(query)
        .map(|(_, d)| d)
        .collect();
    for w in tree_dists.windows(2) {
        assert!(w[0] <= w[1] + 1e-6, "distances not non-decreasing");
    }

    let mut scan: Vec<f64> = verts_list.iter().map(|v| oracle_metres(query, v)).collect();
    scan.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert_eq!(tree_dists.len(), scan.len(), "iterator length mismatch");
    for (rank, (t, s)) in tree_dists.iter().zip(scan.iter()).enumerate() {
        let tol = 1e-3 + s * 1e-9;
        assert!((t - s).abs() <= tol, "rank {rank}: tree={t} scan={s}");
    }
}

#[hegel::test(test_cases = 200)]
fn prop_locate_within_distance_matches_linear_scan(tc: TestCase) {
    let verts_list = draw_lines(&tc, 40);
    let query = draw_coord(&tc);
    let radius = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(HALF_CIRCUMFERENCE),
    );
    let tree = GeodeticRTree::bulk_load(verts_list.iter().map(|v| line(v)).collect());

    const BAND: f64 = 1.0; // metres

    let returned: Vec<&GeodeticLineString> = tree.locate_within_distance(query, radius).collect();
    let returned_keys: HashSet<Vec<(u64, u64)>> =
        returned.iter().map(|l| key(l.coords())).collect();

    // Everything returned is within radius + band.
    for l in &returned {
        let d = oracle_metres(query, l.coords());
        assert!(
            d <= radius + BAND,
            "returned geometry at {d} m exceeds radius {radius} m"
        );
    }
    // Everything comfortably within the radius is returned.
    for v in &verts_list {
        if oracle_metres(query, v) <= radius - BAND {
            assert!(
                returned_keys.contains(&key(v)),
                "geometry within radius {radius} m missing from result"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Section 8.4 explicit edge cases.
// ---------------------------------------------------------------------------

#[test]
fn e1_antimeridian_linestring_nn_and_radius_match_oracle() {
    let target = vec![coord(170.0, 0.0), coord(180.0, 5.0), coord(-175.0, 10.0)];
    let distractor = vec![coord(0.0, 0.0), coord(10.0, 10.0)];
    let tree = GeodeticRTree::bulk_load(vec![line(&target), line(&distractor)]);

    let query = coord(178.0, 6.0); // near the seam, near the target
    let (nn, metres) = tree
        .nearest_neighbor_with_distance(query)
        .expect("non-empty");
    let expected = oracle_metres(query, &target);
    assert!(
        (metres - expected).abs() <= 1e-3 + expected * 1e-9,
        "seam NN {metres} != oracle {expected}"
    );
    assert_eq!(
        nn.coords()[0],
        coord(170.0, 0.0),
        "should return the target"
    );

    // A radius just past the target's distance includes it, with no wrapping helper.
    let within: Vec<_> = tree
        .locate_within_distance(query, expected + 1_000.0)
        .collect();
    assert!(within.iter().any(|l| l.coords()[0] == coord(170.0, 0.0)));
}

#[test]
fn e4_degenerate_inputs() {
    // (a) Duplicate consecutive vertices construct and match the non-degenerate version.
    let dup = line(&[coord(10.0, 10.0), coord(10.0, 10.0), coord(20.0, 20.0)]);
    let plain = line(&[coord(10.0, 10.0), coord(20.0, 20.0)]);
    let q = coord(15.0, 16.0).to_unit_vector();
    assert!((dup.distance_2(&q) - plain.distance_2(&q)).abs() < 1e-12);

    // (b) A single vertex is rejected.
    assert_eq!(
        GeodeticLineString::try_from_lonlat([(10.0, 10.0)]),
        Err(GeodeticError::TooFewPoints {
            found: 1,
            needed: 2
        })
    );

    // (c) An exact-antipode edge is rejected.
    assert_eq!(
        GeodeticLineString::try_from_lonlat([(0.0, 0.0), (180.0, 0.0)]),
        Err(GeodeticError::EdgeSpansHalfCircle { index: 0 })
    );

    // (d) An empty tree returns nothing.
    let tree: GeodeticRTree<GeodeticLineString> = GeodeticRTree::new();
    assert!(tree
        .nearest_neighbor_with_distance(coord(0.0, 0.0))
        .is_none());
    assert_eq!(tree.locate_within_distance(coord(0.0, 0.0), 1e6).count(), 0);
}

#[test]
fn e5_seam_straddling_cluster_forces_internal_node() {
    // More than node capacity of seam-crossing linestrings, so the R*-tree forms at
    // least one internal node whose envelope merges across the seam.
    let mut lines = Vec::new();
    for k in 0..40 {
        let lat = -20.0 + k as f64;
        lines.push(line(&[coord(175.0, lat), coord(-175.0, lat + 0.5)]));
    }
    let tree = GeodeticRTree::bulk_load(lines);
    let query = coord(180.0, 0.0).to_unit_vector();
    // Re-check the lower-bound and containment invariants at every node for a seam query.
    min_leaf_distance(tree.root(), query);
    check_envelope_contains_all(tree.root());
}

// ---------------------------------------------------------------------------
// Polygon membership, box, and tree invariants.
//
// Spherical caps are the exact point-in-polygon oracle at every size, so they validate
// the ray-cast membership over the full sphere, including caps larger than a hemisphere
// (alpha > 90) that the signed-winding test could not handle.
// ---------------------------------------------------------------------------

const CAP_SIDES: usize = 128;

fn cap_polygon(cap: &Cap) -> GeodeticPolygon {
    let ring = GeodeticRing::try_from_lonlat(cap_ring(cap, CAP_SIDES)).expect("valid ring");
    GeodeticPolygon::try_new(ring, Vec::new()).expect("valid polygon")
}

fn cap_polys(tc: &TestCase, n: usize) -> Vec<GeodeticPolygon> {
    (0..n).map(|_| cap_polygon(&draw_cap(tc))).collect()
}

/// Squared chord from `q` to the polygon boundary: the minimum over every ring (exterior
/// and holes) of the externally-anchored [`point_to_linestring_c2`] arc oracle. For a query
/// outside the filled region this is the polygon's distance, computed without the embedding's
/// own `arc_distance_2` path.
fn polygon_boundary_oracle_c2(q: GeodeticCoord, poly: &GeodeticPolygon) -> f64 {
    std::iter::once(poly.exterior())
        .chain(poly.interiors())
        .map(|ring| point_to_linestring_c2(q, ring.coords()))
        .fold(f64::INFINITY, f64::min)
}

#[hegel::test(test_cases = 300)]
fn prop_point_in_polygon_matches_cap_oracle(tc: TestCase) {
    let cap = draw_cap(&tc);
    let poly = cap_polygon(&cap);
    let q = draw_coord(&tc); // full sphere, including the >hemisphere fill
    let angle = central_angle_deg(cap.centre, q);
    // Reject the thin band where the m-gon and the true cap boundary differ (chord
    // sagitta), so Hegel replaces the case rather than counting it as a pass.
    if (angle - cap.alpha_deg).abs() < 0.2 {
        tc.reject();
    }
    assert_eq!(
        poly.contains_point(q.to_unit_vector()),
        cap_contains(&cap, q),
        "PIP != cap oracle: centre=({},{}) alpha={} q=({},{}) angle={}",
        cap.centre.lon,
        cap.centre.lat,
        cap.alpha_deg,
        q.lon,
        q.lat,
        angle
    );
}

#[hegel::test(test_cases = 200)]
fn prop_distance_zero_inside_polygon(tc: TestCase) {
    let cap = draw_cap(&tc);
    let poly = cap_polygon(&cap);
    // A query well within the cap (and so within the inscribed m-gon).
    let az = draw_lon(&tc);
    let r = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(0.9 * cap.alpha_deg),
    );
    let q_in = destination(cap.centre, az, r);
    assert_eq!(
        poly.distance_2(&q_in.to_unit_vector()),
        0.0,
        "interior query distance must be zero; centre=({},{}) alpha={} r={}",
        cap.centre.lon,
        cap.centre.lat,
        cap.alpha_deg,
        r
    );
}

#[hegel::test(test_cases = 200)]
fn prop_polygon_box_contains_interior(tc: TestCase) {
    let cap = draw_cap(&tc);
    let poly = cap_polygon(&cap);
    let env = poly.envelope();
    // An interior sample up to near the boundary must embed inside the box: MINDIST
    // containment of the filled region, not just the boundary.
    let az = draw_lon(&tc);
    let r = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(0.95 * cap.alpha_deg),
    );
    let p = destination(cap.centre, az, r).to_unit_vector();
    assert!(
        env.contains_point(&p),
        "box must contain interior sample; centre=({},{}) alpha={} r={}",
        cap.centre.lon,
        cap.centre.lat,
        cap.alpha_deg,
        r
    );
}

#[hegel::test(test_cases = 200)]
fn prop_antipode_of_interior_is_outside(tc: TestCase) {
    // For a sub-hemisphere cap the antipode of any interior point is beyond 90 deg from
    // the centre, hence outside. Catches the antipodal-lobe false positive.
    let cap = Cap {
        centre: coord(
            draw_lon(&tc),
            tc.draw(generators::floats::<f64>().min_value(-85.0).max_value(85.0)),
        ),
        alpha_deg: tc.draw(generators::floats::<f64>().min_value(2.0).max_value(80.0)),
    };
    let poly = cap_polygon(&cap);
    let az = draw_lon(&tc);
    let r = tc.draw(
        generators::floats::<f64>()
            .min_value(0.0)
            .max_value(0.9 * cap.alpha_deg),
    );
    let pv = destination(cap.centre, az, r).to_unit_vector().0;
    let antipode = UnitVec([-pv[0], -pv[1], -pv[2]]);
    assert!(
        !poly.contains_point(antipode),
        "antipode of an interior point must be outside; centre=({},{}) alpha={}",
        cap.centre.lon,
        cap.centre.lat,
        cap.alpha_deg
    );
}

#[hegel::test(test_cases = 50)]
fn prop_polygon_envelope_distance_is_lower_bound(tc: TestCase) {
    let tree = GeodeticRTree::bulk_load(cap_polys(&tc, 15));
    let query = draw_coord(&tc).to_unit_vector();
    min_leaf_distance(tree.root(), query);
}

#[hegel::test(test_cases = 50)]
fn prop_polygon_min_max_dist_2_is_upper_bound(tc: TestCase) {
    let tree = GeodeticRTree::bulk_load(cap_polys(&tc, 15));
    let query = draw_coord(&tc).to_unit_vector();
    check_min_max_dist(tree.root(), query);
}

#[hegel::test(test_cases = 50)]
fn prop_polygon_parent_envelope_contains_descendants(tc: TestCase) {
    let tree = GeodeticRTree::bulk_load(cap_polys(&tc, 15));
    check_envelope_contains_all(tree.root());
}

#[hegel::test(test_cases = 100)]
fn prop_polygon_nn_matches_linear_scan(tc: TestCase) {
    let polys = cap_polys(&tc, 15);
    let query = draw_coord(&tc);
    let tree = GeodeticRTree::bulk_load(polys.clone());
    let (_, tree_metres) = tree
        .nearest_neighbor_with_distance(query)
        .expect("non-empty");
    // Linear scan with the leaf distance: the membership is validated against the cap
    // oracle and the boundary distance is the externally-anchored arc distance, so this
    // checks that branch-and-bound matches the scan (pruning correctness).
    let scan_best = polys
        .iter()
        .map(|p| squared_chord_to_metres(p.distance_2(&query.to_unit_vector())))
        .fold(f64::INFINITY, f64::min);
    let tol = 1e-3 + scan_best * 1e-9;
    assert!(
        (tree_metres - scan_best).abs() <= tol,
        "polygon tree NN {tree_metres} != linear-scan best {scan_best}"
    );
}

#[hegel::test(test_cases = 300)]
fn prop_polygon_boundary_distance_matches_oracle(tc: TestCase) {
    let cap = draw_cap(&tc);
    let poly = cap_polygon(&cap);
    // Draw the query strictly outside the cap (more than alpha from the centre). The m-gon is
    // inscribed in the cap, so such a query is outside the polygon too: every case exercises
    // the boundary distance, rather than mostly landing in the up-to->hemisphere interior a
    // full-sphere draw would. Interior queries are covered by prop_distance_zero_inside_polygon.
    let az = draw_lon(&tc);
    let angle = tc.draw(
        generators::floats::<f64>()
            .min_value(cap.alpha_deg)
            .exclude_min(true)
            .max_value(180.0),
    );
    let q = destination(cap.centre, az, angle);
    let qv = q.to_unit_vector();
    // The external distance is the distance to the boundary, which the oracle computes from
    // the validated arc ternary search (nearest_arc_c2) rather than the polygon's own
    // arc_distance_2. prop_polygon_nn only checks pruning (distance_2 on both sides); this
    // anchors the value itself.
    let impl_c2 = poly.distance_2(&qv);
    let oracle_c2 = polygon_boundary_oracle_c2(q, &poly);
    let tol = 1e-7 + oracle_c2 * 1e-9;
    assert!(
        (impl_c2 - oracle_c2).abs() <= tol,
        "polygon boundary c2 {impl_c2} != oracle {oracle_c2}; centre=({},{}) alpha={} q=({},{})",
        cap.centre.lon,
        cap.centre.lat,
        cap.alpha_deg,
        q.lon,
        q.lat
    );
}

#[test]
fn polygon_boundary_distance_matches_cap_geometry() {
    // A query radially outward from vertex 0 of the cap m-gon (cap_ring places vertex 0 at
    // azimuth 0 from the centre) has that vertex as its nearest boundary point, so the
    // great-circle distance is exactly R*(theta - alpha) by elementary spherical geometry --
    // a check that touches none of the arc machinery.
    let cap = Cap {
        centre: coord(20.0, 30.0),
        alpha_deg: 25.0,
    };
    let poly = cap_polygon(&cap);
    for extra in [5.0, 20.0, 60.0] {
        let theta = cap.alpha_deg + extra;
        let q = destination(cap.centre, 0.0, theta);
        let metres = squared_chord_to_metres(poly.distance_2(&q.to_unit_vector()));
        let expected = extra.to_radians() * EARTH_RADIUS_METRES;
        assert!(
            (metres - expected).abs() < 1.0,
            "vertex-aligned boundary distance {metres} m != R*(theta-alpha) {expected} m (extra={extra})"
        );
    }
}

#[test]
fn e3_pole_polygon_box_reaches_pole_and_query_at_pole_is_zero() {
    // A deterministic ring at lat 80 traversed eastward (northern region interior): a cap
    // enclosing the north pole.
    let mut ring: Vec<GeodeticCoord> = (0..64)
        .map(|i| coord(-180.0 + 360.0 * (i as f64) / 64.0, 80.0))
        .collect();
    ring.push(ring[0]);
    let poly =
        GeodeticPolygon::try_new(GeodeticRing::try_from_lonlat(ring).unwrap(), Vec::new()).unwrap();
    let pole = UnitVec([0.0, 0.0, 1.0]);
    assert!(
        poly.envelope().contains_point(&pole),
        "box reaches the pole"
    );
    assert_eq!(poly.distance_2(&pole), 0.0, "the pole is interior");
}

#[test]
fn e2_antimeridian_polygon_membership_needs_no_wrapping() {
    // A cap centred on the antimeridian: the seam is an ordinary interior region.
    let cap = Cap {
        centre: coord(180.0, 0.0),
        alpha_deg: 20.0,
    };
    let poly = cap_polygon(&cap);
    assert!(
        poly.contains_point(coord(180.0, 0.0).to_unit_vector()),
        "the centre on the seam is inside"
    );
    assert!(
        poly.contains_point(coord(-175.0, 0.0).to_unit_vector()),
        "175W (5 deg east of the seam) is inside"
    );
    assert!(
        poly.contains_point(coord(175.0, 0.0).to_unit_vector()),
        "175E (5 deg west of the seam) is inside"
    );
    assert!(
        !poly.contains_point(coord(0.0, 0.0).to_unit_vector()),
        "lon 0 (the far side) is outside"
    );
}

#[test]
fn polygon_with_hole_excludes_the_hole() {
    // An annulus about (0, 0): an alpha=40 exterior cap with an alpha=10 cap punched out.
    let centre = coord(0.0, 0.0);
    let outer = GeodeticRing::try_from_lonlat(cap_ring(
        &Cap {
            centre,
            alpha_deg: 40.0,
        },
        64,
    ))
    .unwrap();
    // A hole, supplied clockwise (reversed) per OGC; the crossing-parity test is in any
    // case orientation-agnostic for holes.
    let mut hole_pts = cap_ring(
        &Cap {
            centre,
            alpha_deg: 10.0,
        },
        64,
    );
    hole_pts.reverse();
    let hole = GeodeticRing::try_from_lonlat(hole_pts).unwrap();
    let poly = GeodeticPolygon::try_new(outer, vec![hole]).unwrap();

    // The centre lies in the hole -> outside the filled polygon, with a positive distance.
    let centre_v = centre.to_unit_vector();
    assert!(
        !poly.contains_point(centre_v),
        "the hole interior is outside the filled polygon"
    );
    assert!(poly.distance_2(&centre_v) > 0.0, "distance into the hole");
    // A point in the annulus (25 deg from the centre) is inside.
    assert!(
        poly.contains_point(destination(centre, 0.0, 25.0).to_unit_vector()),
        "the annulus is inside"
    );
    // A point beyond the exterior (50 deg) is outside.
    assert!(
        !poly.contains_point(destination(centre, 0.0, 50.0).to_unit_vector()),
        "beyond the exterior is outside"
    );
}
