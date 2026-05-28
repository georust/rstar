#![cfg(feature = "geodetic")]

//! End-to-end integration tests for [`Geodetic3DTree`].
//!
//! These exercise nearest-neighbour correctness against a brute-force scan, sorted
//! iterator ordering, and the previously-impossible edge cases the 2D design could
//! not handle: the antimeridian, the poles, antipodal points, and coincident
//! points. The rand-driven scans complement the Hegel property tests.

use approx::assert_relative_eq;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;

use rstar::geodetic::distance::{EARTH_RADIUS_METRES, haversine_distance};
use rstar::geodetic::{Geodetic3DTree, GeodeticCoord, GeodeticPoint};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn coord(lon: f64, lat: f64) -> GeodeticCoord {
    GeodeticCoord { lon, lat }
}

fn random_points(rng: &mut StdRng, n: usize) -> Vec<GeodeticPoint> {
    (0..n)
        .map(|_| {
            // Full domain, including the antimeridian and the poles.
            let lon: f64 = rng.random_range(-180.0_f64..180.0_f64);
            let lat: f64 = rng.random_range(-90.0_f64..90.0_f64);
            GeodeticPoint::new(lon, lat)
        })
        .collect()
}

fn random_queries(rng: &mut StdRng, n: usize) -> Vec<GeodeticCoord> {
    (0..n)
        .map(|_| {
            let lon: f64 = rng.random_range(-180.0_f64..180.0_f64);
            let lat: f64 = rng.random_range(-90.0_f64..90.0_f64);
            coord(lon, lat)
        })
        .collect()
}

// Asserts the tree built from `points` agrees with a brute-force haversine scan for
// both `nearest_neighbor` and `locate_within_distance` at the given query/radius.
// Used by the deterministic pole- and seam-cluster tests, where the dataset is sized
// to force a real internal node (see those tests).
fn assert_matches_brute_force(points: &[GeodeticPoint], query: GeodeticCoord, radius_metres: f64) {
    let tree = Geodetic3DTree::bulk_load(points.to_vec());

    // nearest_neighbor distance equals the brute-force minimum.
    let nn = tree.nearest_neighbor(query).expect("non-empty tree");
    let brute_min = points
        .iter()
        .map(|p| haversine_distance(p.coord(), query))
        .fold(f64::MAX, f64::min);
    assert_relative_eq!(
        haversine_distance(nn.coord(), query),
        brute_min,
        epsilon = 1e-3
    );

    // locate_within_distance set-equals the brute-force filter, allowing a small band
    // around the threshold for the metre<->chord round-trip (see the property test).
    const BAND: f64 = 1.0;
    let key = |c: &GeodeticCoord| (c.lon.to_bits(), c.lat.to_bits());
    let mut from_tree: Vec<GeodeticCoord> = tree
        .locate_within_distance(query, radius_metres)
        .map(|p| p.coord())
        .collect();
    from_tree.sort_by_key(key);

    for c in &from_tree {
        let d = haversine_distance(*c, query);
        assert!(
            d <= radius_metres + BAND,
            "returned point at {d} m exceeds {radius_metres} m"
        );
    }
    for p in points {
        let d = haversine_distance(p.coord(), query);
        if d <= radius_metres - BAND {
            let c = p.coord();
            assert!(
                from_tree.binary_search_by_key(&key(&c), key).is_ok(),
                "point at {d} m (radius {radius_metres} m) missing from result"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 1: nearest_neighbor correctness against brute force
// ---------------------------------------------------------------------------

#[test]
fn nearest_neighbor_matches_brute_force() {
    let mut rng = StdRng::seed_from_u64(0x1234_5678_ABCD_EF01);
    let points = random_points(&mut rng, 1000);
    let tree = Geodetic3DTree::bulk_load(points.clone());

    for query in random_queries(&mut rng, 100) {
        let nn = tree.nearest_neighbor(query).unwrap();
        let tree_dist = haversine_distance(nn.coord(), query);

        let best = points
            .iter()
            .map(|p| haversine_distance(p.coord(), query))
            .fold(f64::MAX, f64::min);

        assert_relative_eq!(tree_dist, best, epsilon = 1e-3);
    }
}

// ---------------------------------------------------------------------------
// Test 2: nearest_neighbor_iter_with_distance yields non-decreasing metres
// ---------------------------------------------------------------------------

#[test]
fn nearest_neighbor_iter_returns_sorted_distances() {
    let mut rng = StdRng::seed_from_u64(0xFEED_FACE_DEAD_BEEF);
    let points = random_points(&mut rng, 1000);
    let tree = Geodetic3DTree::bulk_load(points.clone());
    let n = points.len();

    for query in random_queries(&mut rng, 10) {
        let distances: Vec<f64> = tree
            .nearest_neighbor_iter_with_distance(query)
            .map(|(_, d)| d)
            .collect();

        assert_eq!(
            distances.len(),
            n,
            "iterator should yield exactly {n} items"
        );

        for window in distances.windows(2) {
            assert!(
                window[0] <= window[1] + 1e-6,
                "distances not non-decreasing: {} > {}",
                window[0],
                window[1]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 3: the antimeridian is handled (the inverted 2D regression)
// ---------------------------------------------------------------------------

#[test]
fn antimeridian_is_handled() {
    // Points at 179°E and 175°W; a query at 177°E. The 3D embedding puts both
    // points and the query near the seam, so the nearer point (179°E) is returned
    // with no frame-shifting.
    let tree = Geodetic3DTree::bulk_load(vec![
        GeodeticPoint::new(179.0, 0.0),
        GeodeticPoint::new(-175.0, 0.0),
        GeodeticPoint::new(-77.0, 0.0),
    ]);

    let nn = tree.nearest_neighbor(coord(177.0, 0.0)).unwrap();
    assert_eq!(nn.coord().lon, 179.0, "expected the 179°E point");

    // A query at -178° is nearer the 175°W point (177° away vs 183° to 179°E
    // the short way: -178 to -175 is 3°, -178 to 179 is 3° too). Use 178°E side.
    let nn = tree.nearest_neighbor(coord(-178.0, 0.0)).unwrap();
    // -178 to -175 = 3°, -178 to 179 = 3° (across the seam). Both equidistant in
    // longitude at the equator; assert one of the two seam points, not the distant.
    assert_ne!(nn.coord().lon, -77.0, "must not return the distant point");
}

// ---------------------------------------------------------------------------
// Test 4: poles
// ---------------------------------------------------------------------------

#[test]
fn poles_are_handled() {
    let tree = Geodetic3DTree::bulk_load(vec![
        GeodeticPoint::new(0.0, 89.5),    // near north pole
        GeodeticPoint::new(137.0, -89.5), // near south pole
        GeodeticPoint::new(0.0, 0.0),     // equator
    ]);

    // A query at the exact north pole returns the near-north point regardless of
    // the query's (undefined) longitude.
    let nn = tree.nearest_neighbor(coord(45.0, 90.0)).unwrap();
    assert_eq!(nn.coord().lat, 89.5, "expected the near-north-pole point");

    let nn = tree.nearest_neighbor(coord(-12.0, -90.0)).unwrap();
    assert_eq!(nn.coord().lat, -89.5, "expected the near-south-pole point");
}

// ---------------------------------------------------------------------------
// Test 5: antipodal query
// ---------------------------------------------------------------------------

#[test]
fn antipodal_distance_is_half_circumference() {
    let tree = Geodetic3DTree::bulk_load(vec![GeodeticPoint::new(0.0, 0.0)]);
    // The antipode of (0, 0) is (180, 0).
    let (nn, metres) = tree
        .nearest_neighbor_with_distance(coord(180.0, 0.0))
        .unwrap();
    assert_eq!(nn.coord(), coord(0.0, 0.0));
    let half_circumference = std::f64::consts::PI * EARTH_RADIUS_METRES;
    assert_relative_eq!(metres, half_circumference, epsilon = 1.0);
}

// ---------------------------------------------------------------------------
// Test 6: coincident / degenerate
// ---------------------------------------------------------------------------

#[test]
fn coincident_and_empty_cases() {
    // Duplicate coordinates: a query on the point gives distance 0.
    let tree = Geodetic3DTree::bulk_load(vec![
        GeodeticPoint::new(10.0, 20.0),
        GeodeticPoint::new(10.0, 20.0),
    ]);
    let (_, metres) = tree
        .nearest_neighbor_with_distance(coord(10.0, 20.0))
        .unwrap();
    assert_eq!(metres, 0.0);

    // Empty tree.
    let empty = Geodetic3DTree::new();
    assert!(empty.nearest_neighbor(coord(0.0, 0.0)).is_none());
}

// ---------------------------------------------------------------------------
// Test: nearest_neighbors returns all tied (coincident) points on a populated
// tree, and agrees with the brute-force minimum distance.
// ---------------------------------------------------------------------------

#[test]
fn nearest_neighbors_returns_all_ties() {
    // Two coincident points plus a third, nearby but distinct, point.
    let dup = GeodeticPoint::new(10.0, 20.0);
    let other = GeodeticPoint::new(10.5, 20.0);
    let points = vec![dup, dup, other];
    let tree = Geodetic3DTree::bulk_load(points.clone());

    // Querying exactly on the duplicates returns exactly the two coincident points.
    let ties = tree.nearest_neighbors(coord(10.0, 20.0));
    assert_eq!(ties.len(), 2, "both coincident points should be returned");
    for p in &ties {
        assert_eq!(p.coord(), coord(10.0, 20.0));
    }

    // The tie distance equals the brute-force minimum (zero here).
    let brute_min = points
        .iter()
        .map(|p| haversine_distance(p.coord(), coord(10.0, 20.0)))
        .fold(f64::MAX, f64::min);
    assert_relative_eq!(brute_min, 0.0, epsilon = 1e-9);

    // A query strictly nearer one point returns a single nearest, not the tie set.
    let single = tree.nearest_neighbors(coord(10.4, 20.0));
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].coord(), other.coord());
}

// ---------------------------------------------------------------------------
// Test: exact-location facade methods (locate_at_point, locate_all_at_point,
// remove_at_point) plus the iterators (iter, nearest_neighbor_iter).
// ---------------------------------------------------------------------------

#[test]
fn locate_at_point_finds_inserted_point() {
    let mut tree = Geodetic3DTree::new();
    let p = GeodeticPoint::new(13.405, 52.52);
    tree.insert(p);

    let found = tree.locate_at_point(coord(13.405, 52.52)).expect("present");
    assert_eq!(found.coord(), p.coord());

    // A coordinate that was never inserted is absent.
    assert!(tree.locate_at_point(coord(0.0, 0.0)).is_none());
}

#[test]
fn locate_all_at_point_returns_every_duplicate() {
    let dup = GeodeticPoint::new(-3.7038, 40.4168);
    let other = GeodeticPoint::new(2.3522, 48.8566);
    let tree = Geodetic3DTree::bulk_load(vec![dup, dup, dup, other]);

    let all: Vec<GeodeticCoord> = tree
        .locate_all_at_point(coord(-3.7038, 40.4168))
        .map(|p| p.coord())
        .collect();
    assert_eq!(all.len(), 3, "all three duplicates should be returned");
    assert!(all.iter().all(|c| *c == coord(-3.7038, 40.4168)));

    // A distinct coordinate yields none of the duplicates.
    let none: Vec<_> = tree.locate_all_at_point(coord(100.0, 0.0)).collect();
    assert!(none.is_empty());
}

#[test]
fn remove_at_point_removes_a_present_coord_and_shrinks() {
    let mut tree = Geodetic3DTree::bulk_load(vec![
        GeodeticPoint::new(10.0, 20.0),
        GeodeticPoint::new(30.0, 40.0),
    ]);
    assert_eq!(tree.size(), 2);

    let removed = tree.remove_at_point(coord(10.0, 20.0)).expect("present");
    assert_eq!(removed.coord(), coord(10.0, 20.0));
    assert_eq!(tree.size(), 1);
    assert!(tree.locate_at_point(coord(10.0, 20.0)).is_none());

    // Removing an absent coordinate returns None and leaves the size unchanged.
    assert!(tree.remove_at_point(coord(0.0, 0.0)).is_none());
    assert_eq!(tree.size(), 1);
}

#[test]
fn nearest_neighbor_iter_matches_with_distance_ordering() {
    let mut rng = StdRng::seed_from_u64(0x0BAD_C0DE_F00D_1234);
    let points = random_points(&mut rng, 200);
    let tree = Geodetic3DTree::bulk_load(points);

    for query in random_queries(&mut rng, 5) {
        let no_distance: Vec<GeodeticCoord> = tree
            .nearest_neighbor_iter(query)
            .map(|p| p.coord())
            .collect();
        let with_distance: Vec<GeodeticCoord> = tree
            .nearest_neighbor_iter_with_distance(query)
            .map(|(p, _)| p.coord())
            .collect();
        assert_eq!(
            no_distance, with_distance,
            "the no-distance iterator must visit points in the same order"
        );
    }
}

#[test]
fn iter_mut_visits_all_points() {
    let mut tree = Geodetic3DTree::bulk_load(vec![
        GeodeticPoint::new(10.0, 20.0),
        GeodeticPoint::new(30.0, 40.0),
    ]);

    // iter_mut visits every point (parity with iter); we only read here, since
    // mutating the embedded vector through it could corrupt the index.
    let visited = tree.iter_mut().count();
    assert_eq!(visited, 2);
}

#[test]
fn iter_yields_all_inserted_points() {
    let points = vec![
        GeodeticPoint::new(-0.1278, 51.5074),
        GeodeticPoint::new(2.3522, 48.8566),
        GeodeticPoint::new(13.405, 52.52),
    ];
    let tree = Geodetic3DTree::bulk_load(points.clone());

    let mut from_iter: Vec<GeodeticCoord> = tree.iter().map(|p| p.coord()).collect();
    let mut expected: Vec<GeodeticCoord> = points.iter().map(|p| p.coord()).collect();

    let key = |c: &GeodeticCoord| (c.lon.to_bits(), c.lat.to_bits());
    from_iter.sort_by_key(key);
    expected.sort_by_key(key);
    assert_eq!(from_iter, expected);
}

// ---------------------------------------------------------------------------
// Test 7: seam-straddling dataset, indexed directly (no wrap helper)
// ---------------------------------------------------------------------------

#[test]
fn seam_straddling_cluster() {
    // A cluster straddling ±180°, plus one distant point.
    let cluster = vec![
        GeodeticPoint::new(179.0, -17.0),
        GeodeticPoint::new(179.8, -18.0),
        GeodeticPoint::new(-179.5, -17.5),
        GeodeticPoint::new(-178.0, -19.0),
        GeodeticPoint::new(-77.0, -12.0), // distant (South America)
    ];
    let tree = Geodetic3DTree::bulk_load(cluster.clone());

    // NN of a query inside the cluster is a cluster member, not the distant point.
    let query = coord(-179.9, -17.5);
    let nn = tree.nearest_neighbor(query).unwrap();
    assert_ne!(nn.coord().lon, -77.0, "must not return the distant point");

    // Radius query: a 500 km radius around the seam should capture the cluster but
    // not the distant point.
    let within: Vec<GeodeticCoord> = tree
        .locate_within_distance(query, 500_000.0)
        .map(|p| p.coord())
        .collect();
    assert!(
        !within.iter().any(|c| c.lon == -77.0),
        "distant point should not be within 500 km of the seam"
    );
    assert!(
        within.len() >= 2,
        "expected several cluster members within 500 km, got {}",
        within.len()
    );
}

// ---------------------------------------------------------------------------
// Test 8: nearest_neighbor selects correctly when an antipodal competitor is
// present (the maximum-distance case), and that competitor ranks last at ~pi*R.
// ---------------------------------------------------------------------------

#[test]
fn nearest_neighbor_correct_with_antipodal_competitor() {
    // Query at (0, 0). The antipode (180, 0) is the farthest point possible
    // (pi*R), so it must never be chosen as nearest; the closest competitor must
    // win and the antipode must come last in the distance-ordered iterator.
    let query = coord(0.0, 0.0);
    let antipode = GeodeticPoint::new(180.0, 0.0);
    let points = vec![
        GeodeticPoint::new(10.0, 0.0),
        antipode,
        GeodeticPoint::new(1.0, 0.0), // the nearest competitor
        GeodeticPoint::new(90.0, 0.0),
    ];
    let tree = Geodetic3DTree::bulk_load(points.clone());

    let nn = tree.nearest_neighbor(query).unwrap();
    assert_eq!(
        nn.coord(),
        coord(1.0, 0.0),
        "nearest must be the closest competitor, not the antipode"
    );

    // Cross-check the chosen distance against the brute-force minimum.
    let brute_min = points
        .iter()
        .map(|p| haversine_distance(p.coord(), query))
        .fold(f64::MAX, f64::min);
    assert_relative_eq!(
        haversine_distance(nn.coord(), query),
        brute_min,
        epsilon = 1e-3
    );

    // The antipode ranks last, at half the circumference.
    let ordered: Vec<(GeodeticCoord, f64)> = tree
        .nearest_neighbor_iter_with_distance(query)
        .map(|(p, d)| (p.coord(), d))
        .collect();
    let (last_coord, last_dist) = *ordered.last().unwrap();
    assert_eq!(
        last_coord,
        antipode.coord(),
        "the antipode must be farthest"
    );
    let half_circumference = std::f64::consts::PI * EARTH_RADIUS_METRES;
    assert_relative_eq!(last_dist, half_circumference, epsilon = 1.0);
}

// ---------------------------------------------------------------------------
// Test 9: deterministic pole and seam clusters large enough to force a real
// internal node, so an AABB<UnitVec> spanning a pole / the seam is actually
// traversed during pruning (DefaultParams::MAX_SIZE = 6, so > 6 points split).
// ---------------------------------------------------------------------------

#[test]
fn pole_cluster_forms_internal_node_and_matches_brute_force() {
    // Ten points hugging the north pole across a full spread of longitudes; the
    // bounding box has its z bound pinned near +1 and spans the pole region.
    let points: Vec<GeodeticPoint> = (0..10)
        .map(|i| {
            let lon = -180.0 + (i as f64) * 36.0; // -180 .. 144, all distinct
            let lat = 88.0 + (i % 2) as f64; // 88 deg (~222 km) or 89 deg (~111 km)
            GeodeticPoint::new(lon, lat)
        })
        .collect();

    // 150 km radius from the pole includes the lat-89 ring (~111 km) and excludes
    // the lat-88 ring (~222 km), so the radius result is a non-trivial subset.
    assert_matches_brute_force(&points, coord(45.0, 90.0), 150_000.0);
    assert_matches_brute_force(&points, coord(-120.0, 89.5), 200_000.0);
}

#[test]
fn seam_cluster_forms_internal_node_and_matches_brute_force() {
    // Ten points sweeping across +/-180, so a real internal node carries a
    // seam-spanning box.
    let points: Vec<GeodeticPoint> = (0..10)
        .map(|i| {
            let mut lon = 176.0 + (i as f64) * 1.6; // 176 .. 190.4, crossing 180
            if lon > 180.0 {
                lon -= 360.0;
            }
            let lat = -20.0 + (i as f64) * 0.5;
            GeodeticPoint::new(lon, lat)
        })
        .collect();

    assert_matches_brute_force(&points, coord(180.0, -18.0), 400_000.0);
    assert_matches_brute_force(&points, coord(-179.0, -19.0), 300_000.0);
}
