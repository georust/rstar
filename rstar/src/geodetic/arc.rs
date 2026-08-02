//! Great-circle arc primitives for geodetic extent geometry (linestrings and
//! polygons): the arc-aware bounding box, the on-arc predicate, the nearest point
//! on an arc, and the point-to-arc squared-chord distance.
//!
//! All maths is in the unit-sphere embedding ([`UnitVec`]) and reuses
//! [`squared_chord`] as the metric, so an extent leaf's `distance_2` and its
//! `AABB<UnitVec>` envelope share units and the pruning lower bound holds (see the
//! module docs).
//!
//! # The u,v frame
//!
//! Both the bounding box and the point-to-arc distance parametrise the shorter
//! great-circle arc `A -> B` in the orthonormal frame `u = A`,
//! `v = normalise(B - (A . B) A)`: the arc is `{ cos(t) u + sin(t) v : t in
//! [0, theta] }` with `theta` the central angle, so `P(0) = A` and `P(theta) = B`.
//! This frame is well conditioned for near-antipodal arcs, where the plane normal
//! `A x B` is formed by catastrophic cancellation and a box built from it can fail
//! to contain the arc. Near-antipodal edges (`squared_chord >= ANTIPODAL_C2`) are
//! rejected at construction; the residual error inside that bound stays below
//! [`ARC_MARGIN`].
//!
//! # Preconditions
//!
//! Each arc must span `< 180` degrees (the shorter great-circle arc is otherwise
//! undefined). Constructors enforce this; the primitives here assume it and treat a
//! coincident edge (`squared_chord <= DUP_C2`) as the endpoint box / endpoint
//! distance.

// `Float` provides `sqrt`, `acos`, `atan2` on `f64` in `no_std` builds, where the
// inherent methods are unavailable. Under `cfg(test)` the inherent methods win,
// leaving this unused (the same pattern as `distance.rs`/`embedding.rs`).
#[allow(unused_imports)]
use num_traits::Float;

use crate::AABB;

use super::embedding::{squared_chord, UnitVec};

/// Outward margin added to every box face so containment is robust to a final-ulp
/// rounding at a face. Matches the `rectangle_bounding_box` idiom (about 6 micrometres
/// on the sphere), far below any meaningful separation.
const ARC_MARGIN: f64 = 1e-12;

/// Squared chord at or below which the two endpoints are treated as coincident: the
/// arc collapses to a point, the box is the endpoint box, and the turning-point step
/// is skipped (it would divide by a near-zero frame length).
const DUP_C2: f64 = 1e-24;

/// Squared chord at or above which an edge spans `>= ~179.96` degrees and is rejected
/// at construction (the shorter great-circle arc is undefined and the u,v frame is
/// ill-conditioned). Chosen so the residual no-margin box undercoverage for a still-
/// valid edge stays below [`ARC_MARGIN`]; pinned by `prop_arc_box_contains_arc_near_180`.
/// Consumed by linestring/ring construction, which rejects such edges.
pub(crate) const ANTIPODAL_C2: f64 = 4.0 - 4.5e-7;

/// The axis-aligned bounding box, in the unit-sphere embedding, of the shorter
/// great-circle arc between `a` and `b`.
///
/// `a` and `b` are points on the unit sphere; embed a
/// [`GeodeticCoord`](crate::geodetic::GeodeticCoord) with `UnitVec::from(coord)` or
/// [`to_unit_vector`](crate::geodetic::GeodeticCoord::to_unit_vector). The returned
/// [`AABB<UnitVec>`](crate::AABB) encloses the **whole arc**, not just its endpoints: a
/// great-circle edge bulges away from the straight chord between its endpoints, and the
/// box covers that bulge – including the extreme-latitude turning point when the arc
/// passes over one. A small outward margin is added so the box still contains the arc
/// under floating-point rounding. Coincident points give the degenerate box at that
/// point.
///
/// Use this to build a custom leaf's [`RTreeObject::envelope`](crate::RTreeObject::envelope):
/// take one box per edge and [`merge`](crate::Envelope::merge) them. See
/// [`GeodeticObject`](crate::geodetic::GeodeticObject) for a worked example.
///
/// # Preconditions
///
/// `a` and `b` must be less than 180 degrees apart, so that the *shorter* of the two
/// great-circle arcs through them is well defined; the box is meaningless for a
/// near-antipodal pair. The built-in
/// [`GeodeticLineString`](crate::geodetic::GeodeticLineString) and
/// [`GeodeticPolygon`](crate::geodetic::GeodeticPolygon) reject such edges when they are
/// constructed, so a direct caller is responsible for the same.
pub fn arc_bounding_box(a: UnitVec, b: UnitVec) -> AABB<UnitVec> {
    let mut lo = [a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])];
    let mut hi = [a.0[0].max(b.0[0]), a.0[1].max(b.0[1]), a.0[2].max(b.0[2])];

    if squared_chord(a, b) > DUP_C2 {
        if let Some((u, v)) = arc_frame(a, b) {
            let theta = dot(u, b.0).clamp(-1.0, 1.0).acos();
            for i in 0..3 {
                let r = (u[i] * u[i] + v[i] * v[i]).sqrt();
                if r > 0.0 {
                    // P_i(t) = u_i cos t + v_i sin t = r cos(t - phi); the maximum +r
                    // is reached at t = phi, the minimum -r at t = phi + pi.
                    let phi = v[i].atan2(u[i]);
                    if on_arc_param(phi, theta) {
                        hi[i] = hi[i].max(r);
                    }
                    if on_arc_param(phi + core::f64::consts::PI, theta) {
                        lo[i] = lo[i].min(-r);
                    }
                }
            }
        }
    }

    AABB::from_corners(
        UnitVec([lo[0] - ARC_MARGIN, lo[1] - ARC_MARGIN, lo[2] - ARC_MARGIN]),
        UnitVec([hi[0] + ARC_MARGIN, hi[1] + ARC_MARGIN, hi[2] + ARC_MARGIN]),
    )
}

/// True iff the point `m` lies on the shorter great-circle arc between `a` and `b`.
///
/// All three are points on the unit sphere (see [`arc_bounding_box`] for how to embed a
/// lon/lat coordinate), and `a` and `b` must be less than 180 degrees apart. `m` is
/// assumed already to lie on the great circle through `a` and `b`; the test is invariant
/// to positive scaling of `m`, so an unnormalised in-plane direction may be passed.
///
/// With the plane normal `N = cross(A, B)` (oriented so the turn `A -> B` is positive),
/// `m` is on the short arc iff the turns `A -> m` and `m -> B` share that orientation:
/// `(A x m) . N >= 0` and `(m x B) . N >= 0`. Because the arc spans `< 180` degrees this
/// is unambiguous – a point on the complementary long arc fails at least one test.
pub fn arc_contains_point(a: UnitVec, b: UnitVec, m: UnitVec) -> bool {
    let n = cross(a.0, b.0);
    dot(cross(a.0, m.0), n) >= 0.0 && dot(cross(m.0, b.0), n) >= 0.0
}

/// The point of the shorter great-circle arc between `a` and `b` that is nearest to
/// `query`: the foot of the perpendicular when it lies on the arc, otherwise the nearer
/// endpoint.
///
/// All three points are on the unit sphere (see [`arc_bounding_box`] for how to embed a
/// lon/lat coordinate), and `a` and `b` must be less than 180 degrees apart. This returns
/// the nearest *point*; to order or prune a custom leaf use [`arc_distance_2`], which
/// returns the squared-chord distance to that point directly.
pub fn nearest_point_on_arc(a: UnitVec, b: UnitVec, query: UnitVec) -> UnitVec {
    let nearer = if squared_chord(query, a) <= squared_chord(query, b) {
        a
    } else {
        b
    };
    if squared_chord(a, b) <= DUP_C2 {
        return nearer;
    }
    let (u, v) = match arc_frame(a, b) {
        Some(f) => f,
        None => return nearer,
    };
    let n = cross(u, v);
    let d = dot(query.0, n);
    let proj = sub(query.0, scale(n, d));
    let pn = norm(proj);
    if pn <= 0.0 {
        return nearer;
    }
    let foot = UnitVec(scale(proj, 1.0 / pn));
    if arc_contains_point(a, b, foot) {
        return foot;
    }
    let opposite = UnitVec(scale(proj, -1.0 / pn));
    if arc_contains_point(a, b, opposite) {
        return opposite;
    }
    nearer
}

/// Squared-chord distance (in `[0, 4]`) from `query` to the nearest point of the shorter
/// great-circle arc between `a` and `b`.
///
/// All three points are on the unit sphere; embed a
/// [`GeodeticCoord`](crate::geodetic::GeodeticCoord) with `UnitVec::from(coord)` or
/// [`to_unit_vector`](crate::geodetic::GeodeticCoord::to_unit_vector). The result is in
/// the same squared-chord metric the [`AABB<UnitVec>`](crate::AABB) envelope uses (not
/// metres; convert with [`squared_chord_to_metres`](crate::geodetic::squared_chord_to_metres)),
/// so a custom leaf's [`PointDistance::distance_2`](crate::PointDistance::distance_2) can
/// fold it (taking the minimum) over the leaf's edges. See
/// [`GeodeticObject`](crate::geodetic::GeodeticObject) for a worked example. As with
/// [`arc_bounding_box`], `a` and `b` must be less than 180 degrees apart; coincident
/// endpoints give the distance to that point.
///
/// # Derivation
///
/// With `n` the unit normal of the arc plane and `d = query . n`, the nearest and
/// farthest points of the arc's *great circle* are `+-Pc`, `Pc = normalise(query - d n)`.
/// Then `query . Pc = (1 - d^2) / sqrt(1 - d^2) = sqrt(1 - d^2)`, so the squared chords to
/// them are `2 -/+ 2*sqrt(1 - d^2)`. Two rewrites keep this exact where it matters:
///
/// - `root = sqrt(1 - d^2)` is taken as `|query x n|` (the same value), sidestepping the
///   cancellation in `1 - d*d` when `query` nears the arc's pole-of-circle (`d -> 1`).
/// - the near value `2 - 2*root` is written `2*d^2 / (1 + root)` (since `1 - root^2 =
///   d^2`), so a query close to the arc (`root -> 1`) gives `~ d^2` directly instead of
///   cancelling `2 - 2*root` to a spurious zero.
///
/// The in-plane projection `query - d n` is formed only to sign the on-arc test, so its
/// own cancellation never reaches the returned value. A coincident edge returns the
/// endpoint distance; an off-arc foot falls back to the nearer endpoint.
pub fn arc_distance_2(a: UnitVec, b: UnitVec, query: UnitVec) -> f64 {
    let endpoint_min = squared_chord(query, a).min(squared_chord(query, b));
    if squared_chord(a, b) <= DUP_C2 {
        return endpoint_min;
    }
    let (u, v) = match arc_frame(a, b) {
        Some(f) => f,
        None => return endpoint_min,
    };
    let n = cross(u, v);
    let d = dot(query.0, n);
    // In-plane direction of `query`; only its sign feeds the on-arc test, so it need
    // not be normalised (and is not, to keep the cancellation out of the value).
    let proj = sub(query.0, scale(n, d));
    // root = sqrt(1 - d^2), computed as |query x n| to avoid the cancellation in
    // `1 - d*d` near the arc's pole-of-circle (d ~ 1).
    let root = norm(cross(query.0, n));

    let mut best = endpoint_min;
    if arc_contains_point(a, b, UnitVec(proj)) {
        // Nearest great-circle point: 2 - 2*root, rearranged to 2*d^2/(1 + root) so it
        // stays well conditioned as the distance shrinks (root -> 1) instead of
        // cancelling to a spurious zero.
        best = best.min(2.0 * d * d / (1.0 + root));
    }
    if arc_contains_point(a, b, UnitVec(scale(proj, -1.0))) {
        // Farthest great-circle point: 2 + 2*root needs no rearrangement (root >= 0).
        best = best.min(2.0 + 2.0 * root);
    }
    best
}

// --- vector-algebra helpers on raw `[f64; 3]` (results are not unit length) ---

pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Orthonormal frame `(u, v)` of the shorter great-circle arc `A -> B`: `u = A` and
/// `v` is the unit vector in the arc plane orthogonal to `u`, towards `B`. Returns
/// `None` when the frame is undefined (`A ~ B`, or a near-antipodal edge that should
/// have been rejected at construction), so callers fall back to the endpoint result.
fn arc_frame(a: UnitVec, b: UnitVec) -> Option<([f64; 3], [f64; 3])> {
    let u = a.0;
    let cos_theta = dot(u, b.0);
    let w = sub(b.0, scale(u, cos_theta));
    let wn = norm(w);
    if wn <= 0.0 {
        return None;
    }
    Some((u, scale(w, 1.0 / wn)))
}

/// True iff the turning-point parameter `t` (in radians) lies on the arc `[0, theta]`,
/// after normalising it into `[0, 2*pi)`. Since `theta < pi`, membership is `t <= theta`.
fn on_arc_param(t: f64, theta: f64) -> bool {
    let two_pi = 2.0 * core::f64::consts::PI;
    let mut tn = t % two_pi;
    if tn < 0.0 {
        tn += two_pi;
    }
    tn <= theta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geodetic::GeodeticCoord;
    use crate::Envelope;

    fn uv(lon: f64, lat: f64) -> UnitVec {
        GeodeticCoord { lon, lat }.to_unit_vector()
    }

    // --- arc_bounding_box ---

    #[test]
    fn meridian_arc_reaches_x_one_at_endpoint() {
        // Arc up the prime meridian from the equator: x peaks at 1 at (0, 0) = A.
        let bbox = arc_bounding_box(uv(0.0, 0.0), uv(0.0, 80.0));
        assert!((bbox.upper().0[0] - 1.0).abs() < 1e-9, "x should reach 1");
    }

    #[test]
    fn equatorial_arc_collapses_z() {
        // An equatorial arc lies in the plane z = 0; its z range is the endpoints,
        // both ~0, so the box is a thin slab in z (only the outward margin).
        let bbox = arc_bounding_box(uv(10.0, 0.0), uv(40.0, 0.0));
        assert!(bbox.upper().0[2].abs() < 1e-6, "z should not bulge");
        assert!(bbox.lower().0[2].abs() < 1e-6, "z should not bulge");
    }

    #[test]
    fn over_pole_arc_reaches_pole() {
        // The shorter arc (0, 80) -> (180, 80) passes over the north pole, so the box
        // reaches z = 1 although neither vertex does.
        let bbox = arc_bounding_box(uv(0.0, 80.0), uv(180.0, 80.0));
        assert!(
            (bbox.upper().0[2] - 1.0).abs() < 1e-9,
            "z should reach the pole"
        );
    }

    #[test]
    fn antimeridian_arc_built_without_special_case() {
        // An arc straddling the seam is just two ordinary unit vectors; the box
        // contains both endpoints with no wrapping logic.
        let a = uv(170.0, 0.0);
        let b = uv(-170.0, 0.0);
        let bbox = arc_bounding_box(a, b);
        assert!(bbox.contains_point(&a));
        assert!(bbox.contains_point(&b));
    }

    #[test]
    fn duplicate_vertex_arc_is_endpoint_box() {
        let a = uv(10.0, 20.0);
        let bbox = arc_bounding_box(a, a);
        assert!(bbox.contains_point(&a));
        // No bulge: each face is the point coordinate plus/minus the margin.
        for i in 0..3 {
            assert!((bbox.upper().0[i] - a.0[i]).abs() < 1e-9);
            assert!((bbox.lower().0[i] - a.0[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn arc_contains_point_endpoints() {
        let a = uv(0.0, 0.0);
        let b = uv(0.0, 80.0);
        assert!(arc_contains_point(a, b, a));
        assert!(arc_contains_point(a, b, b));
    }

    // --- arc_distance_2 ---

    #[test]
    fn query_on_arc_is_zero() {
        let a = uv(0.0, 0.0);
        let b = uv(0.0, 80.0);
        let mid = uv(0.0, 40.0);
        assert!(arc_distance_2(a, b, mid) < 1e-12, "midpoint is on the arc");
    }

    #[test]
    fn north_pole_to_equatorial_arc_is_two() {
        // The north pole is the pole-of-circle of any equatorial arc: every point of
        // that great circle is 90 degrees away, squared chord 2.
        let a = uv(10.0, 0.0);
        let b = uv(40.0, 0.0);
        let pole = UnitVec([0.0, 0.0, 1.0]);
        assert!((arc_distance_2(a, b, pole) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn foot_beyond_endpoint_picks_nearer_endpoint() {
        // A query well past B along the meridian: nearest arc point is B.
        let a = uv(0.0, 0.0);
        let b = uv(0.0, 40.0);
        let q = uv(0.0, 70.0);
        let expected = squared_chord(q, b);
        assert!((arc_distance_2(a, b, q) - expected).abs() < 1e-12);
    }

    #[test]
    fn opposite_side_foot_is_found() {
        // A query roughly opposite the arc's near side: the far stationary point -Pc
        // is the on-arc nearest, found via its own on-arc test.
        let a = uv(-30.0, 0.0);
        let b = uv(30.0, 0.0);
        let q = uv(0.0, -10.0);
        // Brute-force minimum over a dense sample of the arc.
        let mut brute = f64::INFINITY;
        for k in 0..=2000 {
            let lon = -30.0 + 60.0 * (k as f64) / 2000.0;
            brute = brute.min(squared_chord(q, uv(lon, 0.0)));
        }
        assert!((arc_distance_2(a, b, q) - brute).abs() < 1e-6);
    }

    #[test]
    fn duplicate_edge_returns_endpoint_distance() {
        let a = uv(10.0, 20.0);
        let q = uv(15.0, 25.0);
        assert!((arc_distance_2(a, a, q) - squared_chord(q, a)).abs() < 1e-12);
    }

    // --- nearest_point_on_arc ---

    #[test]
    fn nearest_point_on_arc_returns_foot_when_on_arc() {
        let a = uv(0.0, 0.0);
        let b = uv(0.0, 80.0);
        let q = uv(5.0, 40.0);
        let p = nearest_point_on_arc(a, b, q);
        // The foot is on the meridian (lon ~ 0), near lat 40.
        let back = GeodeticCoord::from_unit_vector(p);
        assert!(back.lon.abs() < 1e-6, "foot should sit on the meridian");
        assert!((back.lat - 40.0).abs() < 1.0);
    }

    #[test]
    fn nearest_point_on_arc_returns_endpoint_when_off_arc() {
        let a = uv(0.0, 0.0);
        let b = uv(0.0, 40.0);
        let q = uv(0.0, 70.0);
        assert_eq!(nearest_point_on_arc(a, b, q), b);
    }
}
