extern crate typenum;
extern crate generic_array;
extern crate num_traits;
extern crate smallvec;

#[cfg(test)]
extern crate rand;

#[allow(dead_code)]

#[cfg(feature = "debug")]
pub mod metrics;
#[cfg(not(feature = "debug"))]
mod metrics;

mod rtree;
mod rstar;
mod params;
pub mod node;
mod point;
mod object;
mod aabb;
mod envelope;
mod iterators;

mod nearest_neighbor;
mod selection_funcs;


#[cfg(test)]
mod testutils;

#[cfg(feature = "debug")]
pub use node::RTreeNode;

pub use params::{RTreeParams, CustomParams};
pub use rstar::{RStarInsertionStrategy};
pub use rtree::RTree;
pub use aabb::AABB;
pub use point::{Point, RTreeNum};
