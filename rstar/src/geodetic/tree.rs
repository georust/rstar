//! The geodetic R-tree: degrees in, metres out.

use alloc::vec::Vec;

use crate::iterators::{RTreeIterator, RTreeIteratorMut};
use crate::primitives::GeomWithData;
use crate::{ParentNode, PointDistance, RTree, RTreeObject, AABB};

use super::coord::GeodeticCoord;
use super::distance::{metres_to_squared_chord, squared_chord_to_metres};
use super::embedding::{rectangle_bounding_box, rectangle_contains, UnitVec};
use super::point::GeodeticPoint;
#[cfg(feature = "geodetic-wgs84")]
use super::spheroid::{
    geodesic_metres, geodesic_spherical_margin, geoid_for, radius_fetch_metres,
    spherical_lower_bound_metres, Ellipsoid,
};

/// A type that can be indexed in a [`GeodeticRTree`]: any [`RTreeObject`] with a
/// unit-sphere [`AABB<UnitVec>`](crate::AABB) envelope that also implements
/// [`PointDistance`], alongside the built-in [`GeodeticPoint`],
/// [`GeodeticLineString`](super::GeodeticLineString), and
/// [`GeodeticPolygon`](super::GeodeticPolygon).
///
/// # Building a custom leaf type
///
/// The trait is an open marker with a blanket impl, so any type that satisfies the two
/// bounds is indexable; you implement [`RTreeObject`] and [`PointDistance`], not this
/// trait directly. Both work in the **unit-sphere embedding**, in which each
/// `(lon, lat)` is a [`UnitVec`](super::UnitVec) on the unit sphere (obtain one with
/// `UnitVec::from(coord)` or [`GeodeticCoord::to_unit_vector`](super::GeodeticCoord::to_unit_vector)):
///
/// - `envelope` returns an [`AABB<UnitVec>`](crate::AABB) that **encloses the whole
///   geometry**, great-circle edges and all. Build it from the public great-circle
///   primitives – [`arc_bounding_box`](super::arc_bounding_box) per edge, merged – so the
///   box covers each edge's bulge, not merely its vertices.
/// - `distance_2` returns the **squared-chord** distance (in `[0, 4]`, the same metric
///   the envelope uses) from the query to the nearest point of the geometry, via
///   [`arc_distance_2`](super::arc_distance_2) or
///   [`squared_chord`](super::squared_chord). The tree converts it to metres on the way
///   out; do not return metres here, or the envelope lower bound and the leaf distance
///   would be in different units and pruning would be unsound.
///
/// ```
/// use rstar::{AABB, PointDistance, RTreeObject};
/// use rstar::geodetic::{arc_bounding_box, arc_distance_2, GeodeticCoord, GeodeticRTree, UnitVec};
///
/// /// A custom leaf: a single great-circle segment between two lon/lat points.
/// struct Segment {
///     a: UnitVec,
///     b: UnitVec,
/// }
///
/// impl Segment {
///     fn new(a: GeodeticCoord, b: GeodeticCoord) -> Self {
///         Segment { a: a.into(), b: b.into() }
///     }
/// }
///
/// impl RTreeObject for Segment {
///     type Envelope = AABB<UnitVec>;
///     fn envelope(&self) -> AABB<UnitVec> {
///         arc_bounding_box(self.a, self.b)
///     }
/// }
///
/// impl PointDistance for Segment {
///     fn distance_2(&self, query: &UnitVec) -> f64 {
///         arc_distance_2(self.a, self.b, *query)
///     }
/// }
///
/// // `Segment` now satisfies `GeodeticObject` via the blanket impl and can be indexed.
/// let tree = GeodeticRTree::bulk_load(vec![Segment::new(
///     GeodeticCoord { lon: 0.0, lat: 0.0 },
///     GeodeticCoord { lon: 10.0, lat: 0.0 },
/// )]);
///
/// // One degree of latitude north of the segment: nearest point is (5, 0), ~111 km away.
/// let query = GeodeticCoord { lon: 5.0, lat: 1.0 };
/// let (_segment, metres) = tree.nearest_neighbor_with_distance(query).unwrap();
/// assert!((110_000.0..112_000.0).contains(&metres));
/// ```
pub trait GeodeticObject: RTreeObject<Envelope = AABB<UnitVec>> + PointDistance {}

impl<T: RTreeObject<Envelope = AABB<UnitVec>> + PointDistance> GeodeticObject for T {}

/// A leaf that is a single point and can report the longitude/latitude it was built
/// from. The point-only queries (the exact-location lookups and the longitude/latitude
/// window query) are available on a [`GeodeticRTree`] of any such leaf: [`GeodeticPoint`]
/// itself, or `GeomWithData<GeodeticPoint, T>` (a point with attached data, such as its
/// position in an input sequence).
pub trait PointLeaf: GeodeticObject {
    /// The point's original longitude/latitude coordinate, in degrees.
    fn coord(&self) -> GeodeticCoord;
}

impl PointLeaf for GeodeticPoint {
    fn coord(&self) -> GeodeticCoord {
        GeodeticPoint::coord(self)
    }
}

impl<T> PointLeaf for GeomWithData<GeodeticPoint, T> {
    fn coord(&self) -> GeodeticCoord {
        self.geom().coord()
    }
}

/// A geodetic R-tree over longitude/latitude data.
///
/// Queries take [`GeodeticCoord`] in degrees and return great-circle distances in
/// **metres**. The antimeridian and the poles need no special handling – there is no
/// wrapping or point duplication. Most methods mirror the matching [`RTree`] methods —
/// see [`RTree`] for their detailed semantics and complexity.
///
/// The leaf type `G` defaults to [`GeodeticPoint`], so a bare `GeodeticRTree` is a point
/// tree; line and polygon trees are `GeodeticRTree<GeodeticLineString>` and
/// `GeodeticRTree<GeodeticPolygon>`. Queries run from a query *point* against the indexed
/// geometries:
///
/// - **Nearest-neighbour** and **radius** queries, against any leaf type, by the
///   great-circle distance from the query point to the nearest point of each geometry —
///   zero when the point is inside a polygon.
/// - **Exact-location** and **longitude/latitude rectangle** lookups (`locate_at_point`,
///   [`locate_in_rectangle`](GeodeticRTree::locate_in_rectangle)), on point trees only:
///   any [`PointLeaf`], such as `GeodeticPoint` or `GeomWithData<GeodeticPoint, T>`; the
///   window query is **not** currently provided for line or polygon extents.
///
/// # Example
///
/// ```
/// use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticPoint};
///
/// let tree = GeodeticRTree::bulk_load(vec![
///     GeodeticPoint::new(-0.1278, 51.5074), // London
///     GeodeticPoint::new(2.3522, 48.8566),  // Paris
///     GeodeticPoint::new(13.4050, 52.5200), // Berlin
/// ]);
///
/// // Nearest city to Amsterdam, with its great-circle distance in metres.
/// let amsterdam = GeodeticCoord { lon: 4.9041, lat: 52.3676 };
/// let (nearest, metres) = tree.nearest_neighbor_with_distance(amsterdam).unwrap();
///
/// assert_eq!(nearest.coord().lon, -0.1278); // London
/// assert!(metres < 400_000.0); // ~360 km
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeodeticRTree<G: GeodeticObject = GeodeticPoint> {
    inner: RTree<G>,
}

// `RTree::<G>::new` requires only `G: RTreeObject`, so `Default` does not need `G:
// Default`; the derive would wrongly demand it, hence the hand-written impl.
impl<G: GeodeticObject> Default for GeodeticRTree<G> {
    fn default() -> Self {
        Self {
            inner: RTree::new(),
        }
    }
}

impl<G: GeodeticObject> GeodeticRTree<G> {
    // --- construction / structure (leaf-agnostic) ---

    /// Creates an empty tree.
    pub fn new() -> Self {
        Self {
            inner: RTree::new(),
        }
    }

    /// Bulk-loads a tree from a vector of geometries (the recommended way to build a
    /// static index).
    pub fn bulk_load(items: Vec<G>) -> Self {
        Self {
            inner: RTree::bulk_load(items),
        }
    }

    /// Inserts a single geometry.
    pub fn insert(&mut self, item: G) {
        self.inner.insert(item);
    }

    /// Removes a geometry equal to `item`, returning it if found.
    pub fn remove(&mut self, item: &G) -> Option<G>
    where
        G: PartialEq,
    {
        self.inner.remove(item)
    }

    /// Returns `true` if a geometry equal to `item` is present.
    pub fn contains(&self, item: &G) -> bool
    where
        G: PartialEq,
    {
        self.inner.contains(item)
    }

    /// Returns the number of geometries in the tree.
    pub fn size(&self) -> usize {
        self.inner.size()
    }

    /// Returns `true` if the tree contains no geometries.
    pub fn is_empty(&self) -> bool {
        self.inner.size() == 0
    }

    /// Returns an iterator over all geometries, in arbitrary order.
    pub fn iter(&self) -> RTreeIterator<'_, G> {
        self.inner.iter()
    }

    /// Returns a mutable iterator over all geometries, in arbitrary order.
    ///
    /// This exists for custom leaf types that carry attached data – for example
    /// updating the payload of a `GeomWithData<GeodeticPoint, T>` leaf. Replacing a
    /// leaf's *geometry* through this iterator (e.g. `*p = GeodeticPoint::new(..)`)
    /// corrupts the index: node envelopes are not recomputed.
    pub fn iter_mut(&mut self) -> RTreeIteratorMut<'_, G> {
        self.inner.iter_mut()
    }

    // --- nearest neighbour (metres out) ---

    /// Returns the nearest geometry to `query`, or `None` if the tree is empty
    /// or `query` has a non-finite coordinate.
    pub fn nearest_neighbor(&self, query: GeodeticCoord) -> Option<&G> {
        if !finite(query) {
            return None;
        }
        self.inner.nearest_neighbor(UnitVec::from(query))
    }

    /// Returns the nearest geometry to `query` together with its **minimum** great-circle
    /// distance in **metres** (to the nearest point of the geometry – which for a leaf
    /// type may be on the interior of an edge, not only a vertex), or `None` if the tree
    /// is empty or `query` has a non-finite coordinate.
    pub fn nearest_neighbor_with_distance(&self, query: GeodeticCoord) -> Option<(&G, f64)> {
        if !finite(query) {
            return None;
        }
        self.inner
            .nearest_neighbor_with_distance_2(UnitVec::from(query))
            .map(|(g, c2)| (g, squared_chord_to_metres(c2)))
    }

    /// Returns all geometries sharing the minimum distance to `query` (ties), or an
    /// empty vector if the tree is empty or `query` has a non-finite coordinate.
    pub fn nearest_neighbors(&self, query: GeodeticCoord) -> Vec<&G> {
        if !finite(query) {
            return Vec::new();
        }
        let q = UnitVec::from(query);
        self.inner.nearest_neighbors(&q)
    }

    /// Returns an iterator over all geometries in non-decreasing distance order.
    /// A non-finite query yields nothing.
    pub fn nearest_neighbor_iter(&self, query: GeodeticCoord) -> impl Iterator<Item = &G> + '_ {
        finite(query)
            .then(move || self.inner.nearest_neighbor_iter(UnitVec::from(query)))
            .into_iter()
            .flatten()
    }

    /// Returns an iterator over `(geometry, distance_in_metres)` in non-decreasing
    /// distance order. A non-finite query yields nothing.
    pub fn nearest_neighbor_iter_with_distance(
        &self,
        query: GeodeticCoord,
    ) -> impl Iterator<Item = (&G, f64)> + '_ {
        finite(query)
            .then(move || {
                self.inner
                    .nearest_neighbor_iter_with_distance_2(UnitVec::from(query))
                    .map(|(g, c2)| (g, squared_chord_to_metres(c2)))
            })
            .into_iter()
            .flatten()
    }

    // --- radius query (metres) ---

    /// Returns an iterator over all geometries within `radius_metres` great-circle
    /// metres of `query` (by nearest point), in arbitrary order.
    ///
    /// A negative or NaN radius yields nothing (the set of points within a negative
    /// distance is empty), as does a non-finite query.
    pub fn locate_within_distance(
        &self,
        query: GeodeticCoord,
        radius_metres: f64,
    ) -> impl Iterator<Item = &G> + '_ {
        let valid = finite(query) && radius_metres >= 0.0;
        valid
            .then(move || {
                let threshold = metres_to_squared_chord(radius_metres);
                self.inner
                    .locate_within_distance(UnitVec::from(query), threshold)
            })
            .into_iter()
            .flatten()
    }

    // --- advanced read-only traversal ---

    /// Returns the tree's root node for advanced read-only traversal – counting
    /// nodes, inspecting envelopes, or running custom tree algorithms over
    /// [`crate::RTreeNode`] / [`ParentNode`], mirroring [`crate::RTree::root`].
    ///
    /// Node envelopes are `AABB<UnitVec>` in the unit-vector embedding, so extents
    /// and node-level distances are in the squared-chord metric, not metres. Use
    /// [`envelope_distance_metres`] for the great-circle metres from a query to a
    /// node, or [`squared_chord_to_metres`] to convert a raw squared-chord value (a
    /// leaf or `min_max_dist_2`). This exposes structure only: there is no raw-query
    /// entry point that would let the tree be queried in non-metre units.
    pub fn root(&self) -> &ParentNode<G> {
        self.inner.root()
    }
}

impl<P: PointLeaf> GeodeticRTree<P> {
    // --- exact-location lookup and window query: point-only ---
    //
    // These are specific to point leaves: an exact-location lookup keyed by a single
    // embedded vector, and the longitude/latitude window query, are ill-defined for an
    // extent geometry, which occupies many vectors rather than one.

    /// Removes a point located exactly at `query` (by embedded vector), returning
    /// it if found.
    pub fn remove_at_point(&mut self, query: GeodeticCoord) -> Option<P> {
        self.inner.remove_at_point(UnitVec::from(query))
    }

    /// Returns a point located exactly at `query` (by embedded vector), if any.
    pub fn locate_at_point(&self, query: GeodeticCoord) -> Option<&P> {
        self.inner.locate_at_point(UnitVec::from(query))
    }

    /// Returns all points located exactly at `query` (by embedded vector).
    pub fn locate_all_at_point(&self, query: GeodeticCoord) -> impl Iterator<Item = &P> + '_ {
        self.inner.locate_all_at_point(UnitVec::from(query))
    }

    /// Returns all points inside the longitude/latitude rectangle whose corners are
    /// `lower` and `upper`, in arbitrary order.
    ///
    /// The rectangle spans the latitude band `[lower.lat, upper.lat]` and the
    /// **eastward** longitude arc from `lower.lon` to `upper.lon`. When
    /// `lower.lon <= upper.lon` this is the ordinary interval; when
    /// `lower.lon > upper.lon` the arc wraps across the ±180° antimeridian, so a
    /// window straddling the seam needs no splitting (for example `lower.lon =
    /// 170.0`, `upper.lon = -170.0` selects the 20°-wide band around 180°). This
    /// west-then-east ordering is the GeoJSON
    /// [RFC 7946 §5.2](https://www.rfc-editor.org/rfc/rfc7946.html#section-5.2)
    /// bounding-box convention for antimeridian crossing. `lower.lat <= upper.lat`
    /// is required. All bounds are inclusive, and a point at a pole is returned
    /// whenever the latitude band reaches it, whatever its longitude. A point on the
    /// ±180° seam is matched under either sign, so a window edge on the seam includes
    /// a seam point however it was stored.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "geodetic")]
    /// # fn main() {
    /// use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticPoint};
    ///
    /// let tree = GeodeticRTree::bulk_load(vec![
    ///     GeodeticPoint::new(179.0, 0.0),  // 179°E, just west of the seam
    ///     GeodeticPoint::new(-178.0, 0.0), // 178°W, just east of the seam
    ///     GeodeticPoint::new(0.0, 0.0),    // far away
    /// ]);
    ///
    /// // A window straddling the antimeridian: 170°E eastward to 170°W.
    /// let lower = GeodeticCoord { lon: 170.0, lat: -10.0 };
    /// let upper = GeodeticCoord { lon: -170.0, lat: 10.0 };
    /// assert_eq!(tree.locate_in_rectangle(lower, upper).count(), 2);
    /// # }
    /// # #[cfg(not(feature = "geodetic"))] fn main() {}
    /// ```
    pub fn locate_in_rectangle(
        &self,
        lower: GeodeticCoord,
        upper: GeodeticCoord,
    ) -> impl Iterator<Item = &P> + '_ {
        debug_assert!(
            lower.lat <= upper.lat,
            "locate_in_rectangle requires lower.lat <= upper.lat (got {} > {})",
            lower.lat,
            upper.lat
        );
        let bounding_box = rectangle_bounding_box(lower, upper);
        self.inner
            .locate_in_envelope_intersecting(bounding_box)
            .filter(move |point| rectangle_contains(lower, upper, point.coord()))
    }
}

/// Ellipsoidal geodesic refine for point trees, on any [`PointLeaf`] (the `geodetic-wgs84` feature).
///
/// The spherical index yields candidates ordered by great-circle distance; these
/// methods re-rank or filter them by the exact geodesic distance on a reference
/// [`Ellipsoid`] (Karney, via `geographiclib-rs`). The spherical distance, deflated by a
/// margin derived from the ellipsoid, is a sound lower bound on the geodesic distance,
/// so the branch-and-bound search stays correct: nearest neighbour never stops early
/// and a radius query never drops an in-range point.
///
/// The `*_on_ellipsoid` methods take the ellipsoid explicitly; the `*_wgs84` methods are
/// the [`Ellipsoid::WGS84`] case. "WGS84" denotes the WGS84 reference *ellipsoid*, which
/// is epoch- and realisation-independent (see the [module docs](super)); these methods
/// perform no datum transformation.
#[cfg(feature = "geodetic-wgs84")]
impl<P: PointLeaf> GeodeticRTree<P> {
    /// Returns the geodesic nearest point to `query` on `ellipsoid`, or `None` if the
    /// tree is empty. See [`Self::nearest_neighbor_with_distance_on_ellipsoid`] for the
    /// metric.
    pub fn nearest_neighbor_on_ellipsoid(
        &self,
        query: GeodeticCoord,
        ellipsoid: Ellipsoid,
    ) -> Option<&P> {
        self.nearest_neighbor_with_distance_on_ellipsoid(query, ellipsoid)
            .map(|(point, _)| point)
    }

    /// Returns the geodesic nearest point to `query` on `ellipsoid` together with its
    /// geodesic distance in **metres**, or `None` if the tree is empty.
    ///
    /// Candidates are visited in spherical-distance order and refined to the geodesic
    /// distance; the walk stops once the next candidate's geodesic lower bound exceeds
    /// the best distance found, so only points that could win are measured on the
    /// ellipsoid.
    ///
    /// # Example
    ///
    /// ```
    /// use rstar::geodetic::{Ellipsoid, GeodeticRTree, GeodeticCoord, GeodeticPoint};
    ///
    /// let tree = GeodeticRTree::bulk_load(vec![
    ///     GeodeticPoint::new(2.3522, 48.8566),  // Paris
    ///     GeodeticPoint::new(13.4050, 52.5200), // Berlin
    /// ]);
    ///
    /// let query = GeodeticCoord { lon: 2.0, lat: 49.0 };
    /// let (nearest, metres) = tree
    ///     .nearest_neighbor_with_distance_on_ellipsoid(query, Ellipsoid::GRS80)
    ///     .unwrap();
    ///
    /// assert_eq!(nearest.coord().lon, 2.3522); // Paris is nearest
    /// assert!(metres < 35_000.0);
    /// ```
    pub fn nearest_neighbor_with_distance_on_ellipsoid(
        &self,
        query: GeodeticCoord,
        ellipsoid: Ellipsoid,
    ) -> Option<(&P, f64)> {
        let geoid = geoid_for(ellipsoid);
        let margin = geodesic_spherical_margin(ellipsoid);
        let mut best: Option<(&P, f64)> = None;
        for (point, c2) in self
            .inner
            .nearest_neighbor_iter_with_distance_2(UnitVec::from(query))
        {
            if let Some((_, best_metres)) = best {
                // Candidates arrive in non-decreasing spherical distance, so every
                // remaining one has a geodesic distance of at least this lower bound.
                if spherical_lower_bound_metres(squared_chord_to_metres(c2), margin) > best_metres {
                    break;
                }
            }
            let metres = geodesic_metres(&geoid, query, point.coord());
            match best {
                Some((_, best_metres)) if metres >= best_metres => {}
                _ => best = Some((point, metres)),
            }
        }
        best
    }

    /// Returns every point within `radius_metres` geodesic metres of `query` on
    /// `ellipsoid`, in arbitrary order.
    ///
    /// The spherical filter fetches a superset (the radius widened by the margin), and
    /// each candidate is kept only if its exact geodesic distance is within
    /// `radius_metres`.
    ///
    /// # Example
    ///
    /// ```
    /// use rstar::geodetic::{Ellipsoid, GeodeticRTree, GeodeticCoord, GeodeticPoint};
    ///
    /// let tree = GeodeticRTree::bulk_load(vec![
    ///     GeodeticPoint::new(2.3522, 48.8566),  // Paris
    ///     GeodeticPoint::new(13.4050, 52.5200), // Berlin
    /// ]);
    ///
    /// // Within 100 km of a point near Paris: Paris only, Berlin is ~900 km away.
    /// let query = GeodeticCoord { lon: 2.0, lat: 49.0 };
    /// let within: Vec<_> = tree
    ///     .locate_within_distance_on_ellipsoid(query, 100_000.0, Ellipsoid::GRS80)
    ///     .collect();
    ///
    /// assert_eq!(within.len(), 1);
    /// assert_eq!(within[0].coord().lon, 2.3522); // Paris
    /// ```
    pub fn locate_within_distance_on_ellipsoid(
        &self,
        query: GeodeticCoord,
        radius_metres: f64,
        ellipsoid: Ellipsoid,
    ) -> impl Iterator<Item = &P> + '_ {
        let geoid = geoid_for(ellipsoid);
        let margin = geodesic_spherical_margin(ellipsoid);
        let threshold = metres_to_squared_chord(radius_fetch_metres(radius_metres, margin));
        self.inner
            .locate_within_distance(UnitVec::from(query), threshold)
            .filter(move |point| geodesic_metres(&geoid, query, point.coord()) <= radius_metres)
    }

    /// Returns the WGS84-ellipsoid geodesic nearest point to `query`, or `None` if the
    /// tree is empty; [`Self::nearest_neighbor_on_ellipsoid`] on [`Ellipsoid::WGS84`].
    pub fn nearest_neighbor_wgs84(&self, query: GeodeticCoord) -> Option<&P> {
        self.nearest_neighbor_on_ellipsoid(query, Ellipsoid::WGS84)
    }

    /// Returns the WGS84-ellipsoid geodesic nearest point to `query` with its geodesic
    /// distance in **metres**, or `None` if the tree is empty;
    /// [`Self::nearest_neighbor_with_distance_on_ellipsoid`] on [`Ellipsoid::WGS84`].
    ///
    /// # Example
    ///
    /// ```
    /// use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticPoint};
    ///
    /// let tree = GeodeticRTree::bulk_load(vec![
    ///     GeodeticPoint::new(2.3522, 48.8566),  // Paris
    ///     GeodeticPoint::new(13.4050, 52.5200), // Berlin
    /// ]);
    ///
    /// let query = GeodeticCoord { lon: 2.0, lat: 49.0 };
    /// let (nearest, metres) = tree.nearest_neighbor_with_distance_wgs84(query).unwrap();
    ///
    /// assert_eq!(nearest.coord().lon, 2.3522); // Paris is nearest
    /// // Exact WGS84-ellipsoid geodesic metres, not the spherical approximation.
    /// assert!(metres < 35_000.0);
    /// ```
    pub fn nearest_neighbor_with_distance_wgs84(&self, query: GeodeticCoord) -> Option<(&P, f64)> {
        self.nearest_neighbor_with_distance_on_ellipsoid(query, Ellipsoid::WGS84)
    }

    /// Returns every point within `radius_metres` WGS84-ellipsoid geodesic metres of
    /// `query`, in arbitrary order;
    /// [`Self::locate_within_distance_on_ellipsoid`] on [`Ellipsoid::WGS84`].
    ///
    /// # Example
    ///
    /// ```
    /// use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticPoint};
    ///
    /// let tree = GeodeticRTree::bulk_load(vec![
    ///     GeodeticPoint::new(2.3522, 48.8566),  // Paris
    ///     GeodeticPoint::new(13.4050, 52.5200), // Berlin
    /// ]);
    ///
    /// // Within 100 km of a point near Paris: Paris only, Berlin is ~900 km away.
    /// let query = GeodeticCoord { lon: 2.0, lat: 49.0 };
    /// let within: Vec<_> = tree.locate_within_distance_wgs84(query, 100_000.0).collect();
    ///
    /// assert_eq!(within.len(), 1);
    /// assert_eq!(within[0].coord().lon, 2.3522); // Paris
    /// ```
    pub fn locate_within_distance_wgs84(
        &self,
        query: GeodeticCoord,
        radius_metres: f64,
    ) -> impl Iterator<Item = &P> + '_ {
        self.locate_within_distance_on_ellipsoid(query, radius_metres, Ellipsoid::WGS84)
    }
}

/// Great-circle metres from `query` to the nearest point of a node `envelope`
/// encountered during [`GeodeticRTree::root`] traversal.
///
/// Node envelopes live in the unit-vector embedding, so their `distance_2` is in
/// squared-chord units; this converts it to metres. The query is a [`UnitVec`]:
/// embed once with
/// [`to_unit_vector`](crate::geodetic::GeodeticCoord::to_unit_vector) and reuse
/// it across the traversal, rather than paying the embedding per node. For a leaf
/// or a `min_max_dist_2` value, convert the raw squared chord with
/// [`squared_chord_to_metres`] directly.
pub fn envelope_distance_metres(query: UnitVec, envelope: &AABB<UnitVec>) -> f64 {
    squared_chord_to_metres(envelope.distance_2(&query))
}

/// NaN coordinates embed to an all-NaN vector whose incomparable distances would
/// panic inside the nearest-neighbour heap ordering; rejecting them at the query
/// boundary makes every query method fail the same way (an empty result) instead.
fn finite(query: GeodeticCoord) -> bool {
    query.lon.is_finite() && query.lat.is_finite()
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use crate::geodetic::distance::haversine_distance;
    use crate::geodetic::{GeodeticCoord, GeodeticPoint};

    use super::GeodeticRTree;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    fn capitals() -> (GeodeticPoint, GeodeticPoint, GeodeticPoint, GeodeticPoint) {
        (
            GeodeticPoint::new(-0.1278, 51.5074), // London
            GeodeticPoint::new(2.3522, 48.8566),  // Paris
            GeodeticPoint::new(13.4050, 52.5200), // Berlin
            GeodeticPoint::new(-3.7038, 40.4168), // Madrid
        )
    }

    #[test]
    fn nearest_neighbor_returns_geographically_nearest_city() {
        let (london, paris, berlin, madrid) = capitals();
        let tree = GeodeticRTree::bulk_load(vec![london, paris, berlin, madrid]);

        let nn = tree.nearest_neighbor(coord(2.0, 49.0)).expect("non-empty");
        assert_eq!(*nn, paris);

        let nn = tree.nearest_neighbor(coord(13.5, 52.0)).expect("non-empty");
        assert_eq!(*nn, berlin);
    }

    #[test]
    fn nearest_neighbor_with_distance_matches_haversine() {
        let (london, paris, berlin, madrid) = capitals();
        let tree = GeodeticRTree::bulk_load(vec![london, paris, berlin, madrid]);

        let query = coord(0.0, 50.0);
        let (nn, metres) = tree
            .nearest_neighbor_with_distance(query)
            .expect("non-empty");
        let expected = haversine_distance(nn.coord(), query);
        assert_relative_eq!(metres, expected, epsilon = 1e-3);
    }

    #[test]
    fn locate_within_distance_set_equals_haversine_filter() {
        let (london, paris, berlin, madrid) = capitals();
        let points = vec![london, paris, berlin, madrid];
        let tree = GeodeticRTree::bulk_load(points.clone());

        let query = coord(0.0, 50.0);
        let radius = 1_000_000.0; // 1000 km

        let mut from_tree: Vec<GeodeticCoord> = tree
            .locate_within_distance(query, radius)
            .map(|p| p.coord())
            .collect();
        let mut from_scan: Vec<GeodeticCoord> = points
            .iter()
            .filter(|p| haversine_distance(p.coord(), query) <= radius)
            .map(|p| p.coord())
            .collect();

        let key = |c: &GeodeticCoord| (c.lon.to_bits(), c.lat.to_bits());
        from_tree.sort_by_key(key);
        from_scan.sort_by_key(key);
        assert_eq!(from_tree, from_scan);
    }

    #[test]
    fn empty_tree_returns_none() {
        // The default type parameter only applies in type position, so annotate to pin
        // the leaf type for a tree never touched by a leaf-typed value.
        let tree: GeodeticRTree = GeodeticRTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.size(), 0);
        assert!(tree.nearest_neighbor(coord(0.0, 0.0)).is_none());
        assert!(tree
            .nearest_neighbor_with_distance(coord(0.0, 0.0))
            .is_none());
        assert!(tree.nearest_neighbors(coord(0.0, 0.0)).is_empty());
    }

    #[test]
    fn insert_remove_and_contains() {
        let mut tree = GeodeticRTree::new();
        let p = GeodeticPoint::new(10.0, 20.0);
        tree.insert(p);
        assert_eq!(tree.size(), 1);
        assert!(tree.contains(&p));
        assert_eq!(tree.remove(&p), Some(p));
        assert!(tree.is_empty());
    }

    fn sorted(mut coords: Vec<GeodeticCoord>) -> Vec<GeodeticCoord> {
        coords.sort_by_key(|c| (c.lon.to_bits(), c.lat.to_bits()));
        coords
    }

    fn rectangle_coords(
        tree: &GeodeticRTree,
        lower: GeodeticCoord,
        upper: GeodeticCoord,
    ) -> Vec<GeodeticCoord> {
        sorted(
            tree.locate_in_rectangle(lower, upper)
                .map(|p| p.coord())
                .collect(),
        )
    }

    #[test]
    fn locate_in_rectangle_returns_points_inside() {
        let (london, paris, berlin, madrid) = capitals();
        let tree = GeodeticRTree::bulk_load(vec![london, paris, berlin, madrid]);

        // A box around London and Paris only.
        let got = rectangle_coords(&tree, coord(-1.0, 48.0), coord(3.0, 52.0));
        assert_eq!(got, sorted(vec![london.coord(), paris.coord()]));
    }

    #[test]
    fn locate_in_rectangle_wraps_across_antimeridian() {
        let near_west = GeodeticPoint::new(179.0, 0.0); // 179°E
        let near_east = GeodeticPoint::new(-178.0, 1.0); // 178°W
        let far = GeodeticPoint::new(0.0, 0.0);
        let tree = GeodeticRTree::bulk_load(vec![near_west, near_east, far]);

        // Wrapping window 170°E -> 170°W spans the seam but not lon 0.
        let got = rectangle_coords(&tree, coord(170.0, -10.0), coord(-170.0, 10.0));
        assert_eq!(got, sorted(vec![near_west.coord(), near_east.coord()]));
    }

    #[test]
    fn locate_in_rectangle_includes_pole_regardless_of_longitude() {
        let pole = GeodeticPoint::new(0.0, 90.0); // north pole, stored lon 0
        let high = GeodeticPoint::new(110.0, 85.0);
        let tree = GeodeticRTree::bulk_load(vec![pole, high]);

        // The longitude band 100°..120° excludes lon 0, but the pole is still in.
        let got = rectangle_coords(&tree, coord(100.0, 80.0), coord(120.0, 90.0));
        assert_eq!(got, sorted(vec![pole.coord(), high.coord()]));
    }

    #[test]
    fn locate_in_rectangle_includes_seam_point_under_either_spelling() {
        // The same meridian, stored under both signs, plus a point just inside.
        let seam_plus = GeodeticPoint::new(180.0, 0.0);
        let seam_minus = GeodeticPoint::new(-180.0, 0.0);
        let inside = GeodeticPoint::new(175.0, 0.0);
        let outside = GeodeticPoint::new(160.0, 0.0);
        let tree = GeodeticRTree::bulk_load(vec![seam_plus, seam_minus, inside, outside]);

        // A non-wrapping window whose east edge is the seam: both seam spellings and
        // the interior point are returned, the point outside the arc is not.
        let got = rectangle_coords(&tree, coord(170.0, -10.0), coord(180.0, 10.0));
        assert_eq!(
            got,
            sorted(vec![seam_plus.coord(), seam_minus.coord(), inside.coord()])
        );
    }

    #[test]
    fn locate_in_rectangle_empty_tree() {
        let tree = GeodeticRTree::<GeodeticPoint>::new();
        assert_eq!(
            tree.locate_in_rectangle(coord(-10.0, -10.0), coord(10.0, 10.0))
                .count(),
            0
        );
    }
}

#[cfg(all(test, feature = "geodetic-wgs84"))]
mod spheroid_tests {
    use approx::assert_relative_eq;
    use hegel::{generators, TestCase};

    use super::GeodeticRTree;
    use crate::geodetic::{
        geodesic_distance, geodesic_distance_wgs84, Ellipsoid, GeodeticCoord, GeodeticPoint,
    };

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

    fn draw_points(tc: &TestCase, n: usize) -> Vec<GeodeticPoint> {
        (0..n)
            .map(|_| GeodeticPoint::new(draw_lon(tc), draw_lat(tc)))
            .collect()
    }

    #[test]
    fn nearest_neighbor_wgs84_returns_nearest_with_geodesic_distance() {
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = GeodeticPoint::new(2.3522, 48.8566);
        let berlin = GeodeticPoint::new(13.4050, 52.5200);
        let madrid = GeodeticPoint::new(-3.7038, 40.4168);
        let tree = GeodeticRTree::bulk_load(vec![london, paris, berlin, madrid]);

        let query = coord(2.0, 49.0);
        let (nn, metres) = tree
            .nearest_neighbor_with_distance_wgs84(query)
            .expect("non-empty");
        assert_eq!(*nn, paris);
        assert_relative_eq!(
            metres,
            geodesic_distance_wgs84(query, paris.coord()),
            epsilon = 1e-6
        );
    }

    #[test]
    fn empty_tree_wgs84_queries_are_empty() {
        let tree: GeodeticRTree = GeodeticRTree::new();
        assert!(tree.nearest_neighbor_wgs84(coord(0.0, 0.0)).is_none());
        assert!(tree
            .nearest_neighbor_with_distance_wgs84(coord(0.0, 0.0))
            .is_none());
        assert_eq!(
            tree.locate_within_distance_wgs84(coord(0.0, 0.0), 1e6)
                .count(),
            0
        );
    }

    /// The `*_wgs84` methods are exactly their `*_on_ellipsoid` counterparts on
    /// [`Ellipsoid::WGS84`], and a different ellipsoid still returns the same nearest
    /// point (the ranking is unchanged at this scale) with a distance matching a direct
    /// geodesic computation on that ellipsoid.
    #[test]
    fn ellipsoid_methods_agree_with_wgs84_wrappers() {
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = GeodeticPoint::new(2.3522, 48.8566);
        let berlin = GeodeticPoint::new(13.4050, 52.5200);
        let madrid = GeodeticPoint::new(-3.7038, 40.4168);
        let tree = GeodeticRTree::bulk_load(vec![london, paris, berlin, madrid]);
        let query = coord(2.0, 49.0);

        let (wgs_nn, wgs_metres) = tree
            .nearest_neighbor_with_distance_wgs84(query)
            .expect("non-empty");
        let (ell_nn, ell_metres) = tree
            .nearest_neighbor_with_distance_on_ellipsoid(query, Ellipsoid::WGS84)
            .expect("non-empty");
        assert_eq!(wgs_nn, ell_nn);
        assert_eq!(wgs_metres, ell_metres);

        // A different ellipsoid: same nearest, distance matching a direct geodesic.
        let (grs_nn, grs_metres) = tree
            .nearest_neighbor_with_distance_on_ellipsoid(query, Ellipsoid::GRS80)
            .expect("non-empty");
        assert_eq!(*grs_nn, paris);
        assert_relative_eq!(
            grs_metres,
            geodesic_distance(query, paris.coord(), Ellipsoid::GRS80),
            epsilon = 1e-6
        );

        // The radius wrapper agrees with the ellipsoid form on WGS84.
        let wgs_within: Vec<_> = tree
            .locate_within_distance_wgs84(query, 100_000.0)
            .collect();
        let ell_within: Vec<_> = tree
            .locate_within_distance_on_ellipsoid(query, 100_000.0, Ellipsoid::WGS84)
            .collect();
        assert_eq!(wgs_within, ell_within);
    }

    /// The branch-and-bound refine returns the same nearest as a brute-force geodesic
    /// scan: this exercises the spherical-filter / geodesic-refine pruning, the part
    /// the spherical lower bound has to get right. (The geodesic distance value itself
    /// is anchored to textbook ellipsoid figures in `spheroid::tests`.)
    #[hegel::test(test_cases = 300)]
    fn prop_nn_wgs84_matches_geodesic_linear_scan(tc: TestCase) {
        let n = (tc.draw(generators::floats::<f64>().min_value(1.0).max_value(30.99)) as usize)
            .clamp(1, 30);
        let points = draw_points(&tc, n);
        let query = coord(draw_lon(&tc), draw_lat(&tc));
        let tree = GeodeticRTree::bulk_load(points.clone());

        let (_, tree_metres) = tree
            .nearest_neighbor_with_distance_wgs84(query)
            .expect("non-empty");
        let scan = points
            .iter()
            .map(|p| geodesic_distance_wgs84(query, p.coord()))
            .fold(f64::INFINITY, f64::min);
        let tol = 1e-3 + scan * 1e-9;
        assert!(
            (tree_metres - scan).abs() <= tol,
            "wgs84 NN {tree_metres} != geodesic scan best {scan}"
        );
    }

    /// The radius refine returns exactly the brute-force geodesic in-range set: the
    /// inflated spherical fetch must drop nothing it should keep.
    #[hegel::test(test_cases = 300)]
    fn prop_locate_within_distance_wgs84_matches_scan(tc: TestCase) {
        let n = (tc.draw(generators::floats::<f64>().min_value(1.0).max_value(40.99)) as usize)
            .clamp(1, 40);
        let points = draw_points(&tc, n);
        let query = coord(draw_lon(&tc), draw_lat(&tc));
        let radius = tc.draw(
            generators::floats::<f64>()
                .min_value(0.0)
                .max_value(10_000_000.0),
        );
        let tree = GeodeticRTree::bulk_load(points.clone());

        let key = |c: &GeodeticCoord| (c.lon.to_bits(), c.lat.to_bits());
        let mut from_tree: Vec<GeodeticCoord> = tree
            .locate_within_distance_wgs84(query, radius)
            .map(|p| p.coord())
            .collect();
        let mut from_scan: Vec<GeodeticCoord> = points
            .iter()
            .filter(|p| geodesic_distance_wgs84(query, p.coord()) <= radius)
            .map(|p| p.coord())
            .collect();
        from_tree.sort_by_key(key);
        from_scan.sort_by_key(key);
        assert_eq!(
            from_tree, from_scan,
            "wgs84 radius set != geodesic scan; query=({},{}) radius={radius}",
            query.lon, query.lat
        );
    }
}
