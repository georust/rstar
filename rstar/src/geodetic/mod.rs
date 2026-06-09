//! A geodetic (longitude/latitude) R-tree using great-circle distance for
//! nearest-neighbour and radius queries.
//!
//! Each `(lon, lat)` is mapped to a unit vector on the sphere (an
//! [n-vector](https://en.wikipedia.org/wiki/N-vector)) and indexed in a stock
//! [`crate::RTree`]. That embedding is continuous over the whole sphere, so the ±180°
//! antimeridian and the poles are ordinary interior points – no wrapping, duplication, or
//! special cases are necessary – and nearest-neighbour ordering matches true great-circle distance.
//! [`GeodeticRTree`] is the entry point; it indexes [`GeodeticPoint`],
//! [`GeodeticLineString`], and [`GeodeticPolygon`] leaves.
//!
//! # Coordinates and units
//!
//! Coordinates are **longitude first, latitude second** (`x = lon`, `y = lat`) – the
//! `geo`/OGC convention, and the opposite of ISO 6709 lat/lon order. Queries take
//! [`GeodeticCoord`] in degrees and return great-circle distances in **metres**. The raw
//! squared-chord metric surfaces only if you call [`crate::PointDistance::distance_2`]
//! directly; [`squared_chord_to_metres`] converts it.
//!
//! # Earth model
//!
//! Distances use a **spherical** Earth (the GRS80 mean radius, 6 371 008.8 m, matching
//! `geo::MEAN_EARTH_RADIUS`); against an ellipsoid the error is at most about 0.5%. For
//! exact ellipsoidal distances on the point tree, enable the optional `geodetic-wgs84`
//! feature: it adds `nearest_neighbor_with_distance_on_ellipsoid` and
//! `locate_within_distance_on_ellipsoid` (Karney's geodesic, via `geographiclib-rs`),
//! their `_wgs84` shorthands, and the standalone `geodesic_distance`. That feature
//! requires `std`; the base is no_std.
//!
//! The ellipsoid is chosen with an `Ellipsoid` value: `Ellipsoid::WGS84` (the default),
//! `Ellipsoid::GRS80`, or a custom `Ellipsoid::new`. "WGS84" here denotes the WGS84
//! reference *ellipsoid*, a geometric surface fixed by `a = 6 378 137.0 m` and
//! `1/f = 298.257 223 563`.
//!
//! # Window queries
//!
//! [`GeodeticRTree::locate_in_rectangle`] returns the points inside a longitude/latitude
//! rectangle. A window crossing the antimeridian is expressed by ordering the corners
//! west-then-east, so `lower.lon > upper.lon` denotes a seam-crossing span – the GeoJSON
//! [RFC 7946 §5.2](https://www.rfc-editor.org/rfc/rfc7946.html#section-5.2) convention; no
//! splitting is needed.
//!
//! # Example
//!
//! A dataset straddling the antimeridian is indexed directly, with no wrapping
//! helper, and queried near the seam:
//!
//! ```
//! # #[cfg(feature = "geodetic")]
//! # fn main() {
//! use rstar::geodetic::{GeodeticRTree, GeodeticCoord, GeodeticPoint};
//!
//! // Two islands either side of the ±180° seam, plus a distant point.
//! let tree = GeodeticRTree::bulk_load(vec![
//!     GeodeticPoint::new(179.0, -17.0),  // 179°E
//!     GeodeticPoint::new(-175.0, -21.0), // 175°W
//!     GeodeticPoint::new(-77.0, -12.0),  // distant
//! ]);
//!
//! // Query near the seam; no frame-shifting needed.
//! let query = GeodeticCoord { lon: -176.0, lat: -21.0 };
//! let (nn, distance_m) = tree.nearest_neighbor_with_distance(query).unwrap();
//!
//! assert_eq!((nn.coord().lon, nn.coord().lat), (-175.0, -21.0)); // the 175°W island
//! assert!(distance_m < 200_000.0); // under 200 km
//! # }
//! # #[cfg(not(feature = "geodetic"))]
//! # fn main() {}
//! ```
//!
//! # Prior art and references
//!
//! The unit-sphere embedding is the baseline approach in Schubert et al. (§3.1), which
//! proves the lower-bound and strict-monotonicity properties the pruning depends on. The
//! same embedding underlies PostGIS `geography`, Google S2, and Uber H3, so the index can
//! be validated against independent implementations. Extent leaves additionally inflate
//! each box to enclose the bulge of its great-circle arcs (after PostGIS's
//! `edge_calculate_gbox`), and point-in-polygon is a great-circle ray cast, so any simple
//! polygon is supported, including ones larger than a hemisphere.
//!
//! - Schubert, Zimek, Kriegel, "Geodetic distance queries on R-trees for indexing
//!   geographic data", SSTD 2013, LNCS 8098, pp. 146–164
//!   ([doi:10.1007/978-3-642-40235-7_9](https://doi.org/10.1007/978-3-642-40235-7_9)).
//! - Roussopoulos, Kelley, Vincent, "Nearest neighbor queries", ACM SIGMOD 1995 – the
//!   MINDIST/MINMAXDIST point-to-box bounds the branch-and-bound traversal uses.
//! - The same embedding in production: [S2 Geometry](https://s2geometry.io/), PostGIS
//!   [`lwgeodetic.c`](https://github.com/postgis/postgis/blob/master/liblwgeom/lwgeodetic.c),
//!   and [Uber H3](https://h3geo.org/).

mod arc;
mod coord;
mod distance;
mod embedding;
mod linestring;
mod point;
mod polygon;
#[cfg(feature = "geodetic-wgs84")]
mod spheroid;
mod tree;

/// Clamps a sine/cosine value to the `asin`/`acos` domain `[-1, 1]`.
///
/// Floating-point round-off can push a value that is mathematically in `[-1, 1]`
/// a few ulp past either bound, where `asin`/`acos` return NaN. Every
/// inverse-trigonometric call in this module goes through this guard so the
/// domain handling stays uniform. NaN is propagated unchanged (`clamp` keeps
/// NaN), so corrupt input surfaces as NaN rather than being silently mapped to
/// a plausible value.
pub(crate) fn clamp_unit(x: f64) -> f64 {
    x.clamp(-1.0, 1.0)
}

pub use arc::{arc_bounding_box, arc_contains_point, arc_distance_2, nearest_point_on_arc};
pub use coord::{GeodeticCoord, GeodeticError};
pub use distance::{
    haversine_distance, metres_to_squared_chord, squared_chord_to_metres, EARTH_RADIUS_METRES,
};
pub use embedding::{squared_chord, UnitVec};
pub use linestring::GeodeticLineString;
pub use point::GeodeticPoint;
pub use polygon::{GeodeticPolygon, GeodeticRing};
#[cfg(feature = "geodetic-wgs84")]
pub use spheroid::{geodesic_distance, geodesic_distance_wgs84, Ellipsoid};
pub use tree::{envelope_distance_metres, GeodeticObject, GeodeticRTree, PointLeaf};
