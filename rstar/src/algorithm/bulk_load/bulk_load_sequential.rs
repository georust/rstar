use crate::envelope::Envelope;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;

#[cfg(not(test))]
use alloc::{vec, vec::Vec};

use super::cluster_group_iterator::ClusterGroupIterator;

/// Builds a subtree of the given `height` from `elements`.
///
/// The height is fixed by the caller (not derived from the number of elements again), which
/// keeps all leaves on the same level.
fn bulk_load_recursive<T, Params>(mut elements: Vec<T>, height: usize) -> ParentNode<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    if height <= 1 {
        // Reached leaf level. Shrink excess capacity so the in-place collect
        // (which reuses the allocation when size_of::<T> == size_of::<RTreeNode<T>>)
        // doesn't preserve a massively over-sized buffer in the final tree node.
        elements.shrink_to_fit();
        let elements: Vec<_> = elements.into_iter().map(RTreeNode::Leaf).collect();
        return ParentNode::new_parent(elements);
    }
    // The number of elements each subtree can hold
    let subtree_capacity = Params::MAX_SIZE.saturating_pow(height as u32 - 1);
    // How many clusters will this node contain at least
    let clusters = elements.len().div_ceil(subtree_capacity);
    // More clusters would leave some subtree less than half full
    let max_clusters = (elements.len() / subtree_capacity.div_ceil(2)).min(Params::MAX_SIZE);

    let iterator = PartitioningTask::<_, Params> {
        subtree_height: height - 1,
        work_queue: vec![PartitioningState {
            elements,
            clusters,
            max_clusters,
            remaining_axes: <T::Envelope as Envelope>::Point::DIMENSIONS,
        }],
        _params: Default::default(),
    };
    ParentNode::new_parent(iterator.collect())
}

/// Represents a partitioning task that still needs to be done.
///
/// A partitioning iterator will take this item from its work queue and start partitioning
/// "elements" into "clusters" clusters along the "remaining_axes" axes that were not used yet.
struct PartitioningState<T: RTreeObject> {
    elements: Vec<T>,
    clusters: usize,
    /// The number of clusters may be raised up to this value to get a full grid of clusters.
    max_clusters: usize,
    remaining_axes: usize,
}

/// Successively partitions the given elements into cluster groups and finally into clusters.
struct PartitioningTask<T: RTreeObject, Params: RTreeParams> {
    work_queue: Vec<PartitioningState<T>>,
    subtree_height: usize,
    _params: core::marker::PhantomData<Params>,
}

impl<T: RTreeObject, Params: RTreeParams> Iterator for PartitioningTask<T, Params> {
    type Item = RTreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.work_queue.pop() {
            let PartitioningState {
                elements,
                mut clusters,
                max_clusters,
                remaining_axes,
            } = next;
            if clusters == 1 {
                // Partitioning finished successfully. The remaining cluster forms a new node
                let data = bulk_load_recursive::<_, Params>(elements, self.subtree_height);
                return RTreeNode::Parent(data).into();
            } else {
                // The cluster group needs to be partitioned further along the next axis.
                // Try to split all clusters among the remaining axes as evenly as possible by
                // taking the nth root. On the last axis, every cluster group is a cluster.
                let groups = ceil_root(clusters, remaining_axes);
                // Clusters of the same shape overlap less than a grid with a row of wider
                // clusters, so fill the grid if that does not leave the subtrees too empty.
                let grid = groups * clusters.div_ceil(groups);
                if grid <= max_clusters {
                    clusters = grid;
                }
                // The first axis gets the most cluster groups. Rotating it with the level of
                // the tree keeps the envelopes of the nodes from being stretched along the
                // same axis on every level.
                let dimensions = <T::Envelope as Envelope>::Point::DIMENSIONS;
                let axis = (remaining_axes - 1 + self.subtree_height) % dimensions;
                let iterator = ClusterGroupIterator::new(elements, clusters, groups, axis);
                self.work_queue
                    .extend(iterator.map(|(slab, clusters)| PartitioningState {
                        elements: slab,
                        clusters,
                        max_clusters: clusters,
                        remaining_axes: remaining_axes - 1,
                    }));
            }
        }
        None
    }
}

/// Returns the smallest `root` with `root.pow(degree) >= value`.
fn ceil_root(value: usize, degree: usize) -> usize {
    let mut root = 1usize;
    while root.saturating_pow(degree as u32) < value {
        root += 1;
    }
    root
}

/// A multi dimensional implementation of the OMT bulk loading algorithm.
///
/// See http://ceur-ws.org/Vol-74/files/FORUM_18.pdf
///
/// All leaves of the resulting tree are on the same level and every node but the root is at
/// least half full.
pub fn bulk_load_sequential<T, Params>(elements: Vec<T>) -> ParentNode<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    // The height of the resulting tree, assuming all nodes will be filled up to MAX_SIZE
    let mut height = 1;
    let mut capacity = Params::MAX_SIZE;
    while capacity < elements.len() {
        capacity = capacity.saturating_mul(Params::MAX_SIZE);
        height += 1;
    }
    bulk_load_recursive::<_, Params>(elements, height)
}

#[cfg(test)]
mod test {
    use crate::test_utilities::*;
    use crate::{Point, RTree, RTreeObject};
    use std::collections::HashSet;
    use std::fmt::Debug;
    use std::hash::Hash;

    #[test]
    fn test_bulk_load_small() {
        let random_points = create_random_integers::<[i32; 2]>(50, SEED_1);
        create_and_check_bulk_loading_with_points(&random_points);
    }

    #[test]
    fn test_bulk_load_large() {
        let random_points = create_random_integers::<[i32; 2]>(3000, SEED_1);
        create_and_check_bulk_loading_with_points(&random_points);
    }

    #[test]
    fn test_bulk_load_with_different_sizes() {
        for size in (0..100).map(|i| i * 7) {
            test_bulk_load_with_size_and_dimension::<[i32; 2]>(size);
            test_bulk_load_with_size_and_dimension::<[i32; 3]>(size);
            test_bulk_load_with_size_and_dimension::<[i32; 4]>(size);
        }
    }

    fn test_bulk_load_with_size_and_dimension<P>(size: usize)
    where
        P: Point<Scalar = i32> + RTreeObject + Send + Sync + Eq + Clone + Debug + Hash + 'static,
        P::Envelope: Send + Sync,
    {
        let random_points = create_random_integers::<P>(size, SEED_1);
        create_and_check_bulk_loading_with_points(&random_points);
    }

    fn create_and_check_bulk_loading_with_points<P>(points: &[P])
    where
        P: RTreeObject + Send + Sync + Eq + Clone + Debug + Hash + 'static,
        P::Envelope: Send + Sync,
    {
        let tree = RTree::bulk_load(points.into());
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), points.len());
    }

    /// All leaves must be on the same level and all nodes sufficiently filled, for any size.
    /// (OMT used to derive the depth of every subtree from its own size, which put leaves of
    /// e.g. 25 elements onto different levels and made later insertions panic.)
    #[test]
    fn test_bulk_load_is_balanced() {
        use crate::params::{DefaultParams, RTreeParams};
        use crate::RStarInsertionStrategy;

        struct SmallParams;
        impl RTreeParams for SmallParams {
            const MIN_SIZE: usize = 2;
            const MAX_SIZE: usize = 4;
            const REINSERTION_COUNT: usize = 1;
            type DefaultInsertionStrategy = RStarInsertionStrategy;
        }

        struct OddParams;
        impl RTreeParams for OddParams {
            const MIN_SIZE: usize = 4;
            const MAX_SIZE: usize = 7;
            const REINSERTION_COUNT: usize = 2;
            type DefaultInsertionStrategy = RStarInsertionStrategy;
        }

        fn check<P, Params>(size: usize)
        where
            P: Point<Scalar = i32> + RTreeObject,
            Params: RTreeParams,
        {
            let points = create_random_integers::<P>(size, SEED_1);
            let tree = RTree::<P, Params>::bulk_load_with_params(points);
            assert_eq!(tree.size(), size);
            tree.root().sanity_check::<Params>(true);
        }

        let sizes = (0..300).chain((300..1300).step_by(7));
        for size in sizes.chain([1296, 1297, 6145, 7167, 7776, 7777]) {
            check::<[i32; 2], DefaultParams>(size);
            check::<[i32; 2], SmallParams>(size);
            check::<[i32; 2], OddParams>(size);
            check::<[i32; 3], DefaultParams>(size);
            check::<[i32; 4], SmallParams>(size);
        }
    }

    #[test]
    fn test_insert_and_remove_after_bulk_load() {
        for size in [25, 27, 97, 111, 385, 447, 1537, 1791] {
            let rectangles = create_random_rectangles(size, SEED_1);
            let more_rectangles = create_random_rectangles(size, SEED_2);
            let mut tree = RTree::bulk_load(rectangles.clone());
            for (old, new) in rectangles.iter().zip(&more_rectangles) {
                assert!(tree.remove(old).is_some());
                tree.insert(*new);
            }
            assert_eq!(tree.size(), size);
            assert!(more_rectangles.iter().all(|r| tree.contains(r)));
        }
    }

    /// Verify that bulk-loaded tree nodes don't retain excessive Vec capacity.
    ///
    /// Without shrinking over-sized allocations during partitioning, Rust's
    /// in-place collect optimization (triggered when
    /// `size_of::<T>() == size_of::<RTreeNode<T>>()`) can preserve them in
    /// the final tree nodes. For large inputs this can waste many gigabytes
    /// of memory.
    #[test]
    fn test_bulk_load_no_excess_capacity() {
        use crate::node::RTreeNode;

        const N: usize = 10_000;
        let points: Vec<[i32; 2]> = (0..N as i32).map(|i| [i, i * 3]).collect();
        let tree = RTree::bulk_load(points);
        assert_eq!(tree.size(), N);

        // Walk all internal nodes and check that children Vecs are not
        // drastically over-allocated. Allow 2x as headroom for normal
        // allocator rounding.
        let max_allowed_ratio = 2.0_f64;
        let mut checked = 0usize;
        let mut stack: Vec<&crate::node::ParentNode<[i32; 2]>> = vec![tree.root()];
        while let Some(node) = stack.pop() {
            let len = node.children.len();
            let cap = node.children.capacity();
            assert!(len > 0, "empty internal node should not exist");
            let ratio = cap as f64 / len as f64;
            assert!(
                ratio <= max_allowed_ratio,
                "node children Vec has excessive capacity: len={len}, cap={cap}, ratio={ratio:.1}x \
                 (max {max_allowed_ratio}x). This indicates split_off over-capacity is leaking \
                 into the tree."
            );
            checked += 1;
            for child in &node.children {
                if let RTreeNode::Parent(ref p) = child {
                    stack.push(p);
                }
            }
        }
        assert!(checked > 1, "expected multiple internal nodes for N={N}");
    }
}
