//! Great-circle distance helpers: the haversine reference distance and the
//! conversions between the internal squared-chord metric and great-circle metres.

// `haversine_distance` is ported from `rust-geo` (https://github.com/georust/geo),
// MIT OR Apache-2.0, specifically
//   geo/src/algorithm/line_measures/metric_spaces/haversine.rs
// It is inlined rather than depended on to avoid a `geo -> rstar -> geo` dependency
// cycle (rust-geo depends on rstar).

// `Float` provides the trig methods (`sin`, `cos`, `asin`, `sqrt`, ...) on `f64` in
// `no_std` builds, where the inherent `f64` methods are unavailable. Under
// `cfg(test)` the crate links `std` and the inherent methods are used instead,
// leaving this import unused; the `allow` covers that case (a `cfg`-gated import
// does not satisfy `clippy --all-targets`).
#[allow(unused_imports)]
use num_traits::Float;

use super::GeodeticCoord;

/// Earth radius in metres. Matches `geo::MEAN_EARTH_RADIUS` (GRS80 mean radius).
pub const EARTH_RADIUS_METRES: f64 = 6_371_008.8;

/// Great-circle distance in metres between two geodetic coordinates.
/// Haversine formula on a spherical Earth (radius = [`EARTH_RADIUS_METRES`]).
///
/// This is a user-facing reference; the index itself does
/// not call it on the traversal hot path (it uses the squared-chord metric).
///
/// # Units
///
/// - `a`, `b`: [`GeodeticCoord`] values with `lon`/`lat` in degrees
/// - returns: distance in metres
pub fn haversine_distance(a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    let theta1 = a.lat.to_radians();
    let theta2 = b.lat.to_radians();
    let delta_theta = (b.lat - a.lat).to_radians();
    let delta_lambda = (b.lon - a.lon).to_radians();
    let inner = (delta_theta / 2.0).sin().powi(2)
        + theta1.cos() * theta2.cos() * (delta_lambda / 2.0).sin().powi(2);
    // Clamp the `asin` argument to guard its domain against ulp drift past `1.0`
    // for near-antipodal pairs (the `inner` term can round to ~1.0000000000000004,
    // making `sqrt(inner) > 1` and `asin(>1) = NaN`). Matches the idiom in
    // `squared_chord_to_metres` and `from_unit_vector`.
    let c = 2.0 * inner.sqrt().min(1.0).asin();
    EARTH_RADIUS_METRES * c
}

/// Converts the internal squared-chord metric (`c²` in `[0, 4]`) to great-circle
/// metres.
///
/// Uses the half-angle `asin` form, not `acos(1 − c²/2)`: `asin` is well
/// conditioned for small angles, whereas `1 − tiny` rounds to `1` and `acos(1)`
/// loses all small-angle precision. The clamp guards the `asin` domain against ulp
/// drift past `1.0`.
///
/// The facade returns metres, so this is only needed to reason about the internal
/// metric directly.
///
/// A non-finite input (`NaN`) maps to `0.0` metres rather than propagating.
pub fn squared_chord_to_metres(c2: f64) -> f64 {
    let half_chord = (c2.max(0.0).sqrt() * 0.5).clamp(0.0, 1.0); // = sin(d/2)
    EARTH_RADIUS_METRES * 2.0 * half_chord.asin()
}

/// Converts a great-circle radius in metres to a squared-chord threshold (`c²` in
/// `[0, 4]`), suitable as the radius argument to a squared-chord radius query.
///
/// The `.min(PI)` clamp is load-bearing: without it an over-large radius pushes
/// `d/2` past `π/2`, where `sin` decreases, silently shrinking the threshold and
/// dropping points that should match. At `d = π` the threshold is `4.0` (the
/// maximum possible `c²`), so a radius at or beyond the antipode matches every
/// point.
///
/// A non-finite radius (`NaN`) maps to `4.0` (match every point), the
/// over-inclusive direction.
pub fn metres_to_squared_chord(radius_metres: f64) -> f64 {
    if radius_metres <= 0.0 {
        return 0.0;
    }
    let d = (radius_metres / EARTH_RADIUS_METRES).min(core::f64::consts::PI);
    let c = 2.0 * (d * 0.5).sin();
    c * c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geodetic::embedding::squared_chord;
    use approx::assert_relative_eq;
    use hegel::generators;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    // --- haversine_distance ---

    /// Coordinates taken directly from rust-geo's `distance::new_york_to_london` test.
    /// NYC: (-74.006, 40.7128), London: (-0.1278, 51.5074).
    /// Expected rounded value: 5_570_230 m.
    #[test]
    fn haversine_distance_new_york_to_london() {
        let new_york = coord(-74.006, 40.7128);
        let london = coord(-0.1278, 51.5074);
        let d = haversine_distance(new_york, london);
        assert_relative_eq!(d, 5_570_230.0, epsilon = 1.0);
    }

    #[test]
    fn haversine_distance_is_symmetric() {
        let new_york = coord(-74.006, 40.7128);
        let london = coord(-0.1278, 51.5074);
        let d1 = haversine_distance(new_york, london);
        let d2 = haversine_distance(london, new_york);
        assert_relative_eq!(d1, d2, epsilon = 1.0e-6);
    }

    #[test]
    fn haversine_distance_same_point_is_zero() {
        let p = coord(10.0, 20.0);
        assert_relative_eq!(haversine_distance(p, p), 0.0, epsilon = 1e-10);
    }

    /// Regression: a near-antipodal in-range pair drives the `asin` argument
    /// fractionally past `1.0` via floating-point round-off. Without the domain
    /// clamp this returns NaN; it must be finite and close to `π·R`.
    #[test]
    fn haversine_distance_near_antipodal_is_finite() {
        // This specific pair was observed to push `inner` to ~1.0000000000000004.
        let a = coord(-149.383_069_176_055_7, -64.023_570_193_244_86);
        let b = coord(30.615_931_232_891_338, 64.023_569_971_397_32);
        let d = haversine_distance(a, b);
        assert!(
            d.is_finite(),
            "near-antipodal distance must be finite, got {d}"
        );
        // The pair is near- not exactly-antipodal, so the true distance is a little
        // under π·R; the point is that it is finite and within a few hundred metres
        // of the half-circumference rather than NaN.
        assert_relative_eq!(d, HALF_CIRCUMFERENCE, epsilon = 1_000.0);
    }

    /// Exact antipodes (e.g. (0, 0) and (180, 0)) must give `π·R`, not NaN.
    #[test]
    fn haversine_distance_exact_antipode_is_half_circumference() {
        let d = haversine_distance(coord(0.0, 0.0), coord(180.0, 0.0));
        assert!(d.is_finite());
        assert_relative_eq!(d, HALF_CIRCUMFERENCE, epsilon = 1.0);
    }

    // --- squared_chord_to_metres / metres_to_squared_chord ---

    const HALF_CIRCUMFERENCE: f64 = core::f64::consts::PI * EARTH_RADIUS_METRES;

    #[test]
    fn squared_chord_to_metres_matches_haversine() {
        // NYC and London, embedded, then chord -> metres compared to haversine.
        let nyc = coord(-74.006, 40.7128);
        let london = coord(-0.1278, 51.5074);
        let c2 = squared_chord(nyc.to_unit_vector(), london.to_unit_vector());
        let from_chord = squared_chord_to_metres(c2);
        let from_haversine = haversine_distance(nyc, london);
        assert_relative_eq!(from_chord, from_haversine, epsilon = 1.0);
    }

    #[test]
    fn squared_chord_to_metres_endpoints() {
        assert_eq!(squared_chord_to_metres(0.0), 0.0);
        // c² = 4 is the antipode: distance = π·R.
        assert_relative_eq!(
            squared_chord_to_metres(4.0),
            HALF_CIRCUMFERENCE,
            epsilon = 1.0
        );
    }

    #[test]
    fn metres_to_squared_chord_endpoints() {
        assert_eq!(metres_to_squared_chord(0.0), 0.0);
        assert_eq!(metres_to_squared_chord(-100.0), 0.0);
        assert_relative_eq!(
            metres_to_squared_chord(HALF_CIRCUMFERENCE),
            4.0,
            epsilon = 1e-9
        );
        // An over-large radius clamps to the maximum c² of 4.
        assert_relative_eq!(metres_to_squared_chord(1e9), 4.0, epsilon = 1e-12);
    }

    #[test]
    fn metres_to_squared_chord_is_monotone_non_decreasing() {
        let mut prev = metres_to_squared_chord(0.0);
        for i in 1..=200 {
            let r = HALF_CIRCUMFERENCE * (i as f64 / 200.0);
            let c2 = metres_to_squared_chord(r);
            assert!(
                c2 >= prev - 1e-12,
                "metres_to_squared_chord decreased at r={r}: {c2} < {prev}"
            );
            prev = c2;
        }
    }

    // --- property tests, driven by Hegel ---

    #[hegel::test(test_cases = 500)]
    fn prop_metres_chord_round_trip(tc: hegel::TestCase) {
        let r = tc.draw(
            generators::floats::<f64>()
                .min_value(0.0)
                .max_value(HALF_CIRCUMFERENCE),
        );
        let back = squared_chord_to_metres(metres_to_squared_chord(r));
        // Near the antipode (r ≈ π·R) the chord->angle inversion via asin is
        // ill-conditioned because sin(d/2) is flat near 1, so allow a tolerance
        // that scales with r (a few mm at half-circumference).
        let tol = 1e-3 + r * 1e-9;
        assert!(
            (back - r).abs() <= tol,
            "round trip diverged: r={r} back={back}"
        );
    }

    #[hegel::test(test_cases = 500)]
    fn prop_chord_to_metres_matches_haversine(tc: hegel::TestCase) {
        let a = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            ),
            tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
        );
        let b = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            ),
            tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
        );
        let c2 = squared_chord(a.to_unit_vector(), b.to_unit_vector());
        let from_chord = squared_chord_to_metres(c2);
        let from_haversine = haversine_distance(a, b);
        // Tolerance scales with distance: near the antipode both conversions go
        // through a flat asin and diverge by a few mm.
        let tol = 1e-3 + from_haversine * 1e-9;
        assert!(
            (from_chord - from_haversine).abs() <= tol,
            "chord={from_chord} haversine={from_haversine}; a=({},{}) b=({},{})",
            a.lon,
            a.lat,
            b.lon,
            b.lat
        );
    }
}
