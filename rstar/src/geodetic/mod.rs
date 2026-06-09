//! A geodetic (longitude/latitude) spatial index built on a unit-sphere embedding.
//!
//! Each `(lon, lat)` is mapped to a unit vector on the sphere (an
//! [n-vector](https://en.wikipedia.org/wiki/N-vector)). That embedding is continuous over
//! the whole sphere, so the ±180° antimeridian and the poles are ordinary interior points –
//! no wrapping, duplication, or special cases are necessary – and chord ordering in the
//! embedding matches true great-circle distance.
//!
//! This module provides the embedding foundation that the leaves and the tree build on:
//! [`GeodeticCoord`] (degrees, longitude first), the [`UnitVec`] embedding, and the
//! great-circle distance helpers ([`haversine_distance`] and the squared-chord/metre
//! conversions).
//!
//! # Coordinates and units
//!
//! Coordinates are **longitude first, latitude second** (`x = lon`, `y = lat`) – the
//! `geo`/OGC convention, and the opposite of ISO 6709 lat/lon order. The raw squared-chord
//! metric is converted to **metres** with [`squared_chord_to_metres`].
//!
//! # Earth model
//!
//! Distances use a **spherical** Earth (the GRS80 mean radius, 6 371 008.8 m, matching
//! `geo::MEAN_EARTH_RADIUS`); against an ellipsoid the error is at most about 0.5%.
//!
//! # Prior art and references
//!
//! The unit-sphere embedding is the baseline approach in Schubert et al. (§3.1), which
//! proves the lower-bound and strict-monotonicity properties that pruning depends on. The
//! same embedding underlies PostGIS `geography`, Google S2, and Uber H3.
//!
//! - Schubert, Zimek, Kriegel, "Geodetic distance queries on R-trees for indexing
//!   geographic data", SSTD 2013, LNCS 8098, pp. 146–164
//!   ([doi:10.1007/978-3-642-40235-7_9](https://doi.org/10.1007/978-3-642-40235-7_9)).

mod coord;
mod distance;
mod embedding;

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

pub use coord::{GeodeticCoord, GeodeticError};
pub use distance::{
    haversine_distance, metres_to_squared_chord, squared_chord_to_metres, EARTH_RADIUS_METRES,
};
pub use embedding::{squared_chord, UnitVec};
