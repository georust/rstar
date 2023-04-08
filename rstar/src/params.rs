/// Defines static parameters for an r-tree.
///
/// Internally, an r-tree contains several nodes, similar to a b-tree. These parameters change
/// the size of these nodes and can be used to fine-tune the tree's performance.
///
/// # Example
/// ```
/// use rstar::{Params, RTree};
/// // This example uses an rtree with larger internal nodes.
///
/// # fn main() {
/// // The only difference from now on is the usage of "new_with_params" instead of "new"
/// let params = Params::new(10, 30, 5);
/// let mut large_node_tree: RTree<_> = RTree::new_with_params(params.clone());
/// // Using the r-tree should allow inference for the point type
/// large_node_tree.insert([1.0, -1.0f32]);
/// // There is also a bulk load method with parameters:
/// # let some_elements = vec![[0.0, 0.0]];
/// let tree: RTree<_> = RTree::bulk_load_with_params(params, some_elements);
/// # }
/// ```

/// hi
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Params {
    min_size: usize,
    max_size: usize,
    reinsertion_count: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            min_size: 3,
            max_size: 6,
            reinsertion_count: 2,
        }
    }
}

impl Params {
    /// hi
    pub fn new(min_size: usize, max_size: usize, reinsertion_count: usize) -> Self {
        // FIXME: add an Error enum and make this function return
        // Result<Self, rstar::Error> instead of asserting....

        // If we don't want to do that, to make this const, we could
        // use the `const_format` crate....
        assert!(max_size >= 4, "MAX_SIZE too small. Must be larger than 4.");

        assert!(min_size > 0, "MIN_SIZE must be at least 1",);
        let max_min_size = (max_size + 1) / 2;
        assert!(
            min_size <= max_min_size,
            "MIN_SIZE too large. Must be less or equal to {:?}",
            max_min_size
        );

        let max_reinsertion_count = max_size - min_size;
        assert!(
            reinsertion_count < max_reinsertion_count,
            "REINSERTION_COUNT too large. Must be smaller than {:?}",
            max_reinsertion_count
        );

        Params {
            min_size,
            max_size,
            reinsertion_count,
        }
    }

    /// hi
    pub fn min_size(&self) -> usize {
        self.min_size
    }

    /// hi
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// hi
    pub fn reinsertion_count(&self) -> usize {
        self.reinsertion_count
    }
}
