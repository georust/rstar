//! [`GeodeticLineString`]: a polyline leaf for the geodetic R-tree.

use alloc::vec::Vec;

use crate::{Envelope, PointDistance, RTreeObject, AABB};

use super::arc::{arc_bounding_box, arc_distance_2, ANTIPODAL_C2};
use super::coord::{GeodeticCoord, GeodeticError};
use super::embedding::{squared_chord, UnitVec};

/// A polyline leaf indexed in a geodetic R-tree: a sequence of `>= 2` vertices joined
/// by shorter great-circle arcs.
///
/// Like [`super::GeodeticPoint`], it stores the original `(lon, lat)` degrees alongside
/// the precomputed unit-sphere embedding; the embedding is what the index uses for the
/// arc-aware envelope and the point-to-geometry distance, while the degrees are kept for
/// results and round-trips. It is deliberately **not** a [`crate::Point`], so the custom
/// great-circle [`PointDistance`] applies rather than the blanket point metric.
///
/// A query's distance to the linestring is the **minimum** great-circle distance to it:
/// the distance to the nearest point on the polyline, which may lie on the interior of an
/// edge, not only at a vertex. The model is spherical; for WGS84, treat the result as a
/// conservative filter. Each edge must span `< 180` degrees (densify longer edges first).
/// The antimeridian needs no special handling – the embedding makes the seam an ordinary
/// interior region.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "geodetic")]
/// # fn main() {
/// use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticLineString};
///
/// let near = GeodeticLineString::try_from_lonlat([(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)]).unwrap();
/// let far = GeodeticLineString::try_from_lonlat([(10.0, 10.0), (11.0, 11.0)]).unwrap();
/// let tree = GeodeticRTree::bulk_load(vec![near, far]);
///
/// // The nearest linestring to the query, with the great-circle distance in metres.
/// let (nearest, metres) = tree
///     .nearest_neighbor_with_distance(GeodeticCoord { lon: 1.0, lat: 0.5 })
///     .unwrap();
/// assert_eq!(nearest.coords()[0], GeodeticCoord { lon: 0.0, lat: 0.0 });
/// assert!(metres < 100_000.0); // within ~100 km of the nearer linestring
/// # }
/// # #[cfg(not(feature = "geodetic"))] fn main() {}
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct GeodeticLineString {
    // Original degrees, for results / round-trip.
    coords: Vec<GeodeticCoord>,
    // Precomputed embedding: one unit vector per vertex, the indexed geometry.
    vectors: Vec<UnitVec>,
    // Precomputed once at construction (O(edges)): the arc-aware bounding box, which the
    // R-tree queries repeatedly. Mirrors the cached envelope on `super::GeodeticPolygon`.
    envelope: AABB<UnitVec>,
}

impl GeodeticLineString {
    /// Builds a linestring from an iterator of `(lon, lat)` degree coordinates,
    /// validating as it goes.
    ///
    /// Accepts `(f64, f64)`, `[f64; 2]`, or [`GeodeticCoord`] items (anything
    /// `Into<GeodeticCoord>`). Each coordinate is range-checked exactly as
    /// [`GeodeticCoord::try_new`] (so a swapped lat/lon with `|lon| > 90` is caught),
    /// then the structural preconditions are enforced:
    ///
    /// - at least two vertices ([`GeodeticError::TooFewPoints`]);
    /// - every edge spans `< 180` degrees ([`GeodeticError::EdgeSpansHalfCircle`]).
    ///
    /// Duplicate consecutive vertices are allowed (a zero-length edge contributes its
    /// endpoint distance and a degenerate box).
    ///
    /// # Performance
    ///
    /// There is deliberately no unchecked constructor. The validation (a few range
    /// comparisons per vertex and one chord test per edge) is dwarfed by the unit-vector
    /// embedding computed for every vertex and the arc-aware envelope built once here,
    /// work any constructor must do regardless. Skipping the checks would save a
    /// negligible fraction of the total.
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

        if coords.len() < 2 {
            return Err(GeodeticError::TooFewPoints {
                found: coords.len(),
                needed: 2,
            });
        }

        let vectors: Vec<UnitVec> = coords.iter().map(|c| c.to_unit_vector()).collect();
        for (index, edge) in vectors.windows(2).enumerate() {
            if squared_chord(edge[0], edge[1]) >= ANTIPODAL_C2 {
                return Err(GeodeticError::EdgeSpansHalfCircle { index });
            }
        }

        let envelope = arc_aware_bounding_box(&vectors);
        Ok(Self {
            coords,
            vectors,
            envelope,
        })
    }

    /// The original `(lon, lat)` vertices in degrees.
    pub fn coords(&self) -> &[GeodeticCoord] {
        &self.coords
    }

    /// The precomputed unit-sphere embedding, one vector per vertex.
    pub fn vectors(&self) -> &[UnitVec] {
        &self.vectors
    }
}

/// The arc-aware bounding box of a vertex sequence: the [`Envelope::merge`] of each
/// edge's [`arc_bounding_box`], so it contains every point of every great-circle edge,
/// not merely the vertices. Computed once in [`GeodeticLineString::try_from_lonlat`] and
/// cached, since the R-tree queries the envelope repeatedly.
fn arc_aware_bounding_box(vectors: &[UnitVec]) -> AABB<UnitVec> {
    let mut edges = vectors.windows(2);
    // `try_from_lonlat` guarantees >= 2 vertices, so there is at least one edge.
    let first = edges.next().expect("a linestring has at least one edge");
    let mut bbox = arc_bounding_box(first[0], first[1]);
    for edge in edges {
        bbox.merge(&arc_bounding_box(edge[0], edge[1]));
    }
    bbox
}

impl RTreeObject for GeodeticLineString {
    type Envelope = AABB<UnitVec>;

    fn envelope(&self) -> AABB<UnitVec> {
        self.envelope
    }
}

impl PointDistance for GeodeticLineString {
    /// Squared chord to the nearest point of the polyline: the minimum over its edges of
    /// the point-to-arc [`arc_distance_2`]. Same squared-chord units as the envelope, so
    /// the pruning lower bound holds.
    ///
    /// `contains_point` and `distance_2_if_less_or_equal` keep the trait defaults. For an
    /// extent leaf the envelope strictly contains the geometry, so the default gate
    /// (`envelope().distance_2(q) <= max`) is still a true lower bound and never wrongly
    /// rejects a candidate.
    fn distance_2(&self, query: &UnitVec) -> f64 {
        self.vectors
            .windows(2)
            .map(|edge| arc_distance_2(edge[0], edge[1], *query))
            .fold(f64::INFINITY, f64::min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geodetic::GeodeticCoord;

    fn ls(pts: &[(f64, f64)]) -> GeodeticLineString {
        GeodeticLineString::try_from_lonlat(pts.iter().copied()).expect("valid")
    }

    fn uv(lon: f64, lat: f64) -> UnitVec {
        GeodeticCoord { lon, lat }.to_unit_vector()
    }

    #[test]
    fn accepts_tuples_arrays_and_coords() {
        assert!(GeodeticLineString::try_from_lonlat([(0.0, 0.0), (1.0, 1.0)]).is_ok());
        assert!(GeodeticLineString::try_from_lonlat([[0.0, 0.0], [1.0, 1.0]]).is_ok());
        assert!(GeodeticLineString::try_from_lonlat([
            GeodeticCoord { lon: 0.0, lat: 0.0 },
            GeodeticCoord { lon: 1.0, lat: 1.0 },
        ])
        .is_ok());
    }

    #[test]
    fn rejects_single_vertex() {
        assert_eq!(
            GeodeticLineString::try_from_lonlat([(0.0, 0.0)]),
            Err(GeodeticError::TooFewPoints {
                found: 1,
                needed: 2
            })
        );
    }

    #[test]
    fn rejects_antipodal_edge() {
        // (0, 0) and (180, 0) are exact antipodes; the shorter arc is undefined.
        assert_eq!(
            GeodeticLineString::try_from_lonlat([(0.0, 0.0), (180.0, 0.0)]),
            Err(GeodeticError::EdgeSpansHalfCircle { index: 0 })
        );
    }

    #[test]
    fn catches_swapped_lat_lon() {
        // Tokyo written as (lat, lon): 139.6917 lands in the lat slot, out of range.
        assert_eq!(
            GeodeticLineString::try_from_lonlat([(35.6895, 139.6917), (0.0, 0.0)]),
            Err(GeodeticError::LatOutOfRange(139.6917))
        );
    }

    #[test]
    fn allows_duplicate_consecutive_vertices() {
        let line = ls(&[(10.0, 10.0), (10.0, 10.0), (20.0, 20.0)]);
        assert_eq!(line.coords().len(), 3);
        // The duplicate (zero-length) edge does not pull the distance below the real one.
        let q = uv(15.0, 15.0);
        let two_vertex = ls(&[(10.0, 10.0), (20.0, 20.0)]);
        assert!((line.distance_2(&q) - two_vertex.distance_2(&q)).abs() < 1e-12);
    }

    #[test]
    fn envelope_contains_vertices_and_over_pole_bulge() {
        // An over-pole edge: the box must reach the pole even though no vertex does.
        let line = ls(&[(0.0, 80.0), (180.0, 80.0)]);
        let env = line.envelope();
        for v in line.vectors() {
            assert!(env.contains_point(v));
        }
        assert!(
            (env.upper().0[2] - 1.0).abs() < 1e-9,
            "box should reach the pole"
        );
    }

    #[test]
    fn distance_2_zero_on_a_vertex_and_on_an_interior_edge_point() {
        let line = ls(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert!(
            line.distance_2(&uv(10.0, 0.0)) < 1e-12,
            "on the shared vertex"
        );
        // (10, 5) lies on the second (meridian) edge.
        assert!(
            line.distance_2(&uv(10.0, 5.0)) < 1e-12,
            "on the second edge"
        );
    }
}
