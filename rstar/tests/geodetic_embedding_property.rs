#![cfg(feature = "geodetic")]

//! Property tests for the unit-sphere embedding itself, independent of the tree.
//!
//! Each `#[hegel::test]` body is run many times with `tc.draw(...)`-generated
//! inputs across the full coordinate domain (lon ∈ [-180, 180], lat ∈ [-90, 90])
//! and shrinks any failure to a minimal counterexample.

use hegel::TestCase;
use hegel::generators;

use rstar::PointDistance;
use rstar::geodetic::distance::{haversine_distance, squared_chord_to_metres};
use rstar::geodetic::{GeodeticCoord, GeodeticPoint, UnitVec};

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

// The forward map is unit length to within a few ulp across the whole domain.
#[hegel::test(test_cases = 500)]
fn prop_unit_length(tc: TestCase) {
    let v = coord(draw_lon(&tc), draw_lat(&tc)).to_unit_vector().0;
    let len2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    assert!(
        (len2 - 1.0).abs() <= 1e-12,
        "embedding not unit length: |v|^2 = {len2}"
    );
}

// Away from the poles, the round trip recovers the original coordinate. Near the
// poles longitude is undefined, so latitude is restricted to leave a margin.
#[hegel::test(test_cases = 500)]
fn prop_round_trip_non_polar(tc: TestCase) {
    let lon = draw_lon(&tc);
    let lat = tc.draw(generators::floats::<f64>().min_value(-89.0).max_value(89.0));
    let c = coord(lon, lat);
    let back = GeodeticCoord::from_unit_vector(c.to_unit_vector());
    assert!(
        (back.lat - lat).abs() <= 1e-9,
        "latitude round trip diverged: {} -> {}",
        lat,
        back.lat
    );
    // Longitude can wrap at ±180; compare modulo 360.
    let dlon = ((back.lon - lon + 540.0) % 360.0) - 180.0;
    assert!(
        dlon.abs() <= 1e-9,
        "longitude round trip diverged: {} -> {}",
        lon,
        back.lon
    );
}

// The leaf squared-chord distance, converted to metres, matches haversine across
// the full domain.
#[hegel::test(test_cases = 500)]
fn prop_leaf_distance_matches_haversine(tc: TestCase) {
    let a = coord(draw_lon(&tc), draw_lat(&tc));
    let b = coord(draw_lon(&tc), draw_lat(&tc));
    let leaf = GeodeticPoint::from(a);
    let c2 = leaf.distance_2(&UnitVec::from(b));
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
