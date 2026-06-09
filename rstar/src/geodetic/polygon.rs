//! [`GeodeticPolygon`] and [`GeodeticRing`]: filled spherical-polygon leaves for the
//! geodetic R-tree.
//!
//! Point-in-polygon is a great-circle ray cast: from a reference point known to be
//! inside the filled region (just inside the first exterior edge), the parity of the
//! boundary-edge crossings of the arc to the query decides membership. Crossing signs
//! use [`robust::orient3d`], so they are exact. This handles any simple polygon,
//! including ones larger than a hemisphere and non-convex ones, with no size limit.
//!
//! Ring orientation is **respected as given**, following OGC/GeoJSON: the interior is to
//! the left of each directed edge (exterior counter-clockwise as seen from outside, holes
//! clockwise). It is deliberately not auto-canonicalised, because forcing an orientation
//! always selects the smaller region – wrong for a polygon larger than a hemisphere. The
//! polygon `envelope()` is the boundary box unioned with the filled interior's coordinate
//! extrema (the cardinal axis points and per-edge great-circle extrema that lie inside),
//! so it covers the filled region – including an enclosed pole – not just the boundary.

#[allow(unused_imports)]
use num_traits::Float;

use alloc::vec::Vec;

use robust::{orient3d, Coord3D};

use crate::{Envelope, PointDistance, RTreeObject, AABB};

use super::arc::{arc_bounding_box, arc_distance_2, cross, dot, norm, scale, sub, ANTIPODAL_C2};
use super::coord::{GeodeticCoord, GeodeticError};
use super::embedding::{squared_chord, UnitVec};

/// How far inside the boundary to place the ray-cast reference point (radians). Small
/// enough to stay inside any non-degenerate polygon, large enough that the crossing
/// signs around it are unambiguous.
const PROBE_EPS: f64 = 1e-7;

// --- raw vector helpers (the rest are reused from `super::arc`) ---

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn normalise(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / norm(a))
}

/// Sign of the scalar triple product `a . (b x c)` (equivalently `det[a, b, c]`),
/// computed with the robust `orient3d` predicate (the fourth point is the origin).
fn triple(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let p = |v: [f64; 3]| Coord3D {
        x: v[0],
        y: v[1],
        z: v[2],
    };
    orient3d(p(a), p(b), p(c), p([0.0, 0.0, 0.0]))
}

/// True iff the shorter great-circle arcs `a -> b` and `c -> d` cross (their interiors
/// intersect). This is s2's `S2EdgeUtil::SimpleCrossing`, with the four orientation
/// determinants evaluated by the robust [`triple`] predicate.
///
/// Each `triple(x, y, z) = det[x, y, z]` is the signed side of `z` relative to the plane
/// spanned by `x, y`. Two shorter arcs cross iff `c` and `d` lie on opposite sides of the
/// plane of `a, b` *and* `a` and `b` lie on opposite sides of the plane of `c, d`, with
/// the four senses consistent: the first product rules out `c, d` sharing a side of
/// `a -> b` (a fast reject), and the last two confirm an interior crossing rather
/// than the great circles meeting outside both arcs. Robust signs make the decision exact
/// on the boundary, so a query whose ray grazes a vertex is classified deterministically.
fn arcs_cross(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> bool {
    let acb = -triple(a, b, c);
    let bda = triple(a, b, d);
    if acb * bda <= 0.0 {
        return false;
    }
    let cbd = -triple(c, d, b);
    let dac = triple(c, d, a);
    acb * cbd > 0.0 && acb * dac > 0.0
}

/// A closed ring of a spherical polygon: a sequence of vertices (first == last) joined
/// by shorter great-circle arcs.
///
/// The orientation is **respected as given**, and encodes which of the two regions the
/// ring bounds is the interior: the interior is to the **left** of each directed edge.
/// Following OGC/GeoJSON, an exterior ring must be counter-clockwise as seen from
/// outside and a hole clockwise. This is deliberately not auto-canonicalised: forcing an
/// orientation would always select the smaller region, which is wrong for a polygon
/// larger than a hemisphere (the case the ray-cast membership test exists to support).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeodeticRing {
    coords: Vec<GeodeticCoord>,
    vectors: Vec<UnitVec>,
}

impl GeodeticRing {
    /// Builds a ring from an iterator of `(lon, lat)` degree coordinates, validating each
    /// (range checks as [`GeodeticCoord::try_new`]) and enforcing the structural
    /// preconditions: the ring is closed (first vertex equals last,
    /// [`GeodeticError::RingNotClosed`]); it has at least three distinct vertices
    /// ([`GeodeticError::TooFewPoints`]); every edge spans `< 180` degrees
    /// ([`GeodeticError::EdgeSpansHalfCircle`]). The orientation is respected as given,
    /// not reordered (see the type-level documentation).
    ///
    /// Like [`GeodeticLineString::try_from_lonlat`](super::GeodeticLineString::try_from_lonlat),
    /// this has no unchecked variant: the per-vertex embedding dominates the range and
    /// closure checks, and the heavier envelope cost lands later in
    /// [`GeodeticPolygon::try_new`].
    pub fn try_from_lonlat<I, P>(points: I) -> Result<Self, GeodeticError>
    where
        I: IntoIterator<Item = P>,
        P: Into<GeodeticCoord>,
    {
        let coords = points
            .into_iter()
            .map(|p| {
                let c = p.into();
                GeodeticCoord::try_new(c.lon, c.lat)
            })
            .collect::<Result<Vec<_>, _>>()?;

        if coords.first() != coords.last() || coords.len() < 2 {
            return Err(GeodeticError::RingNotClosed);
        }
        // Distinct vertices, excluding the closing duplicate.
        let body = &coords[..coords.len() - 1];
        let distinct = body
            .iter()
            .enumerate()
            .filter(|(i, c)| !body[..*i].contains(c))
            .count();
        if distinct < 3 {
            return Err(GeodeticError::TooFewPoints {
                found: distinct,
                needed: 3,
            });
        }

        let vectors: Vec<UnitVec> = coords.iter().map(|c| c.to_unit_vector()).collect();
        for (index, edge) in vectors.windows(2).enumerate() {
            if squared_chord(edge[0], edge[1]) >= ANTIPODAL_C2 {
                return Err(GeodeticError::EdgeSpansHalfCircle { index });
            }
        }

        Ok(Self { coords, vectors })
    }

    /// The original `(lon, lat)` vertices in degrees (closed; first == last).
    pub fn coords(&self) -> &[GeodeticCoord] {
        &self.coords
    }

    /// The precomputed unit-sphere embedding, one vector per vertex (closed).
    pub fn vectors(&self) -> &[UnitVec] {
        &self.vectors
    }
}

/// A filled spherical polygon leaf: an exterior ring and any interior rings (holes).
///
/// A query's distance to the polygon is the **minimum** great-circle distance to it: zero
/// strictly inside the filled region (interior of the exterior ring, outside every hole),
/// otherwise the distance to the nearest boundary point, which may lie on the interior of
/// an edge, not only at a vertex. Spherical model; for WGS84 treat the result as a
/// conservative filter. Each edge must span `< 180` degrees; the
/// antimeridian needs no special handling. Ring orientation follows OGC/GeoJSON
/// (exterior counter-clockwise as seen from outside, holes clockwise) and is respected as
/// given, not auto-corrected, so polygons larger than a hemisphere are expressible.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "geodetic")]
/// # fn main() {
/// use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticPolygon, GeodeticRing};
///
/// // A lon/lat square, counter-clockwise as seen from outside (interior to the left).
/// let ring = GeodeticRing::try_from_lonlat([
///     (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0),
/// ])
/// .unwrap();
/// let poly = GeodeticPolygon::try_new(ring, Vec::new()).unwrap();
/// let tree = GeodeticRTree::bulk_load(vec![poly]);
///
/// // A query inside the polygon is at distance zero; one outside is positive.
/// let (_, inside_m) = tree
///     .nearest_neighbor_with_distance(GeodeticCoord { lon: 5.0, lat: 5.0 })
///     .unwrap();
/// assert_eq!(inside_m, 0.0);
/// let (_, outside_m) = tree
///     .nearest_neighbor_with_distance(GeodeticCoord { lon: -5.0, lat: 5.0 })
///     .unwrap();
/// assert!(outside_m > 0.0);
/// # }
/// # #[cfg(not(feature = "geodetic"))] fn main() {}
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeodeticPolygon {
    exterior: GeodeticRing,
    interiors: Vec<GeodeticRing>,
    // Precomputed once at construction: the envelope is O(edges^2) (each edge great
    // circle's per-axis extrema tested for interior membership), and the R-tree queries
    // it repeatedly.
    envelope: AABB<UnitVec>,
}

impl GeodeticPolygon {
    /// Builds a polygon from an exterior ring and interior rings (holes). Orientation is
    /// respected as given: the exterior must be counter-clockwise as seen from outside
    /// and each hole clockwise (OGC/GeoJSON), which fixes the filled interior – including
    /// for a polygon larger than a hemisphere.
    ///
    /// # Performance
    ///
    /// The dominant cost is the O(edges^2) envelope built here (each edge's great circle
    /// tested for per-axis extrema), not validation: the rings were already checked when
    /// constructed. That is why there is no unchecked constructor; it could not skip the
    /// envelope, which is the expensive step.
    pub fn try_new(
        exterior: GeodeticRing,
        interiors: Vec<GeodeticRing>,
    ) -> Result<Self, GeodeticError> {
        let mut polygon = Self {
            exterior,
            interiors,
            envelope: AABB::new_empty(),
        };
        polygon.envelope = polygon.compute_envelope();
        Ok(polygon)
    }

    /// The exterior ring.
    pub fn exterior(&self) -> &GeodeticRing {
        &self.exterior
    }

    /// The interior rings (holes).
    pub fn interiors(&self) -> &[GeodeticRing] {
        &self.interiors
    }

    fn rings(&self) -> impl Iterator<Item = &GeodeticRing> {
        core::iter::once(&self.exterior).chain(self.interiors.iter())
    }

    /// A reference point just inside the filled region: a small step to the interior
    /// (left) side of the first exterior edge's midpoint. Valid because holes do not
    /// touch the exterior boundary, so a point just inside the exterior is in the fill.
    fn interior_probe(&self) -> [f64; 3] {
        let v0 = self.exterior.vectors[0].0;
        let v1 = self.exterior.vectors[1].0;
        let midpoint = normalise(add(v0, v1));
        // For a counter-clockwise exterior, the interior is the +cross(v0, v1) side.
        let inward = normalise(cross(v0, v1));
        normalise(add(midpoint, scale(inward, PROBE_EPS)))
    }

    /// True iff `query` lies in the filled polygon, by great-circle ray-cast parity from
    /// the interior reference point: an even number of boundary crossings means `query`
    /// is on the same side as the (interior) reference, hence inside.
    pub fn contains_point(&self, query: UnitVec) -> bool {
        let probe = self.interior_probe();
        let mut crossings = 0u32;
        for ring in self.rings() {
            for edge in ring.vectors.windows(2) {
                if arcs_cross(probe, query.0, edge[0].0, edge[1].0) {
                    crossings += 1;
                }
            }
        }
        crossings % 2 == 0
    }
}

impl GeodeticPolygon {
    /// Computes the arc-aware bounding box: the boundary box unioned with the filled
    /// interior's coordinate extrema. Run once at construction and cached in `envelope`.
    fn compute_envelope(&self) -> AABB<UnitVec> {
        // Boundary box: the arc-aware box of every edge of every ring.
        let mut bbox: Option<AABB<UnitVec>> = None;
        for ring in self.rings() {
            for edge in ring.vectors.windows(2) {
                let edge_box = arc_bounding_box(edge[0], edge[1]);
                bbox = Some(match bbox {
                    Some(mut b) => {
                        b.merge(&edge_box);
                        b
                    }
                    None => edge_box,
                });
            }
        }
        let mut bbox = bbox.expect("a polygon has at least one edge");

        // Filled-interior union: a polygon's interior can reach a coordinate extremum at
        // a point strictly inside it (a pole, or a small-circle/edge tangency), which the
        // boundary box misses. Union every such candidate that lies inside.
        let mut union_if_inside = |p: [f64; 3]| {
            if self.contains_point(UnitVec(p)) {
                bbox.merge(&AABB::from_point(UnitVec(p)));
            }
        };

        // The six cardinal axis points (the poles are the +-z cases).
        for axis in [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ] {
            union_if_inside(axis);
        }

        // Each edge great circle's per-axis extreme points (unconditionally, not gated by
        // the on-arc test, since an interior extremum need not lie on the boundary arc).
        for ring in self.rings() {
            for edge in ring.vectors.windows(2) {
                let n = cross(edge[0].0, edge[1].0);
                let nn = norm(n);
                if nn <= 0.0 {
                    continue;
                }
                let n = scale(n, 1.0 / nn);
                for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                    let proj = sub(axis, scale(n, dot(axis, n)));
                    let r = norm(proj);
                    if r > 1e-12 {
                        let extreme = scale(proj, 1.0 / r);
                        union_if_inside(extreme);
                        union_if_inside(scale(extreme, -1.0));
                    }
                }
            }
        }

        bbox
    }
}

impl RTreeObject for GeodeticPolygon {
    type Envelope = AABB<UnitVec>;

    fn envelope(&self) -> AABB<UnitVec> {
        self.envelope
    }
}

impl PointDistance for GeodeticPolygon {
    /// Zero if `query` is inside the filled polygon, otherwise the minimum squared chord
    /// to any boundary edge ([`arc_distance_2`]). Same squared-chord units as the
    /// envelope. `contains_point` and `distance_2_if_less_or_equal` keep the trait
    /// defaults.
    fn distance_2(&self, query: &UnitVec) -> f64 {
        if self.contains_point(*query) {
            return 0.0;
        }
        let mut best = f64::INFINITY;
        for ring in self.rings() {
            for edge in ring.vectors.windows(2) {
                best = best.min(arc_distance_2(edge[0], edge[1], *query));
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uv(lon: f64, lat: f64) -> UnitVec {
        GeodeticCoord { lon, lat }.to_unit_vector()
    }

    /// A polar cap: a ring at constant latitude `lat0`, traversed eastward (north) or
    /// westward (south). `m` segments.
    fn polar_cap(lat0: f64, m: usize, eastward: bool) -> GeodeticRing {
        let mut coords: Vec<(f64, f64)> = (0..m)
            .map(|i| {
                let frac = i as f64 / m as f64;
                let lon = if eastward {
                    -180.0 + 360.0 * frac
                } else {
                    180.0 - 360.0 * frac
                };
                (lon, lat0)
            })
            .collect();
        coords.push(coords[0]);
        GeodeticRing::try_from_lonlat(coords).expect("valid ring")
    }

    #[test]
    fn arcs_cross_basic() {
        // The equatorial arc (-10,0)->(10,0) crosses the meridian arc (0,-10)->(0,10).
        assert!(arcs_cross(
            uv(-10.0, 0.0).0,
            uv(10.0, 0.0).0,
            uv(0.0, -10.0).0,
            uv(0.0, 10.0).0
        ));
        // Two disjoint arcs do not cross.
        assert!(!arcs_cross(
            uv(-10.0, 0.0).0,
            uv(10.0, 0.0).0,
            uv(0.0, 20.0).0,
            uv(0.0, 40.0).0
        ));
    }

    #[test]
    fn north_polar_cap_contains_pole_not_equator() {
        let poly = GeodeticPolygon::try_new(polar_cap(80.0, 32, true), Vec::new()).unwrap();
        assert!(
            poly.contains_point(UnitVec([0.0, 0.0, 1.0])),
            "north pole inside"
        );
        assert!(poly.contains_point(uv(45.0, 85.0)), "lat 85 inside the cap");
        assert!(
            !poly.contains_point(uv(45.0, 70.0)),
            "lat 70 outside the cap"
        );
        assert!(!poly.contains_point(uv(0.0, 0.0)), "equator outside");
        assert!(
            !poly.contains_point(UnitVec([0.0, 0.0, -1.0])),
            "south pole outside"
        );
    }

    #[test]
    fn orientation_is_respected_not_flipped() {
        // The lat-80 ring traversed westward designates the *other* region as interior
        // (everything south of lat 80), so the north pole is now outside. Orientation is
        // respected, not auto-canonicalised – which is what lets >hemisphere polygons work.
        let poly = GeodeticPolygon::try_new(polar_cap(80.0, 32, false), Vec::new()).unwrap();
        assert!(
            !poly.contains_point(UnitVec([0.0, 0.0, 1.0])),
            "north pole outside the southern region"
        );
        assert!(
            poly.contains_point(uv(0.0, 0.0)),
            "equator inside the southern region"
        );
    }

    #[test]
    fn over_hemisphere_cap_contains_far_interior() {
        // A cap around the north pole of angular radius 150 deg: its boundary is the
        // parallel at lat -60, and its filled interior reaches well past the equator
        // (everything within 150 deg of the pole). The ray-cast must get this right where
        // the signed-winding test would fail.
        let poly = GeodeticPolygon::try_new(polar_cap(-60.0, 64, true), Vec::new()).unwrap();
        assert!(
            poly.contains_point(UnitVec([0.0, 0.0, 1.0])),
            "north pole inside the >hemisphere cap"
        );
        assert!(
            poly.contains_point(uv(0.0, -50.0)),
            "lat -50 (140 deg from the pole) inside"
        );
        assert!(
            !poly.contains_point(uv(0.0, -70.0)),
            "lat -70 (160 deg from the pole) outside"
        );
        assert!(
            !poly.contains_point(UnitVec([0.0, 0.0, -1.0])),
            "south pole (180 deg) outside"
        );
    }

    #[test]
    fn south_cap_does_not_contain_north_pole() {
        // A cap around the south pole (the lat -80 ring traversed westward, so the
        // southern region is the interior) must exclude the north pole.
        let poly = GeodeticPolygon::try_new(polar_cap(-80.0, 32, false), Vec::new()).unwrap();
        assert!(
            poly.contains_point(UnitVec([0.0, 0.0, -1.0])),
            "south pole inside"
        );
        assert!(
            !poly.contains_point(UnitVec([0.0, 0.0, 1.0])),
            "north pole NOT inside"
        );
    }

    #[test]
    fn envelope_reaches_enclosed_pole_and_distance_zero_inside() {
        let poly = GeodeticPolygon::try_new(polar_cap(80.0, 64, true), Vec::new()).unwrap();
        let env = poly.envelope();
        assert!(
            env.contains_point(&UnitVec([0.0, 0.0, 1.0])),
            "box must reach the enclosed pole"
        );
        assert_eq!(
            poly.distance_2(&UnitVec([0.0, 0.0, 1.0])),
            0.0,
            "the pole is interior, distance zero"
        );
        // A point well outside has a positive distance.
        assert!(poly.distance_2(&uv(0.0, 0.0)) > 0.0);
    }

    #[test]
    fn construction_rejects_unclosed_and_too_few() {
        // Unclosed ring.
        assert_eq!(
            GeodeticRing::try_from_lonlat([(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]),
            Err(GeodeticError::RingNotClosed)
        );
        // Closed but fewer than three distinct vertices.
        assert_eq!(
            GeodeticRing::try_from_lonlat([(0.0, 0.0), (10.0, 0.0), (0.0, 0.0)]),
            Err(GeodeticError::TooFewPoints {
                found: 2,
                needed: 3
            })
        );
    }
}
