//! Geodetic (latitude/longitude) coordinate types for use with [`crate::RTree`].
//!
//! This module will grow to support indexing using great-circle distance as the
//! distance metric, following Schubert, Zimek & Kriegel (2013), "Geodetic Distance
//! Queries on R-Trees for Indexing Geographic Data".

mod coord;
pub mod distance;
mod envelope;
mod point;
pub use coord::{GeodeticCoord, GeodeticError};
pub use envelope::GeodeticEnvelope;
pub use point::GeodeticPoint;
