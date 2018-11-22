#[cfg(test)]
#[macro_use]
extern crate approx;

extern crate num_traits;
extern crate pdqselect;

#[cfg(test)]
extern crate rand;

mod structures;
mod algorithm;
mod envelope;
mod object;
mod params;
mod point;
mod rtree;
pub mod primitives;

#[cfg(test)]
mod test_utilities;

#[cfg(feature = "debug")]
pub use structures::node::RTreeNode;

pub use crate::structures::aabb::AABB;
pub use crate::object::{PointDistance, RTreeObject};
pub use crate::params::{RTreeParams, DefaultParams};
pub use crate::point::{Point, RTreeNum};
pub use crate::algorithm::rstar::RStarInsertionStrategy;
pub use crate::rtree::RTree;
pub use crate::envelope::Envelope;