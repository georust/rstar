#[cfg(test)]
#[macro_use]
extern crate approx;

extern crate num_traits;
extern crate pdqselect;

#[cfg(test)]
extern crate rand;

#[allow(dead_code)]
#[cfg(feature = "debug")]
pub mod metrics;

mod aabb;
mod bulk_load;
mod envelope;
mod iterators;
pub mod node;
mod object;
mod params;
mod point;
mod rstar;
mod rtree;
mod removal;
pub mod primitives;

mod nearest_neighbor;
mod selection_functions;

#[cfg(test)]
mod test_utilities;

#[cfg(feature = "debug")]
pub use node::RTreeNode;

pub use crate::aabb::AABB;
pub use crate::object::{PointDistance, RTreeObject};
pub use crate::params::{RTreeParams, DefaultParams};
pub use crate::point::{EuclideanPoint, Point, RTreeNum};
pub use crate::rstar::RStarInsertionStrategy;
pub use crate::rtree::RTree;
