//! The geodetic R-tree: degrees in, metres out.

use alloc::vec::Vec;

use crate::iterators::{RTreeIterator, RTreeIteratorMut};
use crate::{ParentNode, PointDistance, RTree, RTreeObject, AABB};

use super::coord::GeodeticCoord;
use super::distance::{metres_to_squared_chord, squared_chord_to_metres};
use super::embedding::{rectangle_bounding_box, rectangle_contains, UnitVec};
use super::point::GeodeticPoint;

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
///   `locate_in_rectangle`), on point trees only.
///
/// The [`locate_in_rectangle`](GeodeticRTree::locate_in_rectangle) window is point-only;
/// it is **not** currently provided for line or polygon extents.
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
    /// Mutating the embedded vectors through this iterator can corrupt the index;
    /// it is provided for parity with [`RTree::iter_mut`].
    pub fn iter_mut(&mut self) -> RTreeIteratorMut<'_, G> {
        self.inner.iter_mut()
    }

    // --- nearest neighbour (metres out) ---

    /// Returns the nearest geometry to `query`, or `None` if the tree is empty.
    pub fn nearest_neighbor(&self, query: GeodeticCoord) -> Option<&G> {
        self.inner.nearest_neighbor(UnitVec::from(query))
    }

    /// Returns the nearest geometry to `query` together with its **minimum** great-circle
    /// distance in **metres** (to the nearest point of the geometry – which for a leaf
    /// type may be on the interior of an edge, not only a vertex), or `None` if the tree
    /// is empty.
    pub fn nearest_neighbor_with_distance(&self, query: GeodeticCoord) -> Option<(&G, f64)> {
        self.inner
            .nearest_neighbor_iter_with_distance_2(UnitVec::from(query))
            .next()
            .map(|(g, c2)| (g, squared_chord_to_metres(c2)))
    }

    /// Returns all geometries sharing the minimum distance to `query` (ties), or an
    /// empty vector if the tree is empty.
    pub fn nearest_neighbors(&self, query: GeodeticCoord) -> Vec<&G> {
        let q = UnitVec::from(query);
        self.inner.nearest_neighbors(&q)
    }

    /// Returns an iterator over all geometries in non-decreasing distance order.
    pub fn nearest_neighbor_iter(&self, query: GeodeticCoord) -> impl Iterator<Item = &G> + '_ {
        self.inner.nearest_neighbor_iter(UnitVec::from(query))
    }

    /// Returns an iterator over `(geometry, distance_in_metres)` in non-decreasing
    /// distance order.
    pub fn nearest_neighbor_iter_with_distance(
        &self,
        query: GeodeticCoord,
    ) -> impl Iterator<Item = (&G, f64)> + '_ {
        self.inner
            .nearest_neighbor_iter_with_distance_2(UnitVec::from(query))
            .map(|(g, c2)| (g, squared_chord_to_metres(c2)))
    }

    // --- radius query (metres) ---

    /// Returns an iterator over all geometries within `radius_metres` great-circle
    /// metres of `query` (by nearest point), in arbitrary order.
    pub fn locate_within_distance(
        &self,
        query: GeodeticCoord,
        radius_metres: f64,
    ) -> impl Iterator<Item = &G> + '_ {
        let threshold = metres_to_squared_chord(radius_metres);
        self.inner
            .locate_within_distance(UnitVec::from(query), threshold)
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

impl GeodeticRTree<GeodeticPoint> {
    // --- exact-location lookup and window query: point-only ---
    //
    // These are specific to point leaves: an exact-location lookup keyed by a single
    // embedded vector, and the longitude/latitude window query, are ill-defined for an
    // extent geometry, which occupies many vectors rather than one.

    /// Removes a point located exactly at `query` (by embedded vector), returning
    /// it if found.
    pub fn remove_at_point(&mut self, query: GeodeticCoord) -> Option<GeodeticPoint> {
        self.inner.remove_at_point(UnitVec::from(query))
    }

    /// Returns a point located exactly at `query` (by embedded vector), if any.
    pub fn locate_at_point(&self, query: GeodeticCoord) -> Option<&GeodeticPoint> {
        self.inner.locate_at_point(UnitVec::from(query))
    }

    /// Returns all points located exactly at `query` (by embedded vector).
    pub fn locate_all_at_point(
        &self,
        query: GeodeticCoord,
    ) -> impl Iterator<Item = &GeodeticPoint> + '_ {
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
    ) -> impl Iterator<Item = &GeodeticPoint> + '_ {
        let bounding_box = rectangle_bounding_box(lower, upper);
        self.inner
            .locate_in_envelope_intersecting(bounding_box)
            .filter(move |point| rectangle_contains(lower, upper, point.coord()))
    }
}

/// Great-circle metres from `query` to the nearest point of a node `envelope`
/// encountered during [`GeodeticRTree::root`] traversal.
///
/// Node envelopes live in the unit-vector embedding, so their `distance_2` is in
/// squared-chord units; this packages the [`UnitVec`] conversion and
/// [`squared_chord_to_metres`] so a traversal can reason in metres. For a leaf or a
/// `min_max_dist_2` value, convert the raw squared chord with
/// [`squared_chord_to_metres`] directly.
pub fn envelope_distance_metres(query: GeodeticCoord, envelope: &AABB<UnitVec>) -> f64 {
    squared_chord_to_metres(envelope.distance_2(&UnitVec::from(query)))
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
        let tree = GeodeticRTree::new();
        assert_eq!(
            tree.locate_in_rectangle(coord(-10.0, -10.0), coord(10.0, 10.0))
                .count(),
            0
        );
    }
}
