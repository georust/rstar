//! A geodetic (longitude/latitude) R-tree built on a 3D unit-sphere embedding,
//! using great-circle distance for nearest-neighbour and radius queries.
//!
//! # Why a 3D embedding?
//!
//! Storing longitude/latitude pairs in a standard [`crate::RTree`] and querying
//! with Euclidean distance produces distances in degrees, which underestimate
//! near-neighbour distances at high latitudes and are meaningless across the
//! antimeridian. Indexing the degrees directly also makes the ±180° seam and the
//! poles special cases that need wrapping or duplication.
//!
//! Instead, each `(lon, lat)` in degrees is mapped to a unit vector `[x, y, z]`
//! on the unit sphere — an [n-vector](https://en.wikipedia.org/wiki/N-vector)
//! representation (see [`GeodeticCoord::to_unit_vector`]). Those vectors are
//! indexed in a stock [`crate::RTree`] whose envelope is the reused [`crate::AABB<UnitVec>`].
//! The embedding is continuous and single-valued over the whole sphere, so the
//! antimeridian and the poles are ordinary interior points: no wrapping, no
//! duplication, no frame-shifting.
//!
//! # Metric
//!
//! The internal metric is the **squared chord** `c² = ‖q − p‖²` between unit
//! vectors, which is exactly squared Euclidean and lies in `[0, 4]`. Squared chord
//! is strictly increasing in the great-circle angle (`c² = 4·sin²(d/2)`), so
//! ordering by `c²` equals ordering by great-circle distance, and the
//! point-to-box Euclidean distance is a valid pruning lower bound (the MINDIST
//! and MINMAXDIST bounds of Roussopoulos et al.; see References). The leaf
//! metric, the envelope `distance_2`, and `min_max_dist_2` are all the same
//! squared-Euclidean function, so there is no envelope/leaf unit-mismatch and no
//! transcendental on the traversal hot path.
//!
//! The [`Geodetic3DTree`] facade converts at the boundary so callers work in
//! **metres**: queries take [`GeodeticCoord`] (degrees) and distances are returned as
//! **great-circle metres** on a **spherical Earth** with radius 6 371 008.8 m (the GRS80
//! mean radius, matching `geo::MEAN_EARTH_RADIUS`). Every facade query returns
//! metres. Squared-chord values surface only if you call
//! [`crate::PointDistance::distance_2`] on a [`GeodeticPoint`] yourself;
//! [`squared_chord_to_metres`] / [`metres_to_squared_chord`] convert between those
//! and metres.
//!
//! # Coordinate order
//!
//! All coordinates are **longitude first, latitude second**, matching the
//! convention used by the `geo` crate and OGC (i.e. `x = longitude`,
//! `y = latitude`). This is the **opposite** of the ISO 6709 lat/lon order; take
//! care when converting from sources that use the latter.
//!
//! # Spherical Earth model
//!
//! This module uses a spherical Earth model. For WGS84 spheroidal accuracy the
//! error is bounded at roughly 0.3%. Using the GRS80 mean radius ensures the
//! point-to-box distance is always a valid lower bound, which is the property
//! required for correct branch-and-bound pruning.
//!
//! # Window and range queries
//!
//! Alongside nearest-neighbour and radius queries,
//! [`Geodetic3DTree::locate_in_rectangle`] returns every point inside a
//! longitude/latitude rectangle. It uses the filter/refine scheme PostGIS applies
//! to its `geography` type: the rectangle is mapped to a 3D bounding box the index
//! scans (the filter), then each candidate is checked against the exact
//! longitude/latitude predicate (the refine). The rectangle's edges are parallels
//! and meridians (a true lon/lat box), and a window crossing the antimeridian is
//! expressed directly by ordering the corners west-then-east, so that
//! `lower.lon > upper.lon` denotes a seam-crossing span — the GeoJSON
//! [RFC 7946 §5.2](https://www.rfc-editor.org/rfc/rfc7946.html#section-5.2)
//! bounding-box convention. No splitting or point duplication is needed.
//!
//! # Example
//!
//! A dataset straddling the antimeridian is indexed directly, with no wrapping
//! helper, and queried near the seam:
//!
//! ```
//! # #[cfg(feature = "geodetic")]
//! # fn main() {
//! use rstar::geodetic::{Geodetic3DTree, GeodeticCoord, GeodeticPoint};
//!
//! // Two islands either side of the ±180° seam, plus a distant point.
//! let tree = Geodetic3DTree::bulk_load(vec![
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
//! # Prior art
//!
//! The unit-sphere embedding is the baseline
//! approach in Schubert et al. (§3.1), with existing implementations identified in
//! Oracle Spatial, IBM Informix, and the PostgreSQL
//! pgSphere project. That section proves the lower-bound and strict-monotonicity
//! properties this index depends on (so radius queries return every in-range
//! object and nearest-neighbour queries return no spurious ones).
//!
//! The same longitude/latitude → geocentric `(x, y, z)` embedding underlies
//! several widely deployed systems, allowing the soundness of this index to be checked
//! against independent implementations:
//!
//! - **PostGIS** indexes its `geography` type this way: liblwgeom's `geog2cart`
//!   maps longitude/latitude to a unit-sphere `(x, y, z)`, and the GiST index is
//!   built over the resulting 3D geocentric bounding boxes. It is the closest
//!   production analogue to this module (see References).
//! - **Google S2** represents every point as a 3D unit vector (`S2Point`).
//! - **Uber H3** uses the same ECEF-like unit-vector representation internally
//!   (`Vec3d`).
//!
//! For point data the axis-aligned box of the embedded vectors is an exact bound.
//! Extent geometry (lines, polygons) would additionally need the box inflated to
//! contain each great-circle arc, which bulges away from the chord joining its
//! endpoints; PostGIS's `edge_calculate_gbox` is the reference for that step. This
//! module indexes points only, so the inflation does not arise here.
//!
//! # References
//!
//! - Schubert, Erich, Arthur Zimek, and Hans-Peter Kriegel. "Geodetic distance
//!   queries on R-trees for indexing geographic data." Symposium on Spatial and
//!   Temporal Databases (SSTD 2013), LNCS 8098, pp. 146–164.
//!   [doi:10.1007/978-3-642-40235-7_9](https://doi.org/10.1007/978-3-642-40235-7_9).
//!   Section 3.1 ("Indexing Geodetic Data Using 3D Euclidean Coordinates") is the
//!   embedding used here, and proves the two properties the pruning relies on:
//!   Euclidean (chord) distance in the embedding is a lower bound for great-circle
//!   distance (its equation 3) and strictly monotone in it (equation 4).
//! - Roussopoulos, Nick, Stephen Kelley, and Frédéric Vincent. "Nearest neighbor
//!   queries." ACM SIGMOD Record 24, no. 2 (1995): 71–79.
//!   [citeseerx](https://citeseerx.ist.psu.edu/viewdoc/summary?doi=10.1.1.133.2288).
//!   The MINDIST point-to-box lower bound and MINMAXDIST upper bound the
//!   branch-and-bound traversal relies on, reused here via [`crate::AABB`].
//! - [S2 Geometry](https://s2geometry.io/). A widely deployed library that indexes
//!   geographic data as unit vectors on the sphere — the same embedding used here.
//! - PostGIS `geography`: the geocentric conversion in
//!   [`liblwgeom/lwgeodetic.c`](https://github.com/postgis/postgis/blob/master/liblwgeom/lwgeodetic.c)
//!   (`geog2cart`) and Paul Ramsey's design note
//!   ["PostGIS gets spherical"](http://blog.cleverelephant.ca/2009/11/postgis-gets-spherical-directors-cut.html),
//!   which describes building "a 3D R-Tree on the geocentric bounds".
//! - [Uber H3](https://h3geo.org/). Uses an ECEF-like unit-vector representation
//!   (`Vec3d`) internally for its geodesic geometry.
//! - [n-vector](https://en.wikipedia.org/wiki/N-vector). The singularity-free
//!   unit-vector representation of horizontal position, the reason the poles and
//!   the ±180° meridian need no special handling.
//! - [Great-circle distance](https://en.wikipedia.org/wiki/Great-circle_distance).
//!   Standard spherical trigonometry for the chord/half-angle identity
//!   `chord = 2·sin(d/2)` underlying the squared-chord metric and the metres
//!   conversion.

mod coord;
pub mod distance;
mod embedding;
mod point;
mod tree;

pub use coord::{GeodeticCoord, GeodeticError};
pub use distance::{metres_to_squared_chord, squared_chord_to_metres};
pub use embedding::UnitVec;
pub use point::GeodeticPoint;
pub use tree::{Geodetic3DTree, envelope_distance_metres};
