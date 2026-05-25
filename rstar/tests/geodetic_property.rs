#![cfg(feature = "geodetic")]

//! Property test: `point_to_mbr_distance` against an independent brute-force oracle,
//! driven by Hegel (property-based testing with automatic shrinking).
//!
//! The oracle computes the great-circle distance from a query to the lat/lon rectangle
//! region (0 if the query is inside it, otherwise the minimum haversine distance to a
//! point on the four parallel/meridian edges). This is the geometry the R-tree's MBR
//! uses (constant-latitude and constant-longitude edges) and is independent of
//! `point_to_mbr_distance`'s internal branch structure.
//!
//! Inputs span the full valid domain (lon in [-180, 180], lat in [-90, 90], any extent
//! including degenerate rectangles), respecting only the documented precondition that an
//! MBR does not wrap across the antimeridian (`lon_l <= lon_h`). `#[hegel::test]` runs the
//! body many times with `tc.draw(...)`-generated inputs and shrinks any failure to a
//! minimal counterexample. See the `mod tests` note in `src/geodetic/distance.rs` for a
//! fuller description of how the Hegel property tests work.

use hegel::generators;

use rstar::geodetic::GeodeticCoord;
use rstar::geodetic::distance::{haversine_distance, point_to_mbr_distance};

fn coord(lon: f64, lat: f64) -> GeodeticCoord {
    GeodeticCoord { lon, lat }
}

/// Minimum haversine distance from `q` to the edge running from `a` to `b` (a parallel
/// or meridian segment, so one coordinate is constant). Found by successive refinement:
/// each pass scans the current `[lo, hi]` window and zooms into the neighbourhood of the
/// best sample. The distance along such an edge is unimodal, so this converges to the
/// true minimum to far below metre precision at any edge length.
fn edge_min(q: GeodeticCoord, a: GeodeticCoord, b: GeodeticCoord) -> f64 {
    const SAMPLES: usize = 1000;
    const PASSES: usize = 3;
    let eval = |t: f64| {
        let lon = a.lon + (b.lon - a.lon) * t;
        let lat = a.lat + (b.lat - a.lat) * t;
        haversine_distance(q, coord(lon, lat))
    };
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    let mut best = f64::MAX;
    for _ in 0..PASSES {
        let mut best_t = lo;
        for i in 0..=SAMPLES {
            let t = lo + (hi - lo) * (i as f64 / SAMPLES as f64);
            let d = eval(t);
            if d < best {
                best = d;
                best_t = t;
            }
        }
        let span = (hi - lo) / SAMPLES as f64;
        lo = (best_t - span).max(0.0);
        hi = (best_t + span).min(1.0);
    }
    best
}

/// Brute-force distance in metres from `q` to the lat/lon rectangle region.
fn brute_region(q: GeodeticCoord, lon_l: f64, lat_l: f64, lon_h: f64, lat_h: f64) -> f64 {
    if lon_l <= q.lon && q.lon <= lon_h && lat_l <= q.lat && q.lat <= lat_h {
        return 0.0;
    }
    let sw = coord(lon_l, lat_l);
    let se = coord(lon_h, lat_l);
    let nw = coord(lon_l, lat_h);
    let ne = coord(lon_h, lat_h);
    edge_min(q, sw, se) // south edge
        .min(edge_min(q, nw, ne)) // north edge
        .min(edge_min(q, sw, nw)) // west edge
        .min(edge_min(q, se, ne)) // east edge
}

/// Draws an ordered pair `(low, high)` within `[min, max]` so the rectangle never wraps
/// across the antimeridian (the documented precondition of `point_to_mbr_distance`).
///
/// Uses [`f64::total_cmp`] rather than `<=`: with `<=`, drawing `+0.0` then `-0.0` would
/// produce the pair `(+0.0, -0.0)`, which then panics a downstream
/// `min_value(+0.0).max_value(-0.0)` generator (IEEE bit-order sees `-0.0 < +0.0`, so
/// the range is empty).
fn ordered(tc: &hegel::TestCase, min: f64, max: f64) -> (f64, f64) {
    let a = tc.draw(generators::floats::<f64>().min_value(min).max_value(max));
    let b = tc.draw(generators::floats::<f64>().min_value(min).max_value(max));
    if a.total_cmp(&b) == std::cmp::Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    }
}

#[hegel::test(test_cases = 500)]
fn point_to_mbr_matches_bruteforce_region(tc: hegel::TestCase) {
    let (lon_l, lon_h) = ordered(&tc, -180.0, 180.0);
    let (lat_l, lat_h) = ordered(&tc, -90.0, 90.0);
    let q = coord(
        tc.draw(
            generators::floats::<f64>()
                .min_value(-180.0)
                .max_value(180.0),
        ),
        tc.draw(generators::floats::<f64>().min_value(-90.0).max_value(90.0)),
    );

    let ours = point_to_mbr_distance(q, lon_l, lat_l, lon_h, lat_h);
    let bf = brute_region(q, lon_l, lat_l, lon_h, lat_h);

    // Valid lower bound: our value must never exceed the true distance. `bf` is an
    // upper bound on the true minimum (it is the distance to a real boundary point), so
    // a violation here means `point_to_mbr_distance` over-estimated -- the bug class that
    // breaks nearest-neighbour pruning.
    assert!(
        ours <= bf + 1e-3,
        "overestimate: ours={ours} bf={bf} q=({},{}) mbr=[{},{}]x[{},{}]",
        q.lon,
        q.lat,
        lon_l,
        lon_h,
        lat_l,
        lat_h
    );
    // Tight: equals the true distance up to the refined oracle's residual error.
    assert!(
        bf - ours <= 1.0,
        "too loose: ours={ours} bf={bf} gap={} q=({},{}) mbr=[{},{}]x[{},{}]",
        bf - ours,
        q.lon,
        q.lat,
        lon_l,
        lon_h,
        lat_l,
        lat_h
    );
}
