use crate::{PointDistance, RTreeObject};

use super::coord::{GeodeticCoord, GeodeticError};
use super::distance::haversine_distance;
use super::envelope::GeodeticEnvelope;

/// A leaf-insertable point in a geodetic R-tree.
///
/// Wraps a [`GeodeticCoord`] and implements [`crate::RTreeObject`] and
/// [`crate::PointDistance`] using great-circle (haversine) distance in metres.
/// Use `rstar::primitives::GeomWithData<GeodeticPoint, T>` to attach data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeodeticPoint(
    /// The wrapped longitude/latitude coordinate (degrees).
    pub GeodeticCoord,
);

impl GeodeticPoint {
    /// Creates a new `GeodeticPoint` from longitude and latitude in degrees.
    ///
    /// `lon` is the longitude (x), in the range `[-180.0, 180.0]`.
    /// `lat` is the latitude (y), in the range `[-90.0, 90.0]`.
    ///
    /// This constructor does **not** validate ranges. Use [`GeodeticPoint::try_new`]
    /// for parsed or external input to catch swapped lat/lon and out-of-range values.
    pub const fn new(lon: f64, lat: f64) -> Self {
        Self(GeodeticCoord { lon, lat })
    }

    /// Creates a `GeodeticPoint` after validating that both components are finite and in
    /// their canonical ranges (`lon` in `[-180, 180]`, `lat` in `[-90, 90]`).
    ///
    /// Use this for parsed or external input. See [`GeodeticError`] for which kinds of
    /// lat/lon swap range validation can and cannot detect.
    pub fn try_new(lon: f64, lat: f64) -> Result<Self, GeodeticError> {
        GeodeticCoord::try_new(lon, lat).map(Self)
    }
}

impl From<GeodeticCoord> for GeodeticPoint {
    fn from(coord: GeodeticCoord) -> Self {
        Self(coord)
    }
}

/// Constructs from `(lon, lat)` in degrees (index 0 = longitude, index 1 = latitude),
/// after the same range validation as [`GeodeticPoint::try_new`].
impl TryFrom<(f64, f64)> for GeodeticPoint {
    type Error = GeodeticError;
    fn try_from((lon, lat): (f64, f64)) -> Result<Self, Self::Error> {
        Self::try_new(lon, lat)
    }
}

/// Constructs from `[lon, lat]` in degrees (index 0 = longitude, index 1 = latitude),
/// after the same range validation as [`GeodeticPoint::try_new`].
impl TryFrom<[f64; 2]> for GeodeticPoint {
    type Error = GeodeticError;
    fn try_from([lon, lat]: [f64; 2]) -> Result<Self, Self::Error> {
        Self::try_new(lon, lat)
    }
}

impl RTreeObject for GeodeticPoint {
    type Envelope = GeodeticEnvelope;

    fn envelope(&self) -> GeodeticEnvelope {
        GeodeticEnvelope {
            lower: self.0,
            upper: self.0,
        }
    }
}

impl PointDistance for GeodeticPoint {
    /// Returns the **un-squared** great-circle distance in metres (despite the
    /// trait method name `distance_2`). This matches `GeodeticEnvelope::distance_2`,
    /// as the trait requires the envelope and point metrics to agree.
    fn distance_2(&self, query: &GeodeticCoord) -> f64 {
        haversine_distance(self.0, *query)
    }

    /// True only when `query` equals the stored coordinate exactly. Because the
    /// comparison is bitwise on `f64`, a query that is *near* the stored point will
    /// not match. For radius-based membership queries use
    /// [`RTree::locate_within_distance`](crate::RTree::locate_within_distance) on the
    /// containing tree instead.
    fn contains_point(&self, query: &GeodeticCoord) -> bool {
        self.0 == *query
    }

    fn distance_2_if_less_or_equal(
        &self,
        query: &GeodeticCoord,
        max_distance_2: f64,
    ) -> Option<f64> {
        let d = haversine_distance(self.0, *query);
        if d <= max_distance_2 { Some(d) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use crate::RTree;
    use crate::RTreeObject;
    use crate::geodetic::coord::GeodeticCoord;
    use crate::geodetic::distance::haversine_distance;

    use super::GeodeticPoint;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    // --- Construction and From conversions ---

    #[test]
    fn new_sets_correct_fields() {
        let p = GeodeticPoint::new(1.0, 2.0);
        assert_eq!(p.0.lon, 1.0);
        assert_eq!(p.0.lat, 2.0);
    }

    #[test]
    fn from_coord_round_trip() {
        let c = coord(10.0, 50.0);
        let p = GeodeticPoint::from(c);
        assert_eq!(p.0, c);
    }

    #[test]
    fn try_from_tuple_maps_index_0_to_lon_1_to_lat() {
        let p = GeodeticPoint::try_from((5.0, 55.0)).expect("valid coordinates");
        assert_eq!(p.0.lon, 5.0, "index 0 of tuple should map to lon");
        assert_eq!(p.0.lat, 55.0, "index 1 of tuple should map to lat");
    }

    #[test]
    fn try_from_array_maps_index_0_to_lon_1_to_lat() {
        let p = GeodeticPoint::try_from([7.0, 53.0]).expect("valid coordinates");
        assert_eq!(p.0.lon, 7.0, "index 0 of array should map to lon");
        assert_eq!(p.0.lat, 53.0, "index 1 of array should map to lat");
    }

    /// `TryFrom` on the unstructured tuple/array forms catches a Tokyo-as-`(lat, lon)`
    /// swap at the conversion site rather than as a downstream wrong-answer query. The
    /// catch only works when `|lon| > 90` (see `GeodeticError` doc); London-style swaps
    /// inside that band cannot be detected by range alone.
    #[test]
    fn try_from_tuple_catches_swapped_lat_lon_for_tokyo() {
        use crate::geodetic::coord::GeodeticError;
        let result = GeodeticPoint::try_from((35.6895, 139.6917));
        assert_eq!(result, Err(GeodeticError::LatOutOfRange(139.6917)));
    }

    #[test]
    fn try_from_array_catches_swapped_lat_lon_for_tokyo() {
        use crate::geodetic::coord::GeodeticError;
        let result = GeodeticPoint::try_from([35.6895, 139.6917]);
        assert_eq!(result, Err(GeodeticError::LatOutOfRange(139.6917)));
    }

    #[test]
    fn try_new_round_trips_valid_input() {
        let p = GeodeticPoint::try_new(-0.1278, 51.5074).expect("valid");
        assert_eq!(p, GeodeticPoint::new(-0.1278, 51.5074));
    }

    // --- Envelope ---

    #[test]
    fn envelope_is_degenerate_point_envelope() {
        use crate::Envelope;
        let p = GeodeticPoint::new(-0.1278, 51.5074);
        let env = p.envelope();
        assert_eq!(env.lower, p.0);
        assert_eq!(env.upper, p.0);
        assert!(env.contains_point(&p.0));
    }

    // --- PointDistance ---

    #[test]
    fn distance_2_matches_haversine_london_paris() {
        use crate::PointDistance;
        // London: (-0.1278, 51.5074), Paris: (2.3522, 48.8566)
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = coord(2.3522, 48.8566);
        let expected = haversine_distance(london.0, paris);
        let got = london.distance_2(&paris);
        assert_relative_eq!(got, expected, epsilon = 1e-6);
    }

    #[test]
    fn contains_point_true_for_same_coord() {
        use crate::PointDistance;
        let p = GeodeticPoint::new(13.4050, 52.5200);
        assert!(p.contains_point(&p.0));
    }

    #[test]
    fn contains_point_false_for_different_coord() {
        use crate::PointDistance;
        let p = GeodeticPoint::new(13.4050, 52.5200);
        assert!(!p.contains_point(&coord(0.0, 0.0)));
    }

    #[test]
    fn distance_2_if_less_or_equal_returns_some_within_bound() {
        use crate::PointDistance;
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = coord(2.3522, 48.8566);
        let d = haversine_distance(london.0, paris);
        let result = london.distance_2_if_less_or_equal(&paris, d + 1.0);
        let dist = result.expect("distance within bound should return Some");
        assert_relative_eq!(dist, d, epsilon = 1e-6);
    }

    #[test]
    fn distance_2_if_less_or_equal_returns_none_beyond_bound() {
        use crate::PointDistance;
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = coord(2.3522, 48.8566);
        let d = haversine_distance(london.0, paris);
        let result = london.distance_2_if_less_or_equal(&paris, d - 1.0);
        assert!(result.is_none());
    }

    // --- End-to-end RTree smoke test ---

    #[test]
    fn rtree_nearest_neighbor_returns_geographically_nearest_city() {
        // Four European capitals.
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = GeodeticPoint::new(2.3522, 48.8566);
        let berlin = GeodeticPoint::new(13.4050, 52.5200);
        let madrid = GeodeticPoint::new(-3.7038, 40.4168);

        let tree = RTree::bulk_load(vec![london, paris, berlin, madrid]);

        // A query near Paris should return Paris.
        let near_paris = coord(2.0, 49.0);
        let nearest = tree
            .nearest_neighbor(near_paris)
            .expect("tree is non-empty");
        assert_eq!(
            *nearest, paris,
            "expected Paris as nearest city to {:?}",
            near_paris
        );

        // A query near Berlin should return Berlin.
        let near_berlin = coord(13.5, 52.0);
        let nearest = tree
            .nearest_neighbor(near_berlin)
            .expect("tree is non-empty");
        assert_eq!(
            *nearest, berlin,
            "expected Berlin as nearest city to {:?}",
            near_berlin
        );
    }
}
