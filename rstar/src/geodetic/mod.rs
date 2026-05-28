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
//! indexed in a stock R-tree whose envelope is the reused [`crate::AABB<UnitVec>`].
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
//! and MINMAXDIST bounds of Roussopoulos et al.; see References).
//!
//! Distances are reported in **metres** on a spherical Earth with radius
//! 6 371 008.8 m (the GRS80 mean radius, matching `geo::MEAN_EARTH_RADIUS`).
//! Convert between the squared-chord metric and metres with
//! [`squared_chord_to_metres`] / [`metres_to_squared_chord`].
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
//! # References
//!
//! - Schubert, Erich, Arthur Zimek, and Hans-Peter Kriegel. "Geodetic distance
//!   queries on R-trees for indexing geographic data." Symposium on Spatial and
//!   Temporal Databases (SSTD 2013), LNCS 8098, pp. 146–164.
//!   [doi:10.1007/978-3-642-40235-7_9](https://doi.org/10.1007/978-3-642-40235-7_9).
//!   Frames the problem and compares a 2D minimum-bounding-rectangle index (its
//!   Algorithm 2) against a unit-sphere projection. This module takes the latter
//!   so that antimeridian- and pole-crossing data are correct rather than a
//!   documented caveat.
//! - Roussopoulos, Nick, Stephen Kelley, and Frédéric Vincent. "Nearest neighbor
//!   queries." ACM SIGMOD Record 24, no. 2 (1995): 71–79.
//!   [citeseerx](https://citeseerx.ist.psu.edu/viewdoc/summary?doi=10.1.1.133.2288).
//!   The MINDIST point-to-box lower bound and MINMAXDIST upper bound the
//!   branch-and-bound traversal relies on, reused here via [`crate::AABB`].
//! - [S2 Geometry](https://s2geometry.io/). A widely deployed library that indexes
//!   geographic data as unit vectors on the sphere — the same embedding used here.
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

pub use coord::{GeodeticCoord, GeodeticError};
pub use distance::{metres_to_squared_chord, squared_chord_to_metres};
pub use embedding::UnitVec;
pub use point::GeodeticPoint;
