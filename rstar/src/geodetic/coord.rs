// `Float` provides the trig methods (`sin`, `cos`, `asin`, `atan2`, ...) on `f64`
// in `no_std` builds, where the inherent `f64` methods are unavailable. Under
// `cfg(test)` the crate links `std` and the inherent methods are used instead,
// leaving this import unused; the `allow` covers that case.
#[allow(unused_imports)]
use num_traits::Float;

use super::embedding::UnitVec;

/// A geodetic coordinate in degrees: (longitude, latitude).
///
/// Coordinate order matches the `rust-geo`/OGC convention (`x = lon`, `y = lat`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeodeticCoord {
    /// Longitude in degrees, in the range `[-180.0, 180.0]`.
    pub lon: f64,
    /// Latitude in degrees, in the range `[-90.0, 90.0]`.
    pub lat: f64,
}

/// Error returned when a coordinate fails range validation in [`GeodeticCoord::try_new`].
///
/// A common source of [`GeodeticError::LatOutOfRange`] is a swapped lat/lon: any
/// longitude with absolute value greater than `90` (most of Asia, the Pacific, and the
/// Americas west of `-90°`) becomes an out-of-range latitude when written in the wrong
/// slot. Swaps inside `|lon| <= 90` (Europe, Africa) cannot be caught by range checks
/// alone because the value is a valid latitude either way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeodeticError {
    /// Longitude was outside `[-180.0, 180.0]`. Carries the offending value.
    LonOutOfRange(f64),
    /// Latitude was outside `[-90.0, 90.0]`. Carries the offending value.
    LatOutOfRange(f64),
    /// Longitude or latitude was NaN or infinite.
    NotFinite,
}

impl core::fmt::Display for GeodeticError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LonOutOfRange(v) => write!(f, "longitude {v} outside [-180.0, 180.0]"),
            Self::LatOutOfRange(v) => write!(f, "latitude {v} outside [-90.0, 90.0]"),
            Self::NotFinite => write!(f, "longitude or latitude was NaN or infinite"),
        }
    }
}

impl core::error::Error for GeodeticError {}

impl GeodeticCoord {
    /// Creates a coordinate after validating that both components are finite and in
    /// their canonical ranges (`lon` in `[-180, 180]`, `lat` in `[-90, 90]`).
    ///
    /// Use this for parsed or external input. For declared-good data, construct the
    /// struct directly (`GeodeticCoord { lon, lat }`) – that path is infallible.
    pub fn try_new(lon: f64, lat: f64) -> Result<Self, GeodeticError> {
        if !lon.is_finite() || !lat.is_finite() {
            return Err(GeodeticError::NotFinite);
        }
        if !(-180.0..=180.0).contains(&lon) {
            return Err(GeodeticError::LonOutOfRange(lon));
        }
        if !(-90.0..=90.0).contains(&lat) {
            return Err(GeodeticError::LatOutOfRange(lat));
        }
        Ok(Self { lon, lat })
    }

    /// Maps this `(lon, lat)` in degrees to a unit vector on the sphere.
    ///
    /// The forward map is `x = cos φ cos λ`, `y = cos φ sin λ`, `z = sin φ`
    /// (`λ = lon·π/180`, `φ = lat·π/180`), satisfying `x² + y² + z² = 1` in exact
    /// arithmetic. The result is unit length to within a few ulp; do not
    /// renormalise it (renormalising adds error and breaks the exact identity).
    ///
    /// The two degenerate spellings are canonicalised so that equal locations
    /// embed to bit-identical vectors: `lon == -180` embeds as `lon == 180`
    /// (the same meridian), and a pole (`lat == ±90`, where longitude is
    /// undefined) embeds to the exact axis vector `(0, 0, ±1)` regardless of
    /// its longitude. Exact-equality lookups on the embedding therefore treat
    /// these spellings as the same point.
    pub fn to_unit_vector(self) -> UnitVec {
        if self.lat == 90.0 {
            return UnitVec([0.0, 0.0, 1.0]);
        }
        if self.lat == -90.0 {
            return UnitVec([0.0, 0.0, -1.0]);
        }
        let lon = if self.lon == -180.0 { 180.0 } else { self.lon };
        let lambda = lon.to_radians();
        let phi = self.lat.to_radians();
        let (sin_lambda, cos_lambda) = lambda.sin_cos();
        let (sin_phi, cos_phi) = phi.sin_cos();
        UnitVec([cos_phi * cos_lambda, cos_phi * sin_lambda, sin_phi])
    }

    /// Inverse of [`Self::to_unit_vector`]: maps a unit vector back to
    /// `(lon, lat)` in degrees.
    ///
    /// Longitude at a pole is reported as `0` (`atan2(0, 0) = 0`). The `z`
    /// component is clamped to `[-1, 1]` before `asin` to guard the domain
    /// against ulp drift past `1.0`.
    pub fn from_unit_vector(v: UnitVec) -> Self {
        let z = super::clamp_unit(v.0[2]);
        GeodeticCoord {
            lon: v.0[1].atan2(v.0[0]).to_degrees(),
            lat: z.asin().to_degrees(),
        }
    }
}

/// Constructs from `(lon, lat)` in degrees. Infallible (no range validation); use
/// [`GeodeticCoord::try_new`] when the input needs range-checking.
impl From<(f64, f64)> for GeodeticCoord {
    fn from((lon, lat): (f64, f64)) -> Self {
        Self { lon, lat }
    }
}

/// Constructs from `[lon, lat]` in degrees. Infallible, like [`From<(f64, f64)>`].
impl From<[f64; 2]> for GeodeticCoord {
    fn from([lon, lat]: [f64; 2]) -> Self {
        Self { lon, lat }
    }
}

#[cfg(test)]
mod tests {
    use super::{GeodeticCoord, GeodeticError, UnitVec};

    #[test]
    fn try_new_accepts_valid_coord() {
        let c = GeodeticCoord::try_new(-0.1278, 51.5074).expect("valid");
        assert_eq!(c.lon, -0.1278);
        assert_eq!(c.lat, 51.5074);
    }

    #[test]
    fn try_new_accepts_range_boundaries() {
        // ±180 lon and ±90 lat are valid (same meridian / poles).
        assert!(GeodeticCoord::try_new(180.0, 90.0).is_ok());
        assert!(GeodeticCoord::try_new(-180.0, -90.0).is_ok());
    }

    #[test]
    fn try_new_rejects_lon_out_of_range() {
        assert_eq!(
            GeodeticCoord::try_new(180.0001, 0.0),
            Err(GeodeticError::LonOutOfRange(180.0001))
        );
        assert_eq!(
            GeodeticCoord::try_new(-180.0001, 0.0),
            Err(GeodeticError::LonOutOfRange(-180.0001))
        );
    }

    #[test]
    fn try_new_rejects_lat_out_of_range() {
        assert_eq!(
            GeodeticCoord::try_new(0.0, 90.0001),
            Err(GeodeticError::LatOutOfRange(90.0001))
        );
        assert_eq!(
            GeodeticCoord::try_new(0.0, -90.0001),
            Err(GeodeticError::LatOutOfRange(-90.0001))
        );
    }

    #[test]
    fn try_new_rejects_non_finite() {
        assert_eq!(
            GeodeticCoord::try_new(f64::NAN, 0.0),
            Err(GeodeticError::NotFinite)
        );
        assert_eq!(
            GeodeticCoord::try_new(0.0, f64::INFINITY),
            Err(GeodeticError::NotFinite)
        );
        assert_eq!(
            GeodeticCoord::try_new(f64::NEG_INFINITY, 0.0),
            Err(GeodeticError::NotFinite)
        );
    }

    /// The headline value of `try_new`: catching `(lat, lon)` written as `(lon, lat)`
    /// for any location whose `|lon| > 90`. Tokyo is conventionally `(139.6917, 35.6895)`;
    /// the swap puts `139.6917` in the lat slot, which the range check rejects.
    #[test]
    fn try_new_catches_swapped_lat_lon_for_tokyo() {
        assert_eq!(
            GeodeticCoord::try_new(35.6895, 139.6917),
            Err(GeodeticError::LatOutOfRange(139.6917))
        );
    }

    /// Counterpart: a swap entirely inside `|lon| <= 90` (e.g. London) is *not* caught
    /// by range validation, because the value is a valid latitude either way. Pins
    /// this limitation so the docstring's caveat stays accurate.
    #[test]
    fn try_new_does_not_catch_swap_when_lon_within_lat_range() {
        // London: (-0.1278, 51.5074). Both swap orders pass the range check.
        let result = GeodeticCoord::try_new(51.5074, -0.1278);
        assert!(
            result.is_ok(),
            "swap inside |lon|<=90 cannot be caught by ranges"
        );
    }

    #[test]
    fn field_assignment_is_not_swapped() {
        let coord = GeodeticCoord { lon: 1.0, lat: 2.0 };
        assert_eq!(coord.lon, 1.0);
        assert_eq!(coord.lat, 2.0);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let c = GeodeticCoord {
            lon: -0.1278,
            lat: 51.5074,
        };
        let json = serde_json::to_string(&c).expect("serialise");
        let back: GeodeticCoord = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(c, back);
    }

    // --- unit-sphere embedding ---

    use approx::assert_relative_eq;

    fn approx_vec(a: [f64; 3], b: [f64; 3], eps: f64) {
        for i in 0..3 {
            assert_relative_eq!(a[i], b[i], epsilon = eps);
        }
    }

    #[test]
    fn embedding_maps_cardinal_points() {
        // +X = (lon 0, lat 0).
        approx_vec(
            GeodeticCoord { lon: 0.0, lat: 0.0 }.to_unit_vector().0,
            [1.0, 0.0, 0.0],
            1e-15,
        );
        // +Y = (lon 90E, lat 0).
        approx_vec(
            GeodeticCoord {
                lon: 90.0,
                lat: 0.0,
            }
            .to_unit_vector()
            .0,
            [0.0, 1.0, 0.0],
            1e-15,
        );
        // +Z = North pole.
        approx_vec(
            GeodeticCoord {
                lon: 0.0,
                lat: 90.0,
            }
            .to_unit_vector()
            .0,
            [0.0, 0.0, 1.0],
            1e-15,
        );
        // -Z = South pole.
        approx_vec(
            GeodeticCoord {
                lon: 0.0,
                lat: -90.0,
            }
            .to_unit_vector()
            .0,
            [0.0, 0.0, -1.0],
            1e-15,
        );
        // 180E embeds near -X.
        approx_vec(
            GeodeticCoord {
                lon: 180.0,
                lat: 0.0,
            }
            .to_unit_vector()
            .0,
            [-1.0, 0.0, 0.0],
            1e-15,
        );
    }

    #[test]
    fn embedding_is_unit_length() {
        for &(lon, lat) in &[
            (0.0, 0.0),
            (13.4050, 52.5200),
            (-74.006, 40.7128),
            (139.6917, 35.6895),
            (-179.99, -89.0),
            (180.0, 90.0),
        ] {
            let v = GeodeticCoord { lon, lat }.to_unit_vector().0;
            let len2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            assert_relative_eq!(len2, 1.0, epsilon = 1e-15);
        }
    }

    #[test]
    fn embedding_round_trip_non_polar() {
        for &(lon, lat) in &[
            (0.0, 0.0),
            (13.4050, 52.5200),
            (-74.006, 40.7128),
            (139.6917, 35.6895),
            (-0.1278, 51.5074),
        ] {
            let c = GeodeticCoord { lon, lat };
            let back = GeodeticCoord::from_unit_vector(c.to_unit_vector());
            assert_relative_eq!(back.lon, lon, epsilon = 1e-9);
            assert_relative_eq!(back.lat, lat, epsilon = 1e-9);
        }
    }

    #[test]
    fn embedding_pole_inverts_latitude_exactly() {
        // At the pole, latitude inverts to ±90 exactly. Longitude is undefined;
        // the embedding canonicalises any pole spelling to the exact axis vector,
        // so the inverse reports lon = 0 (`atan2(0, 0) = 0`).
        let north = GeodeticCoord {
            lon: 137.0,
            lat: 90.0,
        };
        let back = GeodeticCoord::from_unit_vector(north.to_unit_vector());
        assert_relative_eq!(back.lat, 90.0, epsilon = 1e-9);
        assert_eq!(back.lon, 0.0);
    }

    /// The degenerate spellings canonicalise: every pole spelling embeds to the
    /// exact axis vector, and ±180 (the same meridian) embed bit-identically.
    /// Exact-equality lookups on the embedding depend on this.
    #[test]
    fn embedding_canonicalises_pole_and_seam_spellings() {
        for lon in [-180.0, -137.0, 0.0, 45.0, 180.0] {
            assert_eq!(
                GeodeticCoord { lon, lat: 90.0 }.to_unit_vector().0,
                [0.0, 0.0, 1.0]
            );
            assert_eq!(
                GeodeticCoord { lon, lat: -90.0 }.to_unit_vector().0,
                [0.0, 0.0, -1.0]
            );
        }
        for lat in [-89.0, -10.5, 0.0, 33.0, 89.9] {
            let east = GeodeticCoord { lon: 180.0, lat }.to_unit_vector();
            let west = GeodeticCoord { lon: -180.0, lat }.to_unit_vector();
            assert_eq!(east.0, west.0);
        }
    }

    #[test]
    fn embedding_exact_pole_vector_inverts_to_zero_longitude() {
        // An exactly-axis-aligned pole vector (x = y = 0) inverts to lon = 0 via
        // atan2(0, 0) = 0, the convention for the degenerate case.
        let north = GeodeticCoord::from_unit_vector(UnitVec([0.0, 0.0, 1.0]));
        assert_relative_eq!(north.lat, 90.0, epsilon = 1e-12);
        assert_eq!(north.lon, 0.0);

        let south = GeodeticCoord::from_unit_vector(UnitVec([0.0, 0.0, -1.0]));
        assert_relative_eq!(south.lat, -90.0, epsilon = 1e-12);
        assert_eq!(south.lon, 0.0);
    }

    #[test]
    fn embedding_antimeridian_inverts_to_the_seam() {
        // ±180° denote the same meridian and embed to (almost) the same vector.
        // The inverse reports ±180 depending on the residual sign of the tiny
        // `sin(±π)` term; either value is correct. Latitude is exact.
        for lon in [180.0, -180.0] {
            let back =
                GeodeticCoord::from_unit_vector(GeodeticCoord { lon, lat: 10.0 }.to_unit_vector());
            assert_relative_eq!(back.lon.abs(), 180.0, epsilon = 1e-9);
            assert_relative_eq!(back.lat, 10.0, epsilon = 1e-9);
        }
    }
}
