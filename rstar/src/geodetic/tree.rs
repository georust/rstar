//! The [`Geodetic3DTree`] facade: degrees in, metres out.

use alloc::vec::Vec;

use crate::iterators::{RTreeIterator, RTreeIteratorMut};
use crate::{AABB, ParentNode, RTree};

use super::coord::GeodeticCoord;
use super::distance::{metres_to_squared_chord, squared_chord_to_metres};
use super::embedding::{UnitVec, rectangle_bounding_box, rectangle_contains};
use super::point::GeodeticPoint;

/// A geodetic R-tree over the 3D unit-sphere embedding.
///
/// This facade is the recommended entry point. It takes [`GeodeticCoord`]
/// (degrees) as queries and returns great-circle distances in **metres**, hiding
/// both the internal [`UnitVec`] query type and the opaque squared-chord metric.
/// Its methods mirror the matching [`RTree`] methods, converting at the boundary, so
/// see [`RTree`] for their detailed semantics and complexity; the indexed leaf
/// [`GeodeticPoint`] implements [`crate::RTreeObject`] and [`crate::PointDistance`].
///
/// The embedding maps `(lon, lat)` to a unit vector, so the antimeridian and the
/// poles are ordinary interior points: there is no wrapping, duplication, or
/// frame-shifting to do, and nearest-neighbour pruning uses a true Euclidean
/// lower bound that is monotone in great-circle distance.
#[derive(Clone, Debug, Default)]
pub struct Geodetic3DTree {
    inner: RTree<GeodeticPoint>,
}

impl Geodetic3DTree {
    // --- construction / structure ---

    /// Creates an empty tree.
    pub fn new() -> Self {
        Self {
            inner: RTree::new(),
        }
    }

    /// Bulk-loads a tree from a vector of points (the recommended way to build a
    /// static index).
    pub fn bulk_load(points: Vec<GeodeticPoint>) -> Self {
        Self {
            inner: RTree::bulk_load(points),
        }
    }

    /// Inserts a single point.
    pub fn insert(&mut self, point: GeodeticPoint) {
        self.inner.insert(point);
    }

    /// Removes a point equal to `point` (by embedded vector), returning it if found.
    pub fn remove(&mut self, point: &GeodeticPoint) -> Option<GeodeticPoint> {
        self.inner.remove(point)
    }

    /// Removes a point located exactly at `query` (by embedded vector), returning
    /// it if found.
    pub fn remove_at_point(&mut self, query: GeodeticCoord) -> Option<GeodeticPoint> {
        self.inner.remove_at_point(UnitVec::from(query))
    }

    /// Returns `true` if a point equal to `point` (by embedded vector) is present.
    pub fn contains(&self, point: &GeodeticPoint) -> bool {
        self.inner.contains(point)
    }

    /// Returns the number of points in the tree.
    pub fn size(&self) -> usize {
        self.inner.size()
    }

    /// Returns `true` if the tree contains no points.
    pub fn is_empty(&self) -> bool {
        self.inner.size() == 0
    }

    /// Returns an iterator over all points, in arbitrary order.
    pub fn iter(&self) -> RTreeIterator<'_, GeodeticPoint> {
        self.inner.iter()
    }

    /// Returns a mutable iterator over all points, in arbitrary order.
    ///
    /// Mutating the embedded vector through this iterator can corrupt the index;
    /// it is provided for parity with [`RTree::iter_mut`].
    pub fn iter_mut(&mut self) -> RTreeIteratorMut<'_, GeodeticPoint> {
        self.inner.iter_mut()
    }

    // --- exact-location lookup (no metric) ---

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

    // --- nearest neighbour ---

    /// Returns the nearest point to `query`, or `None` if the tree is empty.
    pub fn nearest_neighbor(&self, query: GeodeticCoord) -> Option<&GeodeticPoint> {
        self.inner.nearest_neighbor(UnitVec::from(query))
    }

    /// Returns the nearest point to `query` together with its great-circle
    /// distance in **metres**, or `None` if the tree is empty.
    pub fn nearest_neighbor_with_distance(
        &self,
        query: GeodeticCoord,
    ) -> Option<(&GeodeticPoint, f64)> {
        self.inner
            .nearest_neighbor_iter_with_distance_2(UnitVec::from(query))
            .next()
            .map(|(p, c2)| (p, squared_chord_to_metres(c2)))
    }

    /// Returns all points sharing the minimum distance to `query` (ties), or an
    /// empty vector if the tree is empty.
    pub fn nearest_neighbors(&self, query: GeodeticCoord) -> Vec<&GeodeticPoint> {
        let q = UnitVec::from(query);
        self.inner.nearest_neighbors(&q)
    }

    /// Returns an iterator over all points in non-decreasing distance order.
    pub fn nearest_neighbor_iter(
        &self,
        query: GeodeticCoord,
    ) -> impl Iterator<Item = &GeodeticPoint> + '_ {
        self.inner.nearest_neighbor_iter(UnitVec::from(query))
    }

    /// Returns an iterator over `(point, distance_in_metres)` in non-decreasing
    /// distance order.
    pub fn nearest_neighbor_iter_with_distance(
        &self,
        query: GeodeticCoord,
    ) -> impl Iterator<Item = (&GeodeticPoint, f64)> + '_ {
        self.inner
            .nearest_neighbor_iter_with_distance_2(UnitVec::from(query))
            .map(|(p, c2)| (p, squared_chord_to_metres(c2)))
    }

    // --- radius query (metres) ---

    /// Returns an iterator over all points within `radius_metres` great-circle
    /// metres of `query`, in arbitrary order.
    pub fn locate_within_distance(
        &self,
        query: GeodeticCoord,
        radius_metres: f64,
    ) -> impl Iterator<Item = &GeodeticPoint> + '_ {
        let threshold = metres_to_squared_chord(radius_metres);
        self.inner
            .locate_within_distance(UnitVec::from(query), threshold)
    }

    // --- window / range query (longitude/latitude rectangle) ---

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
    /// whenever the latitude band reaches it, whatever its longitude.
    ///
    /// This follows the filter/refine scheme PostGIS uses for its `geography` type:
    /// the rectangle is mapped to a 3D bounding box containing its spherical region
    /// (the filter, accelerated by the index), then each candidate is tested against
    /// the exact longitude/latitude predicate (the refine). The result is exactly
    /// the set of indexed points inside the rectangle.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "geodetic")]
    /// # fn main() {
    /// use rstar::geodetic::{Geodetic3DTree, GeodeticCoord, GeodeticPoint};
    ///
    /// let tree = Geodetic3DTree::bulk_load(vec![
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

    // --- advanced read-only traversal ---

    /// Returns the tree's root node for advanced read-only traversal — counting
    /// nodes, inspecting envelopes, or running custom tree algorithms over
    /// [`crate::RTreeNode`] / [`ParentNode`], mirroring [`crate::RTree::root`].
    ///
    /// Node envelopes are `AABB<UnitVec>` in the unit-vector embedding, so extents
    /// and node-level distances are in the squared-chord metric, not metres. Use
    /// [`envelope_distance_metres`] for the great-circle metres from a query to a
    /// node, or [`squared_chord_to_metres`] to convert a raw squared-chord value (a
    /// leaf or `min_max_dist_2`). This exposes structure only: there is no raw-query
    /// entry point that would let the tree be queried in non-metre units.
    pub fn root(&self) -> &ParentNode<GeodeticPoint> {
        self.inner.root()
    }
}

/// Great-circle metres from `query` to the nearest point of a node `envelope`
/// encountered during [`Geodetic3DTree::root`] traversal.
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

    use super::Geodetic3DTree;

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
        let tree = Geodetic3DTree::bulk_load(vec![london, paris, berlin, madrid]);

        let nn = tree.nearest_neighbor(coord(2.0, 49.0)).expect("non-empty");
        assert_eq!(*nn, paris);

        let nn = tree.nearest_neighbor(coord(13.5, 52.0)).expect("non-empty");
        assert_eq!(*nn, berlin);
    }

    #[test]
    fn nearest_neighbor_with_distance_matches_haversine() {
        let (london, paris, berlin, madrid) = capitals();
        let tree = Geodetic3DTree::bulk_load(vec![london, paris, berlin, madrid]);

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
        let tree = Geodetic3DTree::bulk_load(points.clone());

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
        let tree = Geodetic3DTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.size(), 0);
        assert!(tree.nearest_neighbor(coord(0.0, 0.0)).is_none());
        assert!(
            tree.nearest_neighbor_with_distance(coord(0.0, 0.0))
                .is_none()
        );
        assert!(tree.nearest_neighbors(coord(0.0, 0.0)).is_empty());
    }

    #[test]
    fn insert_remove_and_contains() {
        let mut tree = Geodetic3DTree::new();
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
        tree: &Geodetic3DTree,
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
        let tree = Geodetic3DTree::bulk_load(vec![london, paris, berlin, madrid]);

        // A box around London and Paris only.
        let got = rectangle_coords(&tree, coord(-1.0, 48.0), coord(3.0, 52.0));
        assert_eq!(got, sorted(vec![london.coord(), paris.coord()]));
    }

    #[test]
    fn locate_in_rectangle_wraps_across_antimeridian() {
        let near_west = GeodeticPoint::new(179.0, 0.0); // 179°E
        let near_east = GeodeticPoint::new(-178.0, 1.0); // 178°W
        let far = GeodeticPoint::new(0.0, 0.0);
        let tree = Geodetic3DTree::bulk_load(vec![near_west, near_east, far]);

        // Wrapping window 170°E -> 170°W spans the seam but not lon 0.
        let got = rectangle_coords(&tree, coord(170.0, -10.0), coord(-170.0, 10.0));
        assert_eq!(got, sorted(vec![near_west.coord(), near_east.coord()]));
    }

    #[test]
    fn locate_in_rectangle_includes_pole_regardless_of_longitude() {
        let pole = GeodeticPoint::new(0.0, 90.0); // north pole, stored lon 0
        let high = GeodeticPoint::new(110.0, 85.0);
        let tree = Geodetic3DTree::bulk_load(vec![pole, high]);

        // The longitude band 100°..120° excludes lon 0, but the pole is still in.
        let got = rectangle_coords(&tree, coord(100.0, 80.0), coord(120.0, 90.0));
        assert_eq!(got, sorted(vec![pole.coord(), high.coord()]));
    }

    #[test]
    fn locate_in_rectangle_empty_tree() {
        let tree = Geodetic3DTree::new();
        assert_eq!(
            tree.locate_in_rectangle(coord(-10.0, -10.0), coord(10.0, 10.0))
                .count(),
            0
        );
    }
}
