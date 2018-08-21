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
#[cfg(not(feature = "debug"))]
mod metrics;

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
mod selection_funcs;

#[cfg(test)]
mod testutils;

#[cfg(feature = "debug")]
pub use node::RTreeNode;

pub use aabb::AABB;
pub use object::{PointDistance, RTreeObject};
pub use params::RTreeParams;
pub use point::{EuclideanPoint, Point, RTreeNum};
pub use rstar::RStarInsertionStrategy;
pub use rtree::RTree;
