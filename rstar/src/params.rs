use crate::algorithm::rstar::RStarInsertionStrategy;
use crate::rtree::InsertionStrategy;

/// Defines static parameters for an r-tree.
///
/// Internally, an r-tree contains several nodes, similar to a b-tree. These parameters change
/// the size of these nodes and can be used to fine tune the tree's performance.
pub trait RTreeParams {
    /// The minimum size of an internal node. Must be at most half as large as `MAX_SIZE`.
    /// Choosing a value around one half or one third of `MAX_SIZE` is recommended. Higher
    /// values should yield slightly better tree quality while lower values may benefit
    /// insertion performance.
    const MIN_SIZE: usize;

    /// The maximum size of an internal node. Larger values will improve insertion performance
    /// but increase the average query time.
    const MAX_SIZE: usize;

    /// The insertion strategy which is used when calling [insert](struct.RTree.html#method.insert).
    type DefaultInsertionStrategy: InsertionStrategy;
}

/// The default parameters used when creating an r-tree without specific parameters.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DefaultParams;

impl RTreeParams for DefaultParams {
    const MIN_SIZE: usize = 3;
    const MAX_SIZE: usize = 6;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}
