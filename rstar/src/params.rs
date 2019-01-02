use crate::algorithm::rstar::RStarInsertionStrategy;
use crate::{Envelope, Point, RTree, RTreeObject};

/// Defines static parameters for an r-tree.
///
/// Internally, an r-tree contains several nodes, similar to a b-tree. These parameters change
/// the size of these nodes and can be used to fine tune the tree's performance.
///
/// # Example
/// ```
/// use rstar::{RTreeParams, RTree, RStarInsertionStrategy};
///
/// // This example uses an rtree with larger internal nodes.
/// struct LargeNodeParameters;
///
/// impl RTreeParams for LargeNodeParameters
/// {
///     const MIN_SIZE: usize = 10;
///     const MAX_SIZE: usize = 30;
///     const REINSERTION_COUNT: usize = 5;
///     type DefaultInsertionStrategy = RStarInsertionStrategy;
/// }
///
/// // Optional but helpful: Define a type alias for the new r-tree
/// type LargeNodeRTree<T> = RTree<T, LargeNodeParameters>;
///
/// # fn main() {
/// // The only difference from now on is the usage of "new_with_params" instead of "new"
/// let mut large_node_tree: LargeNodeRTree<_> = RTree::new_with_params();
/// // Using the r-tree should allow inference for the point type
/// large_node_tree.insert([1.0, -1.0f32]);
/// // There is also a bulk load method with parameters:
/// # let some_elements = vec![[0.0, 0.0]];
/// let tree: LargeNodeRTree<_> = RTree::bulk_load_with_params(some_elements);
/// # }
/// ```
pub trait RTreeParams: Send + Sync {
    /// The minimum size of an internal node. Must be at most half as large as `MAX_SIZE`.
    /// Choosing a value around one half or one third of `MAX_SIZE` is recommended. Higher
    /// values should yield slightly better tree quality while lower values may benefit
    /// insertion performance.
    const MIN_SIZE: usize;

    /// The maximum size of an internal node. Larger values will improve insertion performance
    /// but increase the average query time.
    const MAX_SIZE: usize;

    /// The number of nodes that the insertion strategy tries to reinsert sometimes to
    /// maintain a good tree quality. Must be smaller than `MAX_SIZE` - `MIN_SIZE`.
    /// Larger values will improve query times but increase insertion time.
    const REINSERTION_COUNT: usize;

    /// The insertion strategy which is used when calling [insert](struct.RTree.html#method.insert).
    type DefaultInsertionStrategy: InsertionStrategy;
}

/// The default parameters used when creating an r-tree without specific parameters.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DefaultParams;

impl RTreeParams for DefaultParams {
    const MIN_SIZE: usize = 3;
    const MAX_SIZE: usize = 6;
    const REINSERTION_COUNT: usize = 2;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}

/// Defines how points are inserted into an r-tree.
///
/// Different strategies try to minimize both _insertion time_ (how long does it take to add a new
/// object into the tree?) and _querying time_ (how long does an average nearest neighbor query
/// take?).
/// Currently, only one insertion strategy is implemented: R* (R-star) insertion. R* insertion
/// tries to minimize querying performance while yielding reasonable insertion times, making it a
/// good default strategy. More strategies might be implemented in the future.
///
/// Only calls to [insert](struct.RTree.html#method.insert) are affected by this strategy.
///
/// This trait is not meant to be implemented by the user.
pub trait InsertionStrategy {
    #[doc(hidden)]
    fn insert<T, Params>(tree: &mut RTree<T, Params>, t: T)
    where
        Params: RTreeParams,
        T: RTreeObject;
}

pub fn verify_parameters<T: RTreeObject, P: RTreeParams>() {
    assert!(
        P::MAX_SIZE >= 4,
        "MAX_SIZE too small. Must be larger than 4."
    );

    let max_min_size = (P::MAX_SIZE + 1) / 2;
    assert!(
        P::MIN_SIZE <= max_min_size,
        "MIN_SIZE too large. Must be less or equal to {:?}",
        max_min_size
    );

    let max_reinsertion_count = P::MAX_SIZE - P::MIN_SIZE;
    assert!(
        P::REINSERTION_COUNT < max_reinsertion_count,
        "REINSERTION_COUNT too large. Must be smaller than {:?}",
        max_reinsertion_count
    );

    let dimension = <T::Envelope as Envelope>::Point::DIMENSIONS;
    assert!(
        dimension > 1,
        "Point dimension too small - must be at least 2"
    );
}
