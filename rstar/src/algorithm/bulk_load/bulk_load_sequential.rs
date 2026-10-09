use crate::envelope::Envelope;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;

#[cfg(not(test))]
use alloc::{vec, vec::Vec};

#[allow(unused_imports)] // Import is required when building without std
use num_traits::Float;

use super::cluster_group_iterator::{calculate_cuts_per_axis, ClusterGroupIterator};
use smallvec::SmallVec;

/// Computes the depth (number of node levels, leaves included) of the tree that
/// bulk loading will build for `number_of_elements` elements.
///
/// This mirrors the partitioning performed by [`bulk_load_recursive`]: on every
/// level a node is split into `number_of_clusters_on_axis ^ DIMENSIONS`
/// clusters, so a balanced cluster shrinks by that factor per level. The
/// returned depth is the first level at which a balanced cluster fits into a
/// single (leaf) node, i.e. holds at most `MAX_SIZE` elements.
///
/// Threading this depth through the recursion (instead of stopping each branch
/// individually as soon as it reaches `MAX_SIZE` elements) guarantees that all
/// leaves end up on the same level. Otherwise clusters whose size straddles
/// `MAX_SIZE` -- some just below, some just above -- would produce leaves on
/// different levels, yielding a malformed R-tree that violates the "all leaves
/// share the same depth" invariant and makes a subsequent `insert` panic.
fn bulk_load_depth<T, Params>(number_of_elements: usize) -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    let mut depth = 1;
    let mut cluster_size = number_of_elements;
    while cluster_size > m {
        // Mirror the partitioning exactly: on every axis `ClusterGroupIterator`
        // cuts a group of `len` elements into slabs of `len.div_ceil(cuts)`, so
        // the largest cluster shrinks by one such step per axis. Rounding down
        // instead would understate it and could pick a depth one level too
        // shallow, leaving leaf nodes above `MAX_SIZE`.
        let cuts = calculate_cuts_per_axis::<T, Params>(cluster_size, 0);
        let next = cuts
            .iter()
            .fold(cluster_size, |size, &cuts| size.div_ceil(cuts));
        // `calculate_cuts_per_axis` always cuts at least one axis in two while
        // `cluster_size > MAX_SIZE >= 1`, so this terminates.
        debug_assert!(next < cluster_size);
        cluster_size = next;
        depth += 1;
    }
    depth
}

fn bulk_load_recursive<T, Params>(mut elements: Vec<T>, remaining_depth: usize) -> ParentNode<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    if remaining_depth <= 1 {
        // Reached leaf level. Shrink excess capacity so the in-place collect
        // (which reuses the allocation when size_of::<T> == size_of::<RTreeNode<T>>)
        // doesn't preserve a massively over-sized buffer in the final tree node.
        elements.shrink_to_fit();
        let elements: Vec<_> = elements.into_iter().map(RTreeNode::Leaf).collect();
        return ParentNode::new_parent(elements);
    }
    // Only two or three axes fit into a fan-out of `MAX_SIZE`, so a single node
    // cannot cut every axis once `DIMENSIONS` grows. Rotating the starting axis
    // with the level spreads the cuts over all axes across the tree's depth,
    // which keeps node envelopes tight without inflating the fan-out.
    let cuts_per_axis = calculate_cuts_per_axis::<T, Params>(
        elements.len(),
        remaining_depth % <T::Envelope as Envelope>::Point::DIMENSIONS,
    );

    let iterator = PartitioningTask::<_, Params> {
        cuts_per_axis,
        remaining_depth,
        work_queue: vec![PartitioningState {
            current_axis: <T::Envelope as Envelope>::Point::DIMENSIONS,
            elements,
        }],
        _params: Default::default(),
    };
    ParentNode::new_parent(iterator.collect())
}

/// Represents a partitioning task that still needs to be done.
///
/// A partitioning iterator will take this item from its work queue and start partitioning "elements"
/// along "current_axis" .
struct PartitioningState<T: RTreeObject> {
    elements: Vec<T>,
    current_axis: usize,
}

/// Successively partitions the given elements into  cluster groups and finally into clusters.
struct PartitioningTask<T: RTreeObject, Params: RTreeParams> {
    work_queue: Vec<PartitioningState<T>>,
    cuts_per_axis: SmallVec<[usize; 8]>,
    remaining_depth: usize,
    _params: core::marker::PhantomData<Params>,
}

impl<T: RTreeObject, Params: RTreeParams> Iterator for PartitioningTask<T, Params> {
    type Item = RTreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.work_queue.pop() {
            let PartitioningState {
                elements,
                current_axis,
            } = next;
            if current_axis == 0 {
                // Partitioning finished successfully on all axis. The remaining cluster forms a new node
                let data = bulk_load_recursive::<_, Params>(elements, self.remaining_depth - 1);
                return RTreeNode::Parent(data).into();
            } else {
                // The cluster group needs to be partitioned further along the next axis
                let iterator = ClusterGroupIterator::new(
                    elements,
                    self.cuts_per_axis[current_axis - 1],
                    current_axis - 1,
                );
                self.work_queue
                    .extend(iterator.map(|slab| PartitioningState {
                        elements: slab,
                        current_axis: current_axis - 1,
                    }));
            }
        }
        None
    }
}

/// A multi dimensional implementation of the OMT bulk loading algorithm.
///
/// See http://ceur-ws.org/Vol-74/files/FORUM_18.pdf
pub fn bulk_load_sequential<T, Params>(elements: Vec<T>) -> ParentNode<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let depth = bulk_load_depth::<T, Params>(elements.len());
    bulk_load_recursive::<_, Params>(elements, depth)
}

#[cfg(test)]
mod test {
    use crate::params::DefaultParams;
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

    /// Bulk loading must produce a valid R-tree in any number of dimensions.
    ///
    /// Splitting every axis into the same number of clusters gave a node
    /// `clusters ^ DIMENSIONS` children, which runs away from `MAX_SIZE` as
    /// soon as there is more than one axis to split: with the default
    /// parameters a three dimensional load produced nodes with 8 children, a
    /// six dimensional one nodes with 64, and the 100 dimensional case from
    /// issue #197 put all 100000 elements directly under the root.
    #[test]
    fn test_bulk_load_sanity_in_all_dimensions() {
        fn check<P>(size: usize)
        where
            P: Point<Scalar = i32> + RTreeObject + Clone + Send + Sync + 'static,
            P::Envelope: Send + Sync,
        {
            let elements: Vec<P> = (0..size as i32)
                .map(|i| Point::generate(|d| i.wrapping_mul(d as i32 * 7 + 13) % 1000))
                .collect();
            let tree = RTree::bulk_load(elements);
            assert_eq!(tree.size(), size);
            // `true` also asserts that no node exceeds `MAX_SIZE`.
            tree.root().sanity_check::<DefaultParams>(true);
        }

        for size in 1..=300 {
            check::<[i32; 2]>(size);
            check::<[i32; 3]>(size);
            check::<[i32; 4]>(size);
            check::<[i32; 5]>(size);
            check::<[i32; 6]>(size);
        }
        // The element counts named in issue #197.
        check::<[i32; 3]>(1000);
        check::<[i32; 100]>(1000);
    }

    #[test]
    fn test_bulk_load_large_counts_with_insert_churn() {
        for size in [6200, 25000] {
            let points: Vec<[i32; 2]> = (0..size).map(|i| [i, i * 3]).collect();
            let mut tree = RTree::bulk_load(points.clone());
            tree.root().sanity_check::<DefaultParams>(true);
            for point in points.iter().take(300) {
                assert_eq!(tree.remove(point), Some(*point));
                tree.insert(*point);
            }
            assert_eq!(tree.size(), points.len());
            tree.root().sanity_check::<DefaultParams>(true);
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
