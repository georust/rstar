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
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod aabb;
mod algorithm;
mod envelope;
mod node;
mod object;
mod params;
mod point;
pub mod primitives;
mod rtree;

#[cfg(test)]
mod test_utilities;

pub use crate::{
    aabb::AABB,
    algorithm::{rstar::RStarInsertionStrategy, selection_functions::SelectionFunction},
    envelope::Envelope,
    node::{ParentNode, RTreeNode},
    object::{PointDistance, RTreeObject},
    params::{DefaultParams, InsertionStrategy, RTreeParams},
    point::{Point, RTreeNum},
    rtree::RTree,
};
