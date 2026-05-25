//! An R-tree variant for geodetic (longitude/latitude) data, using great-circle
//! distance for nearest-neighbour and radius queries.
//!
//! The implementation follows the algorithm of Schubert, Zimek & Kriegel (2013).
//! See the full citation at the end of this page.
//!
//! # Why not `RTree<[f64; 2]>`?
//!
//! Storing longitude/latitude pairs in a standard [`crate::RTree`] and querying with
//! Euclidean distance produces distances in degrees. That measure underestimates
//! near-neighbour distances at high latitudes (a degree of longitude shrinks towards
//! the poles) and is entirely meaningless for queries that cross the antimeridian.
//! This module replaces both the point-to-point and point-to-MBR distance functions
//! with great-circle equivalents, giving queries that behave correctly anywhere on
//! the globe.
//!
//! # Design
//!
//! The tree structure is identical to the standard `AABB`-based rstar: the same
//! bulk-loading and insertion algorithms are used, and minimum bounding rectangles
//! (MBRs) remain plain longitude/latitude rectangles. Only the distance functions
//! change: [`GeodeticEnvelope::distance_2`](crate::Envelope::distance_2) uses
//! Algorithm 2 from Schubert et al. (the optimised point-to-geodetic-rectangle
//! distance), and [`GeodeticPoint::distance_2`](crate::PointDistance::distance_2)
//! uses the haversine formula. The rest of the nearest-neighbour branch-and-bound
//! logic is unchanged.
//!
//! # Coordinate order
//!
//! All coordinates are **longitude first, latitude second**, matching the convention
//! used by the `geo` crate and OGC (i.e. `x = longitude`, `y = latitude`). This is the
//! **opposite** of the ISO 6709 lat/lon order; take care when converting from sources
//! that use the latter.
//!
//! # Distance units
//!
//! All distances are great-circle metres on a spherical Earth with radius
//! 6 371 008.8 m (the GRS80 mean radius, matching `geo::MEAN_EARTH_RADIUS`).
//!
//! Note: [`crate::PointDistance::distance_2`] normally returns a *squared* distance,
//! but the trait only requires the metric to be consistent with the envelope metric.
//! In this module both `distance_2` methods return an **un-squared** distance in
//! metres. The `_2` suffix is inherited from the trait name; it **does not** indicate
//! squaring here.
//!
//! # Antimeridian caveat
//!
//! MBRs do not wrap across ±180°. If your data spans the antimeridian (e.g. a region
//! that covers both +179° and −179°), you must either duplicate affected items under
//! both their original coordinates and their shifted equivalents, or split MBRs at
//! the ±180° seam before insertion. Without this, an MBR may be computed to span the
//! globe "the long way", making nearest-neighbour pruning suboptimal. Query *results*
//! remain correct because the leaf-level haversine handles the wrap correctly; only
//! the efficiency of the branch-and-bound traversal is affected.
//!
//! # Spherical Earth model
//!
//! This module uses a spherical Earth model. For WGS84 spheroidal accuracy the
//! error is bounded at roughly 0.3% (Schubert et al., §3.3). Using the GRS80 mean
//! radius ensures the point-to-MBR distance is always a valid lower bound, which is
//! the property required for correct branch-and-bound pruning.
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "geodetic")]
//! # fn main() {
//! use rstar::RTree;
//! use rstar::geodetic::{GeodeticCoord, GeodeticPoint};
//!
//! let cities = vec![
//!     GeodeticPoint::new(-74.0060, 40.7128),  // New York
//!     GeodeticPoint::new(-0.1278, 51.5074),   // London
//!     GeodeticPoint::new(139.6917, 35.6895),  // Tokyo
//!     GeodeticPoint::new(151.2093, -33.8688), // Sydney
//! ];
//! let tree = RTree::bulk_load(cities);
//!
//! // Which city is closest to Reykjavik?
//! let reykjavik = GeodeticCoord { lon: -21.94, lat: 64.13 };
//! let (nearest, distance_m) =
//!     tree.nearest_neighbor_iter_with_distance_2(reykjavik).next().unwrap();
//!
//! assert_eq!(nearest.0.lon, -0.1278); // London
//! assert!((distance_m - 1_888_513.0).abs() < 1_000.0); // about 1,889 km
//! # }
//! # #[cfg(not(feature = "geodetic"))]
//! # fn main() {}
//! ```
//!
//! # Reference
//!
//! Schubert, E., Zimek, A., & Kriegel, H.-P. (2013). Geodetic Distance Queries on
//! R-Trees for Indexing Geographic Data. In: Nascimento, M.A., Sellis, T., Cucchiara,
//! R., Sander, J., Zheng, Y., Kriegel, H.-P., Renz, M., Sengstock, C. (eds.),
//! *Advances in Spatial and Temporal Databases*, SSTD 2013, LNCS vol. 8098,
//! pp. 146–164. Springer, Berlin, Heidelberg.
//! DOI: [10.1007/978-3-642-40235-7_9](https://doi.org/10.1007/978-3-642-40235-7_9).

mod coord;
pub mod distance;
mod envelope;
mod point;
pub use coord::{GeodeticCoord, GeodeticError};
pub use envelope::GeodeticEnvelope;
pub use point::GeodeticPoint;
