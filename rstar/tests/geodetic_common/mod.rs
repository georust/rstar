//! Shared generators, the externally-validated arc oracle, and tree-walk helpers for the
//! geodetic extent-geometry integration tests.
//!
//! Included via `mod geodetic_common;` from each `tests/geodetic_*.rs`. Cargo compiles
//! every top-level `tests/*.rs` as its own test binary but not a subdirectory module, so
//! this file is shared scaffolding, not a test target.
//!
//! # Verification architecture
//!
//! Correctness rests on three layers, each independent of the one it checks:
//!
//! 1. **Implementation under test** – the embedding's analytic maths (`arc_distance_2`
//!    and the leaf `distance_2`s), via the well-conditioned u,v frame.
//! 2. **In-test oracle** – `nearest_arc_c2` computes the same point-to-arc squared chord a
//!    different way: a ternary search over a spherical-linear-interpolation (`slerp`,
//!    defined below) parametrisation of the arc. It shares no code with the analytic path,
//!    so a bug in one is unlikely to be mirrored in the other; the property tests compare
//!    the two over the whole sphere.
//! 3. **External reference** – `ARC_GOLDEN` (in `geodetic_arc_property.rs`) pins *both* the
//!    implementation and `nearest_arc_c2` to ground truth from
//!    `tests/reference/arc_distance_reference.py` (60-digit `mpmath` by two independent
//!    methods, cross-checked against `s2sphere`). This is what lets the property tests
//!    trust the oracle: were it to drift from truth, the golden table would catch it.
//!
//! The arc oracle is the building block for the higher-level oracles:
//!
//! - `point_to_linestring_c2` is the minimum of `nearest_arc_c2` over a polyline's edges —
//!   the linestring distance oracle, and the per-ring core of the polygon boundary oracle
//!   in `geodetic_extent_property.rs`.
//! - `Cap` is an *exact* spherical cap with a closed-form membership test (`cap_contains`)
//!   at every angular radius, so it anchors point-in-polygon independently of the ray-cast
//!   implementation – including caps larger than a hemisphere, the regime that test exists
//!   to cover.
//!
//! (The WGS84 refine has its own anchor: its spherical/geodesic margin is checked against
//! `geographiclib-rs` in `src/geodetic/spheroid.rs`.)

#![allow(dead_code)] // each test binary uses a different subset of these helpers

use hegel::generators;
use hegel::TestCase;

use rstar::geodetic::{squared_chord, GeodeticCoord, GeodeticObject, UnitVec};
use rstar::{Envelope, ParentNode, RTreeNode};

// --- base draw helpers ---

pub fn coord(lon: f64, lat: f64) -> GeodeticCoord {
    GeodeticCoord { lon, lat }
}

pub fn draw_lon(tc: &TestCase) -> f64 {
    tc.draw(
        generators::floats::<f64>()
            .min_value(-180.0)
            .max_value(180.0),
    )
}

pub fn draw_lat(tc: &TestCase) -> f64 {
    tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0))
}

pub fn draw_coord(tc: &TestCase) -> GeodeticCoord {
    coord(draw_lon(tc), draw_lat(tc))
}

// --- raw vector helpers (for the pole-of-circle bias and slerp) ---

fn lcross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn ldot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn lnorm(a: [f64; 3]) -> f64 {
    ldot(a, a).sqrt()
}

// --- geometric helpers (lon/lat space) ---

/// Point at angular distance `dist_deg` and initial bearing `az_deg` from `start`
/// (the spherical direct geodesic). Used by every generator to place a vertex a bounded
/// step from the previous one, so each edge spans `< 180` degrees by construction.
pub fn destination(start: GeodeticCoord, az_deg: f64, dist_deg: f64) -> GeodeticCoord {
    let (phi1, lam1) = (start.lat.to_radians(), start.lon.to_radians());
    let (theta, delta) = (az_deg.to_radians(), dist_deg.to_radians());
    let phi2 = (phi1.sin() * delta.cos() + phi1.cos() * delta.sin() * theta.cos()).asin();
    let lam2 = lam1
        + (theta.sin() * delta.sin() * phi1.cos()).atan2(delta.cos() - phi1.sin() * phi2.sin());
    let mut lon = lam2.to_degrees();
    lon = ((lon + 540.0) % 360.0) - 180.0;
    coord(lon, phi2.to_degrees().clamp(-90.0, 90.0))
}

/// Spherical linear interpolation: `slerp(a, b, t)` is the point a fraction `t` of the
/// way along the great-circle arc from `a` to `b` (`t = 0` gives `a`, `t = 1` gives `b`),
/// moving at constant angular speed. It is a test-only helper (no counterpart in the
/// crate, which parametrises arcs by the u,v frame instead).
///
/// The standard `(sin((1 - t) w) a + sin(t w) b) / sin(w)` form (with `w` the central
/// angle) is renormalised to unit length so near-antipodal samples (where the blend
/// magnitude is tiny) still lie exactly on the sphere, and a near-coincident arc still
/// distinguishes the two endpoints.
pub fn slerp(a: UnitVec, b: UnitVec, t: f64) -> UnitVec {
    let d = (a.0[0] * b.0[0] + a.0[1] * b.0[1] + a.0[2] * b.0[2]).clamp(-1.0, 1.0);
    let omega = d.acos();
    let (w0, w1) = if omega < 1e-6 {
        (1.0 - t, t)
    } else {
        let s = omega.sin();
        (((1.0 - t) * omega).sin() / s, (t * omega).sin() / s)
    };
    let v = [
        w0 * a.0[0] + w1 * b.0[0],
        w0 * a.0[1] + w1 * b.0[1],
        w0 * a.0[2] + w1 * b.0[2],
    ];
    let n = lnorm(v);
    UnitVec([v[0] / n, v[1] / n, v[2] / n])
}

/// `n + 1` unit vectors densely sampling the arc `a -> b`, endpoints included.
pub fn sample_arc(a: GeodeticCoord, b: GeodeticCoord, n: usize) -> Vec<UnitVec> {
    let (av, bv) = (a.to_unit_vector(), b.to_unit_vector());
    (0..=n)
        .map(|i| slerp(av, bv, i as f64 / n as f64))
        .collect()
}

// --- the externally-validated arc oracle ---

/// Minimum squared chord from `q` to the shorter arc `a -> b`, via a ternary search over
/// `slerp`. Independent of the embedding's u,v-frame analytic path under test and
/// anchored to an external reference by `ARC_GOLDEN` in `geodetic_arc_property.rs`. The
/// per-edge building block of the linestring and polygon distance oracles.
pub fn nearest_arc_c2(q: GeodeticCoord, a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    let qv = q.to_unit_vector();
    let (av, bv) = (a.to_unit_vector(), b.to_unit_vector());
    let c2_at = |s: f64| squared_chord(qv, slerp(av, bv, s));

    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..100 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        if c2_at(m1) < c2_at(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    c2_at(0.5 * (lo + hi))
}

/// Minimum squared chord from `q` to a polyline, the minimum over its edges of the
/// validated [`nearest_arc_c2`]. The independent point-to-linestring oracle.
pub fn point_to_linestring_c2(q: GeodeticCoord, verts: &[GeodeticCoord]) -> f64 {
    verts
        .windows(2)
        .map(|edge| nearest_arc_c2(q, edge[0], edge[1]))
        .fold(f64::INFINITY, f64::min)
}

// --- generators ---

/// A great-circle arc whose central angle is `< 180` degrees by construction (a geodesic
/// step of `gamma in [0, 179]` from a freely-drawn first endpoint).
pub fn draw_arc(tc: &TestCase) -> (GeodeticCoord, GeodeticCoord) {
    let a = draw_coord(tc);
    let gamma = tc.draw(generators::floats::<f64>().min_value(0.0).max_value(179.0));
    let az = tc.draw(
        generators::floats::<f64>()
            .min_value(-180.0)
            .max_value(180.0),
    );
    (a, destination(a, az, gamma))
}

/// A near-antipodal arc in the thin shell just inside the antipodal reject cutoff
/// (`squared_chord` in `[4 - 1e-6, 4 - 5e-7)`, about `179.94` degrees), where the
/// cross-product normal is worst conditioned and the random generator almost never lands.
pub fn draw_arc_near_180(tc: &TestCase) -> (GeodeticCoord, GeodeticCoord) {
    let a = draw_coord(tc);
    let gap = tc.draw(generators::floats::<f64>().min_value(5e-7).max_value(1e-6));
    let gamma = (-1.0 + gap / 2.0).clamp(-1.0, 1.0).acos().to_degrees();
    let az = tc.draw(
        generators::floats::<f64>()
            .min_value(-180.0)
            .max_value(180.0),
    );
    (a, destination(a, az, gamma))
}

/// A query for the point-to-arc property: half the time an unbiased coordinate, half the
/// time within `~1e-9..1e-6` radians of the arc's pole-of-circle, the regime that exposes
/// the projection cancellation the analytic distance guards against.
pub fn draw_query_for_arc(tc: &TestCase, a: GeodeticCoord, b: GeodeticCoord) -> GeodeticCoord {
    let biased = tc.draw(generators::booleans());
    if biased {
        let n = lcross(a.to_unit_vector().0, b.to_unit_vector().0);
        let nn = lnorm(n);
        if nn > 1e-6 {
            let pole = GeodeticCoord::from_unit_vector(UnitVec([n[0] / nn, n[1] / nn, n[2] / nn]));
            let off_rad = tc.draw(generators::floats::<f64>().min_value(1e-9).max_value(1e-6));
            let az = tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            );
            return destination(pole, az, off_rad.to_degrees());
        }
    }
    draw_coord(tc)
}

/// A linestring of `2..=6` vertices, each edge `< 180` degrees by construction: every
/// vertex is a bounded geodesic step from the previous one, with an unconstrained azimuth
/// so seam- and pole-crossing chains arise.
pub fn draw_linestring(tc: &TestCase) -> Vec<GeodeticCoord> {
    let n = tc.draw(generators::integers::<usize>().min_value(2).max_value(6));
    let mut verts = Vec::with_capacity(n);
    verts.push(draw_coord(tc));
    for _ in 1..n {
        let prev = *verts.last().unwrap();
        let step = tc.draw(generators::floats::<f64>().min_value(0.0).max_value(120.0));
        let az = tc.draw(
            generators::floats::<f64>()
                .min_value(-180.0)
                .max_value(180.0),
        );
        verts.push(destination(prev, az, step));
    }
    verts
}

// --- spherical caps (an exact point-in-polygon oracle at every size) ---

/// A spherical cap: every point within `alpha_deg` great-circle degrees of `centre`. The
/// cap is the polygon-like region with an *exact* membership test, so it backs the
/// point-in-polygon property at all sizes – including `alpha > 90` (larger than a
/// hemisphere), which the ray-cast membership test exists to support.
pub struct Cap {
    pub centre: GeodeticCoord,
    pub alpha_deg: f64,
}

/// Draws a cap with a full-sphere centre and an angular radius up to `170` degrees (so
/// `alpha > 90` and the consequent >hemisphere fills are well represented).
pub fn draw_cap(tc: &TestCase) -> Cap {
    // Keep the centre off the poles: `destination` (and so `cap_ring`) is unstable when
    // building a circle around a pole. Pole-enclosing caps are exercised by the
    // deterministic E3 edge case instead.
    let centre = coord(
        draw_lon(tc),
        tc.draw(generators::floats::<f64>().min_value(-85.0).max_value(85.0)),
    );
    Cap {
        centre,
        alpha_deg: tc.draw(generators::floats::<f64>().min_value(2.0).max_value(170.0)),
    }
}

/// A closed `m`-vertex ring approximating the cap boundary, traced **counter-clockwise
/// around the centre** (decreasing azimuth) so the centre-cap is the interior under the
/// left-hand-rule orientation convention, for any `alpha`.
pub fn cap_ring(cap: &Cap, m: usize) -> Vec<GeodeticCoord> {
    let mut ring: Vec<GeodeticCoord> = (0..m)
        .map(|i| destination(cap.centre, -360.0 * (i as f64) / (m as f64), cap.alpha_deg))
        .collect();
    ring.push(ring[0]);
    ring
}

/// Great-circle angle between two coordinates, in degrees (well conditioned via atan2).
pub fn central_angle_deg(a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    let (av, bv) = (a.to_unit_vector().0, b.to_unit_vector().0);
    lnorm(lcross(av, bv)).atan2(ldot(av, bv)).to_degrees()
}

/// Exact cap membership: `q` is inside iff it is within `alpha` of the centre.
pub fn cap_contains(cap: &Cap, q: GeodeticCoord) -> bool {
    central_angle_deg(cap.centre, q) <= cap.alpha_deg
}

// --- tree-walk helpers (generic over the leaf type) ---

/// Returns the minimum leaf `distance_2` under `node` and asserts the MINDIST invariant
/// at every node: `env.distance_2(q) <= min contained leaf distance + 1e-12`.
pub fn min_leaf_distance<G: GeodeticObject>(node: &ParentNode<G>, query: UnitVec) -> f64 {
    let min_leaf = node
        .children()
        .iter()
        .map(|child| match child {
            RTreeNode::Leaf(g) => g.distance_2(&query),
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

/// Returns the minimum leaf `distance_2` under `node` and asserts the MINMAXDIST upper
/// bound at every node: `min_max_dist_2(q) + 1e-9 >= min contained leaf distance`.
pub fn check_min_max_dist<G: GeodeticObject>(node: &ParentNode<G>, query: UnitVec) -> f64 {
    let min_leaf = node
        .children()
        .iter()
        .map(|child| match child {
            RTreeNode::Leaf(g) => g.distance_2(&query),
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

/// Asserts the structural invariant that every parent envelope contains all descendants.
pub fn check_envelope_contains_all<G: GeodeticObject>(node: &ParentNode<G>) {
    let env = node.envelope();
    for child in node.children() {
        match child {
            RTreeNode::Leaf(g) => assert!(
                env.contains_envelope(&g.envelope()),
                "parent envelope does not contain a leaf envelope"
            ),
            RTreeNode::Parent(parent) => {
                assert!(
                    env.contains_envelope(&parent.envelope()),
                    "parent envelope does not contain a child envelope"
                );
                check_envelope_contains_all(parent);
            }
        }
    }
}
