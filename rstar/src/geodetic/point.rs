use crate::{AABB, PointDistance, RTreeObject};

use super::coord::{GeodeticCoord, GeodeticError};
use super::embedding::{UnitVec, squared_chord};

/// A leaf-insertable point in a geodetic R-tree.
///
/// Stores the original `(lon, lat)` in degrees alongside its precomputed
/// unit-sphere embedding. The embedding is the indexed point; the original
/// coordinate is kept so query results return `lon`/`lat` without an inverse
/// `asin`/`atan2` per result.
///
/// `GeodeticPoint` is deliberately **not** a [`crate::Point`], so the blanket
/// `impl<P: Point> PointDistance for P` does not apply and the custom
/// squared-chord metric below is used instead. Use
/// `rstar::primitives::GeomWithData<GeodeticPoint, T>` to attach data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeodeticPoint {
    // Original degrees, for display / round-trip.
    coord: GeodeticCoord,
    // Precomputed embedding: the indexed point.
    vector: UnitVec,
}

impl GeodeticPoint {
    /// Creates a new `GeodeticPoint` from longitude and latitude in degrees.
    ///
    /// `lon` is the longitude (x), in the range `[-180.0, 180.0]`.
    /// `lat` is the latitude (y), in the range `[-90.0, 90.0]`.
    ///
    /// This constructor does **not** validate ranges. Use [`GeodeticPoint::try_new`]
    /// for parsed or external input to catch swapped lat/lon and out-of-range values.
    ///
    /// Not `const`: the embedding uses trigonometric functions, which are not
    /// available in a `const` context.
    pub fn new(lon: f64, lat: f64) -> Self {
        Self::from(GeodeticCoord { lon, lat })
    }

    /// Creates a `GeodeticPoint` after validating that both components are finite and in
    /// their canonical ranges (`lon` in `[-180, 180]`, `lat` in `[-90, 90]`).
    ///
    /// Use this for parsed or external input. See [`GeodeticError`] for which kinds of
    /// lat/lon swap range validation can and cannot detect.
    pub fn try_new(lon: f64, lat: f64) -> Result<Self, GeodeticError> {
        GeodeticCoord::try_new(lon, lat).map(Self::from)
    }

    /// Returns the original longitude/latitude coordinate (degrees).
    pub fn coord(&self) -> GeodeticCoord {
        self.coord
    }

    /// Returns the precomputed unit-sphere embedding (the indexed point).
    pub fn unit_vec(&self) -> UnitVec {
        self.vector
    }
}

impl From<GeodeticCoord> for GeodeticPoint {
    fn from(coord: GeodeticCoord) -> Self {
        Self {
            coord,
            vector: coord.to_unit_vector(),
        }
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
    type Envelope = AABB<UnitVec>;

    fn envelope(&self) -> AABB<UnitVec> {
        AABB::from_point(self.vector)
    }
}

impl PointDistance for GeodeticPoint {
    /// Squared chord = squared Euclidean between the unit vectors. This is the
    /// **same** metric that [`AABB<UnitVec>::distance_2`](crate::Envelope::distance_2)
    /// returns, so the leaf exact distance and the envelope lower bound share one
    /// code path. Despite the `_2` suffix this is the metric the index orders by,
    /// not a value to be square-rooted; it is monotone in the great-circle distance.
    ///
    /// Do not replace this with a haversine call: for a degenerate (single-point)
    /// box the envelope value must equal the leaf value bit-for-bit, keeping the
    /// default `distance_2_if_less_or_equal` gate exact.
    fn distance_2(&self, query: &UnitVec) -> f64 {
        squared_chord(self.vector, *query)
    }

    // `contains_point` keeps the trait default (`distance_2(q) <= 0`), making
    // containment chord-based: true iff the embedded vectors coincide. This is
    // consistent with the index metric and avoids a second degree-space notion of
    // identity that could disagree with it.
    //
    // `distance_2_if_less_or_equal` keeps the trait default. It gates on
    // `envelope().distance_2(q)` as a lower bound, then computes the exact leaf
    // `distance_2`. For a point leaf the envelope is the point, so the gate is
    // exact, not merely a bound.
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use crate::geodetic::coord::GeodeticCoord;
    use crate::geodetic::distance::{haversine_distance, squared_chord_to_metres};
    use crate::geodetic::embedding::{UnitVec, squared_chord};
    use crate::{Envelope, PointDistance, RTreeObject};

    use super::GeodeticPoint;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    // --- Construction and From conversions ---

    #[test]
    fn new_sets_correct_fields() {
        let p = GeodeticPoint::new(1.0, 2.0);
        assert_eq!(p.coord().lon, 1.0);
        assert_eq!(p.coord().lat, 2.0);
        // The embedding matches the coord's own embedding.
        assert_eq!(p.unit_vec(), coord(1.0, 2.0).to_unit_vector());
    }

    #[test]
    fn from_coord_round_trip() {
        let c = coord(10.0, 50.0);
        let p = GeodeticPoint::from(c);
        assert_eq!(p.coord(), c);
    }

    #[test]
    fn try_from_tuple_maps_index_0_to_lon_1_to_lat() {
        let p = GeodeticPoint::try_from((5.0, 55.0)).expect("valid coordinates");
        assert_eq!(p.coord().lon, 5.0, "index 0 of tuple should map to lon");
        assert_eq!(p.coord().lat, 55.0, "index 1 of tuple should map to lat");
    }

    #[test]
    fn try_from_array_maps_index_0_to_lon_1_to_lat() {
        let p = GeodeticPoint::try_from([7.0, 53.0]).expect("valid coordinates");
        assert_eq!(p.coord().lon, 7.0, "index 0 of array should map to lon");
        assert_eq!(p.coord().lat, 53.0, "index 1 of array should map to lat");
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

    /// London-style swap inside `|lon| <= 90` is not caught by range validation.
    #[test]
    fn try_new_does_not_catch_london_swap() {
        let result = GeodeticPoint::try_new(51.5074, -0.1278);
        assert!(
            result.is_ok(),
            "swap inside |lon|<=90 cannot be caught by ranges"
        );
    }

    #[test]
    fn try_new_round_trips_valid_input() {
        let p = GeodeticPoint::try_new(-0.1278, 51.5074).expect("valid");
        assert_eq!(p, GeodeticPoint::new(-0.1278, 51.5074));
    }

    // --- Envelope ---

    #[test]
    fn envelope_is_degenerate_point_envelope() {
        let p = GeodeticPoint::new(-0.1278, 51.5074);
        let env = p.envelope();
        // The envelope is the embedded vector as a degenerate box.
        assert!(env.contains_point(&p.unit_vec()));
        // distance_2 to the leaf's own vector is zero, and the envelope agrees.
        assert_eq!(env.distance_2(&p.unit_vec()), 0.0);
    }

    // --- PointDistance ---

    #[test]
    fn distance_2_equals_squared_chord() {
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = coord(2.3522, 48.8566).to_unit_vector();
        let expected = squared_chord(london.unit_vec(), paris);
        assert_eq!(london.distance_2(&paris), expected);
    }

    #[test]
    fn distance_2_converted_matches_haversine() {
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = coord(2.3522, 48.8566);
        let c2 = london.distance_2(&paris.to_unit_vector());
        let metres = squared_chord_to_metres(c2);
        let expected = haversine_distance(london.coord(), paris);
        assert_relative_eq!(metres, expected, epsilon = 1e-3);
    }

    #[test]
    fn contains_point_true_only_for_coincident_vector() {
        let p = GeodeticPoint::new(13.4050, 52.5200);
        assert!(p.contains_point(&p.unit_vec()));
        assert!(!p.contains_point(&coord(0.0, 0.0).to_unit_vector()));
    }

    #[test]
    fn distance_2_if_less_or_equal_some_at_boundary() {
        let london = GeodeticPoint::new(-0.1278, 51.5074);
        let paris = coord(2.3522, 48.8566).to_unit_vector();
        let c2 = london.distance_2(&paris);
        // Exactly at the bound: Some.
        let some = london.distance_2_if_less_or_equal(&paris, c2);
        assert_eq!(some, Some(c2));
        // Just below the bound: None.
        let none = london.distance_2_if_less_or_equal(&paris, c2 - 1e-9);
        assert!(none.is_none());
    }

    #[test]
    fn distance_2_to_self_is_zero() {
        let p = GeodeticPoint::new(13.4050, 52.5200);
        assert_eq!(p.distance_2(&p.unit_vec()), 0.0);
    }

    // A query handed a raw UnitVec built from a coord matches the leaf metric.
    #[test]
    fn distance_2_via_explicit_unit_vec() {
        let p = GeodeticPoint::new(0.0, 0.0);
        let q = UnitVec([0.0, 1.0, 0.0]); // (lon 90, lat 0)
        // ‖[1,0,0] − [0,1,0]‖² = 2.
        assert_relative_eq!(p.distance_2(&q), 2.0, epsilon = 1e-15);
    }
}
