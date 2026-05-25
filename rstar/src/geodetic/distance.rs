//! Trigonometric helpers for great-circle calculations: haversine distance,
//! bearing, and cross-track distance.

// Trigonometric helpers (haversine distance, bearing, cross-track distance) are ported
// from `rust-geo` (https://github.com/georust/geo), MIT OR Apache-2.0, specifically
//   geo/src/algorithm/line_measures/metric_spaces/haversine.rs
//   geo/src/algorithm/cross_track_distance.rs
// They are inlined rather than depended on to avoid a `geo -> rstar -> geo` dependency
// cycle (rust-geo depends on rstar).

// `Float` provides the trig methods (`sin`, `cos`, `atan2`, ...) on `f64` in `no_std`
// builds, where the inherent `f64` methods are unavailable. Under `cfg(test)` the crate
// links `std` and the inherent methods are used instead, leaving this import unused; the
// `allow` covers that case (a `cfg`-gated import does not satisfy `clippy --all-targets`).
#[allow(unused_imports)]
use num_traits::Float;

use super::GeodeticCoord;

/// Earth radius in metres. Matches `geo::MEAN_EARTH_RADIUS` (GRS80 mean radius).
pub const EARTH_RADIUS_METRES: f64 = 6_371_008.8;

// Derived from `EARTH_RADIUS_METRES`: circumference = 2π × r.
const EARTH_CIRCUMFERENCE_METRES: f64 = 2.0 * core::f64::consts::PI * EARTH_RADIUS_METRES;

/// Returns `x` reduced to the range `[0, 360)`.
#[inline]
fn mod360(x: f64) -> f64 {
    let m = x % 360.0;
    if m < 0.0 { m + 360.0 } else { m }
}

// `x` is in degrees.
#[inline]
fn tan_deg(x: f64) -> f64 {
    x.to_radians().tan()
}

// `x` is in degrees.
#[inline]
fn cos_deg(x: f64) -> f64 {
    x.to_radians().cos()
}

/// Great-circle distance in metres between two geodetic coordinates.
/// Haversine formula on a spherical Earth (radius = [`EARTH_RADIUS_METRES`]).
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
    let c = 2.0 * inner.sqrt().asin();
    EARTH_RADIUS_METRES * c
}

/// Initial bearing (azimuth) in degrees in `[0, 360)` from `a` to `b`.
///
/// North is 0°, East is 90°, South is 180°, West is 270°.
///
/// # Units
///
/// - `a`, `b`: [`GeodeticCoord`] values with `lon`/`lat` in degrees
/// - returns: bearing in degrees, in the range `[0, 360)`
pub fn bearing(a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    let (lng_a, lat_a) = (a.lon.to_radians(), a.lat.to_radians());
    let (lng_b, lat_b) = (b.lon.to_radians(), b.lat.to_radians());
    let delta_lng = lng_b - lng_a;
    let s = lat_b.cos() * delta_lng.sin();
    let c = lat_a.cos() * lat_b.sin() - lat_a.sin() * lat_b.cos() * delta_lng.cos();
    let degrees = s.atan2(c).to_degrees();
    mod360(degrees)
}

/// Cross-track distance in metres: the shortest distance from `query` to the great-circle
/// line through `line_a` and `line_b`. Always non-negative.
///
/// # Units
///
/// - `query`, `line_a`, `line_b`: [`GeodeticCoord`] values with `lon`/`lat` in degrees
/// - returns: distance in metres
pub fn cross_track_distance(
    query: GeodeticCoord,
    line_a: GeodeticCoord,
    line_b: GeodeticCoord,
) -> f64 {
    let l_delta_13 = haversine_distance(line_a, query) / EARTH_RADIUS_METRES;
    let theta_13 = bearing(line_a, query).to_radians();
    let theta_12 = bearing(line_a, line_b).to_radians();
    let l_delta_xt = (l_delta_13.sin() * (theta_12 - theta_13).sin()).asin();
    EARTH_RADIUS_METRES * l_delta_xt.abs()
}

/// The paper's optimised Algorithm 2: shortest great-circle distance in metres from
/// `query` to the geodetic rectangle `[lon_l..lon_h] x [lat_l..lat_h]` (degrees).
///
/// Returns 0 if the query is inside the rectangle.
///
/// Does not handle MBRs that cross the antimeridian (`lon_l > lon_h`). Callers must
/// either duplicate items spanning +/-180 degrees or split such MBRs at insertion time.
///
/// # Units
///
/// - `query`: [`GeodeticCoord`] with `lon`/`lat` in degrees
/// - `lon_l`, `lat_l`, `lon_h`, `lat_h`: bounding rectangle bounds in degrees
/// - returns: distance in metres
pub fn point_to_mbr_distance(
    query: GeodeticCoord,
    lon_l: f64,
    lat_l: f64,
    lon_h: f64,
    lat_h: f64,
) -> f64 {
    let GeodeticCoord {
        lon: lon_q,
        lat: lat_q,
    } = query;
    let circumference = EARTH_CIRCUMFERENCE_METRES;

    // Branch A: query longitude is within the MBR's longitude band, so the
    // nearest MBR point lies on the same meridian as the query and the shortest
    // path is a meridian arc. `circumference * Δφ / 360.0` is the degree-form of
    // `r · Δφ` (radians) — exact under the spherical model, not an approximation.
    if lon_l <= lon_q && lon_q <= lon_h {
        if lat_q < lat_l {
            return circumference * (lat_l - lat_q) / 360.0; // South
        }
        if lat_q > lat_h {
            return circumference * (lat_q - lat_h) / 360.0; // North
        }
        return 0.0; // Inside
    }

    // Branch B: query is east or west of the MBR. Decide which by shorter angular delta.
    let west = mod360(lon_l - lon_q) <= mod360(lon_q - lon_h);

    if west {
        let tau = tan_deg(lat_q);

        // Separations of 90 degrees or more put the perpendicular foot off the
        // meridian arc, so the nearest point on the western edge is one of the two
        // corners. Algorithm 2 picks between them with the mid-parallel heuristic:
        // tan(lat_q) vs tan(mid_lat) * cos(Δlon). Matches ELKI's `latlngMinDistRad`.
        if mod360(lon_l - lon_q) >= 90.0 {
            let mid_tan = tan_deg((lat_l + lat_h) * 0.5);
            let target_lat = if tau >= mid_tan * cos_deg(lon_l - lon_q) {
                lat_h
            } else {
                lat_l
            };
            return haversine_distance(
                query,
                GeodeticCoord {
                    lon: lon_l,
                    lat: target_lat,
                },
            );
        }

        if tau >= tan_deg(lat_h) * cos_deg(lon_l - lon_q) {
            return haversine_distance(
                query,
                GeodeticCoord {
                    lon: lon_l,
                    lat: lat_h,
                },
            ); // NW corner
        }
        if tau <= tan_deg(lat_l) * cos_deg(lon_l - lon_q) {
            return haversine_distance(
                query,
                GeodeticCoord {
                    lon: lon_l,
                    lat: lat_l,
                },
            ); // SW corner
        }
        cross_track_distance(
            query,
            GeodeticCoord {
                lon: lon_l,
                lat: lat_l,
            },
            GeodeticCoord {
                lon: lon_l,
                lat: lat_h,
            },
        ) // West edge
    } else {
        let tau = tan_deg(lat_q);

        // Symmetric to the western large-Δlon branch: pick NE vs SE via the
        // mid-parallel heuristic.
        if mod360(lon_q - lon_h) >= 90.0 {
            let mid_tan = tan_deg((lat_l + lat_h) * 0.5);
            let target_lat = if tau >= mid_tan * cos_deg(lon_h - lon_q) {
                lat_h
            } else {
                lat_l
            };
            return haversine_distance(
                query,
                GeodeticCoord {
                    lon: lon_h,
                    lat: target_lat,
                },
            );
        }

        if tau >= tan_deg(lat_h) * cos_deg(lon_h - lon_q) {
            return haversine_distance(
                query,
                GeodeticCoord {
                    lon: lon_h,
                    lat: lat_h,
                },
            ); // NE corner
        }
        if tau <= tan_deg(lat_l) * cos_deg(lon_h - lon_q) {
            return haversine_distance(
                query,
                GeodeticCoord {
                    lon: lon_h,
                    lat: lat_l,
                },
            ); // SE corner
        }
        cross_track_distance(
            query,
            GeodeticCoord {
                lon: lon_h,
                lat: lat_l,
            },
            GeodeticCoord {
                lon: lon_h,
                lat: lat_h,
            },
        ) // East edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use hegel::generators;

    fn coord(lon: f64, lat: f64) -> GeodeticCoord {
        GeodeticCoord { lon, lat }
    }

    /// Draws an ordered pair `(low, high)` within `[min, max]` so a generated MBR never
    /// wraps across the antimeridian (the documented precondition of
    /// [`point_to_mbr_distance`]). Degenerate (`low == high`) pairs are allowed.
    ///
    /// Uses [`f64::total_cmp`] rather than `<=`: with `<=`, drawing `+0.0` then `-0.0`
    /// would produce the pair `(+0.0, -0.0)`, which then panics a downstream
    /// `min_value(+0.0).max_value(-0.0)` generator (IEEE bit-order sees `-0.0 < +0.0`,
    /// so the range is empty).
    fn ordered(tc: &hegel::TestCase, min: f64, max: f64) -> (f64, f64) {
        let a = tc.draw(generators::floats::<f64>().min_value(min).max_value(max));
        let b = tc.draw(generators::floats::<f64>().min_value(min).max_value(max));
        if a.total_cmp(&b) == core::cmp::Ordering::Greater {
            (b, a)
        } else {
            (a, b)
        }
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

    // --- bearing ---

    /// Cardinal-direction cases from rust-geo's `bearing` tests.
    #[test]
    fn bearing_north() {
        let origin = coord(0.0, 0.0);
        let destination = coord(0.0, 1.0);
        assert_relative_eq!(bearing(origin, destination), 0.0);
    }

    #[test]
    fn bearing_east() {
        let origin = coord(0.0, 0.0);
        let destination = coord(1.0, 0.0);
        assert_relative_eq!(bearing(origin, destination), 90.0);
    }

    #[test]
    fn bearing_south() {
        let origin = coord(0.0, 0.0);
        let destination = coord(0.0, -1.0);
        assert_relative_eq!(bearing(origin, destination), 180.0);
    }

    #[test]
    fn bearing_west() {
        let origin = coord(0.0, 0.0);
        let destination = coord(-1.0, 0.0);
        assert_relative_eq!(bearing(origin, destination), 270.0);
    }

    // --- cross_track_distance ---

    /// Point on the great-circle line: distance should be approximately zero.
    /// Coordinates from rust-geo's `cross_track_distance_to_line_passing_through_point`.
    #[test]
    fn cross_track_distance_point_on_line() {
        let p = coord(0.0, 0.0);
        let line_a = coord(1.0, 0.0);
        let line_b = coord(2.0, 0.0);
        assert_relative_eq!(
            cross_track_distance(p, line_a, line_b),
            0.0,
            epsilon = 1.0e-6
        );
    }

    /// Point orthogonal to the line: distance should match haversine distance to
    /// the foot of the perpendicular.
    /// Coordinates from rust-geo's `cross_track_distance_to_line_orthogonal_to_point`.
    #[test]
    fn cross_track_distance_orthogonal() {
        let p = coord(0.0, 0.0);
        let line_a = coord(1.0, -1.0);
        let line_b = coord(1.0, 1.0);
        let expected = haversine_distance(p, coord(1.0, 0.0));
        assert_relative_eq!(
            cross_track_distance(p, line_a, line_b),
            expected,
            epsilon = 1.0e-6
        );
        // Also test reversed line direction.
        assert_relative_eq!(
            cross_track_distance(p, line_b, line_a),
            expected,
            epsilon = 1.0e-6
        );
    }

    /// Tight numeric test from rust-geo's `distance1_test`.
    /// p = (-0.7972, 53.2611), line_a = (-1.7297, 53.3206), line_b = (0.1334, 53.1887).
    /// Expected: 307.549995 m.
    #[test]
    fn cross_track_distance1() {
        let p = coord(-0.7972, 53.2611);
        let line_a = coord(-1.7297, 53.3206);
        let line_b = coord(0.1334, 53.1887);
        assert_relative_eq!(
            cross_track_distance(p, line_a, line_b),
            307.549_995,
            epsilon = 1.0e-6
        );
    }

    /// NYC to line through Miami–Washington.
    /// Coordinates from rust-geo's `new_york_to_line_between_miami_and_washington`.
    /// NYC: (-74.006, 40.7128), Miami: (-80.1918, 25.7617), Washington: (-120.7401, 47.7511).
    /// Expected: 1_547_104 m.
    #[test]
    fn cross_track_distance_nyc_miami_washington() {
        let nyc = coord(-74.006, 40.7128);
        let miami = coord(-80.1918, 25.7617);
        let washington = coord(-120.7401, 47.7511);
        assert_relative_eq!(
            cross_track_distance(nyc, miami, washington),
            1_547_104.0,
            epsilon = 1.0
        );
    }

    // --- point_to_mbr_distance ---
    //
    // These tests verify branch/closest-feature selection but reuse the same internal
    // helpers the algorithm calls. The independent oracle (refined edge sweep, not
    // dependent on this module's branch structure) lives in
    // `tests/geodetic_property.rs::point_to_mbr_matches_bruteforce_region`.
    //
    // MBR: lon_l=10, lat_l=40, lon_h=20, lat_h=50

    #[test]
    fn mbr_inside() {
        assert_relative_eq!(
            point_to_mbr_distance(coord(15.0, 45.0), 10.0, 40.0, 20.0, 50.0),
            0.0,
            epsilon = 1e-9
        );
    }

    #[test]
    fn mbr_north() {
        let query = coord(15.0, 60.0);
        let expected = haversine_distance(coord(15.0, 50.0), coord(15.0, 60.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_south() {
        let query = coord(15.0, 30.0);
        let expected = haversine_distance(coord(15.0, 40.0), coord(15.0, 30.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_east_edge() {
        let query = coord(30.0, 45.0);
        let expected =
            cross_track_distance(coord(30.0, 45.0), coord(20.0, 40.0), coord(20.0, 50.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_west_edge() {
        let query = coord(0.0, 45.0);
        let expected = cross_track_distance(coord(0.0, 45.0), coord(10.0, 40.0), coord(10.0, 50.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_nw_corner() {
        let query = coord(0.0, 60.0);
        let expected = haversine_distance(coord(0.0, 60.0), coord(10.0, 50.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_ne_corner() {
        let query = coord(30.0, 60.0);
        let expected = haversine_distance(coord(30.0, 60.0), coord(20.0, 50.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_sw_corner() {
        let query = coord(0.0, 30.0);
        let expected = haversine_distance(coord(0.0, 30.0), coord(10.0, 40.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    #[test]
    fn mbr_se_corner() {
        let query = coord(30.0, 30.0);
        let expected = haversine_distance(coord(30.0, 30.0), coord(20.0, 40.0));
        assert_relative_eq!(
            point_to_mbr_distance(query, 10.0, 40.0, 20.0, 50.0),
            expected,
            epsilon = 1e-3
        );
    }

    // --- property tests (algebraic invariants), driven by Hegel ---
    //
    // How these work: unlike the example-based tests above (which assert fixed inputs),
    // each `#[hegel::test]` function is a *property* that must hold for every input. The
    // body is run many times; `tc.draw(generators::...)` pulls each value from Hegel
    // rather than from a literal or a seeded RNG. When a run fails, Hegel automatically
    // *shrinks* the input to a minimal counterexample (e.g. the smallest MBR/query that
    // breaks the invariant) instead of reporting whatever random case happened to trip it
    // – that shrinking is the main reason to prefer this over a fixed-seed `rand` loop.
    //
    // Generators span the full valid domain (lon in [-180, 180], lat in [-90, 90], any
    // extent including degenerate rectangles). The one precondition of
    // `point_to_mbr_distance` – `lon_l <= lon_h` (no antimeridian wrap) – is satisfied by
    // construction: `ordered` draws two bounds and swaps them, rather than rejection-
    // sampling (which Hegel would flag as discarding too many cases).
    //
    // These run under plain `cargo test --features geodetic`; the independent oracle test
    // in `tests/geodetic_property.rs` complements them. `hegeltest` is a dev-dependency
    // (so it does not affect the published MSRV); in CI the failure database is disabled
    // and runs are derandomised automatically, and Hegel's local state lives in a
    // git-ignored `.hegel/` directory.

    #[hegel::test(test_cases = 500)]
    fn prop_distance_is_nonneg_and_bounded(tc: hegel::TestCase) {
        let half_circumference = std::f64::consts::PI * EARTH_RADIUS_METRES;
        let (lon_l, lon_h) = ordered(&tc, -180.0, 180.0);
        let (lat_l, lat_h) = ordered(&tc, -90.0, 90.0);
        let query = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            ),
            tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
        );

        let ours = point_to_mbr_distance(query, lon_l, lat_l, lon_h, lat_h);
        assert!(
            ours >= 0.0 && ours <= half_circumference + 1.0,
            "distance {ours} out of range [0, {half_circumference}+1]; \
             query=({},{}), mbr=[{lon_l},{lat_l}]-[{lon_h},{lat_h}]",
            query.lon,
            query.lat
        );
    }

    #[hegel::test(test_cases = 500)]
    fn prop_distance_le_every_corner(tc: hegel::TestCase) {
        let (lon_l, lon_h) = ordered(&tc, -180.0, 180.0);
        let (lat_l, lat_h) = ordered(&tc, -90.0, 90.0);
        let query = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            ),
            tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
        );

        let ours = point_to_mbr_distance(query, lon_l, lat_l, lon_h, lat_h);
        for c in [
            coord(lon_l, lat_l),
            coord(lon_l, lat_h),
            coord(lon_h, lat_l),
            coord(lon_h, lat_h),
        ] {
            let corner_dist = haversine_distance(query, c);
            assert!(
                ours <= corner_dist + 1e-3,
                "ours={ours} > corner_dist={corner_dist} + 1e-3; corner=({},{}) \
                 query=({},{}), mbr=[{lon_l},{lat_l}]-[{lon_h},{lat_h}]",
                c.lon,
                c.lat,
                query.lon,
                query.lat
            );
        }
    }

    #[hegel::test(test_cases = 500)]
    fn prop_degenerate_mbr_equals_haversine(tc: hegel::TestCase) {
        let p = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            ),
            tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
        );
        let query = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(-180.0)
                    .max_value(180.0),
            ),
            tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
        );

        let ours = point_to_mbr_distance(query, p.lon, p.lat, p.lon, p.lat);
        let expected = haversine_distance(query, p);
        assert!(
            (ours - expected).abs() <= 1e-6,
            "ours={ours} != haversine={expected}; query=({},{}), degenerate_mbr=({},{})",
            query.lon,
            query.lat,
            p.lon,
            p.lat
        );
    }

    #[hegel::test(test_cases = 500)]
    fn prop_inside_is_zero(tc: hegel::TestCase) {
        let (lon_l, lon_h) = ordered(&tc, -180.0, 180.0);
        let (lat_l, lat_h) = ordered(&tc, -90.0, 90.0);
        // Draw a query inside (or on the boundary of) the rectangle.
        let query = coord(
            tc.draw(
                generators::floats::<f64>()
                    .min_value(lon_l)
                    .max_value(lon_h),
            ),
            tc.draw(
                generators::floats::<f64>()
                    .min_value(lat_l)
                    .max_value(lat_h),
            ),
        );

        let ours = point_to_mbr_distance(query, lon_l, lat_l, lon_h, lat_h);
        assert_eq!(
            ours, 0.0,
            "expected 0.0 for interior query ({},{}) in \
             mbr=[{lon_l},{lat_l}]-[{lon_h},{lat_h}], got {ours}",
            query.lon, query.lat
        );
    }

    // Large delta-longitude (>= 90 degrees) regression tests. At these separations a
    // higher-latitude corner can be closer than a lower-latitude one, even though
    // the latter looks "closer in latitude" – the great-circle wraps. Algorithm 2's
    // mid-parallel heuristic must pick the nearer corner; these tests pin both
    // possible outcomes and guard against future regressions to the heuristic.

    // Pole regression tests. Bearing is undefined at a pole, but the algorithm never
    // calls `bearing(query, _)`: inside `cross_track_distance` the bearings are
    // `bearing(line_a, query)` and `bearing(line_a, line_b)`, both originating at
    // an edge endpoint (a non-polar MBR corner). These tests pin that property.

    #[test]
    fn mbr_query_at_north_pole_is_safe() {
        let q = coord(0.0, 90.0);
        let d = point_to_mbr_distance(q, 10.0, 40.0, 20.0, 50.0);
        let nearest_corner = haversine_distance(q, coord(10.0, 50.0))
            .min(haversine_distance(q, coord(20.0, 50.0)))
            .min(haversine_distance(q, coord(10.0, 40.0)))
            .min(haversine_distance(q, coord(20.0, 40.0)));
        assert!(d.is_finite() && d >= 0.0);
        assert!(d <= nearest_corner + 1e-3);
    }

    #[test]
    fn mbr_query_at_south_pole_is_safe() {
        let q = coord(0.0, -90.0);
        let d = point_to_mbr_distance(q, 10.0, 40.0, 20.0, 50.0);
        let nearest_corner = haversine_distance(q, coord(10.0, 50.0))
            .min(haversine_distance(q, coord(20.0, 50.0)))
            .min(haversine_distance(q, coord(10.0, 40.0)))
            .min(haversine_distance(q, coord(20.0, 40.0)));
        assert!(d.is_finite() && d >= 0.0);
        assert!(d <= nearest_corner + 1e-3);
    }

    #[test]
    fn mbr_query_at_pole_within_lon_band_uses_meridian_arc() {
        // Query lon falls inside the MBR's lon band, so Branch A fires and we
        // return the meridian-arc length from the pole to lat_h.
        let q = coord(15.0, 90.0);
        let d = point_to_mbr_distance(q, 10.0, 40.0, 20.0, 50.0);
        let expected = haversine_distance(q, coord(15.0, 50.0));
        assert_relative_eq!(d, expected, epsilon = 1e-3);
    }

    #[test]
    fn mbr_west_large_delta_lon() {
        // Query > 90 degrees west of the MBR's western edge (mod360(10 - (-85)) == 95).
        let q = coord(-85.0, 45.0);
        let nw = haversine_distance(q, coord(10.0, 50.0));
        let sw = haversine_distance(q, coord(10.0, 40.0));
        assert!(nw < sw, "NW should be the nearer corner for this query");
        let d = point_to_mbr_distance(q, 10.0, 40.0, 20.0, 50.0);
        assert_relative_eq!(d, nw, epsilon = 1e-3);
        assert!(d < sw, "must not return the farther (SW) corner");
    }

    #[test]
    fn mbr_east_large_delta_lon() {
        // Query > 90 degrees east of the MBR's eastern edge (mod360(125 - 20) == 105).
        let q = coord(125.0, 45.0);
        let ne = haversine_distance(q, coord(20.0, 50.0));
        let se = haversine_distance(q, coord(20.0, 40.0));
        assert!(ne < se, "NE should be the nearer corner for this query");
        let d = point_to_mbr_distance(q, 10.0, 40.0, 20.0, 50.0);
        assert_relative_eq!(d, ne, epsilon = 1e-3);
        assert!(d < se, "must not return the farther (SE) corner");
    }
}
