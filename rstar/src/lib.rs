//! An n-dimensional r*-tree implementation.
//!
//! This crate implements a flexible, n-dimensional r-tree implementation with
//! the r* (r star) insertion strategy.
//!
//! # R-Tree
//! An r-tree is a data structure containing _spatial data_ and is optimized for
//! nearest neighbor search.
//! _Spatial data_ refers to an object that has the notion of a position and extent,
//! for example points, lines and rectangles in any dimension.
//!
//!
//! # Further documentation
//! The crate's main data structure and documentation is struct
//! [RTree](struct.RTree.html).
//!
//! Also, the pre-defined primitives like lines and rectangles contained in
//! the [primitives module](primitives/index.html) may be of interest for a quick start.
//!
//! # (De)Serialization
//! Enable the `serde` feature for [Serde](https://crates.io/crates/serde) support.
//!
#![deny(missing_docs)]
mod algorithm;
mod envelope;
mod object;
mod params;
mod point;
pub mod primitives;
mod rtree;
mod structures;

#[cfg(test)]
mod test_utilities;

#[cfg(feature = "debug")]
pub use crate::rtree::root;
#[cfg(feature = "debug")]
pub use crate::structures::node::RTreeNode;

pub use crate::algorithm::rstar::RStarInsertionStrategy;
pub use crate::envelope::Envelope;
pub use crate::object::{PointDistance, RTreeObject};
pub use crate::params::{DefaultParams, InsertionStrategy, RTreeParams};
pub use crate::point::{Point, RTreeNum};
pub use crate::rtree::RTree;
pub use crate::structures::aabb::AABB;
