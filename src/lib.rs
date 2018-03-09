extern crate typenum;
extern crate generic_array;
extern crate num_traits;
extern crate smallvec;

#[cfg(test)]
extern crate rand;

#[allow(dead_code)]


mod rtree;
mod rstar;
mod params;
mod node;
mod point;
mod object;
mod mbr;
mod iterator;
mod nearest_neighbor;

#[cfg(test)]
mod testutils;

#[cfg(feature = "debug")]
pub use node::RTreeNode;

pub use rtree::RTree;
pub use iterator::RTreeIterator;
pub use mbr::MBR;
pub use point::{Point, RTreeNum};
