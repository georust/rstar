use crate::envelope::Envelope;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;

#[cfg(not(test))]
use alloc::{vec, vec::Vec};

#[allow(unused_imports)] // Import is required when building without std
use num_traits::Float;

use super::cluster_group_iterator::{calculate_number_of_clusters_on_axis, ClusterGroupIterator};

/// Builds a subtree whose leaves are `depth` levels below it. Where the clusters of a node get
/// different depths on their own (some just above `MAX_SIZE` elements, others not), the depth
/// passed down keeps all leaves on the same level; the deeper clusters end up as nodes with a
/// few more than `MAX_SIZE` children instead.
fn bulk_load_recursive<T, Params>(mut elements: Vec<T>, depth: usize) -> ParentNode<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    if depth <= 1 {
        // Reached leaf level. Shrink excess capacity so the in-place collect
        // (which reuses the allocation when size_of::<T> == size_of::<RTreeNode<T>>)
        // doesn't preserve a massively over-sized buffer in the final tree node.
        elements.shrink_to_fit();
        let elements: Vec<_> = elements.into_iter().map(RTreeNode::Leaf).collect();
        return ParentNode::new_parent(elements);
    }
    let number_of_clusters_on_axis =
        calculate_number_of_clusters_on_axis::<T, Params>(elements.len()).max(2);

    let iterator = PartitioningTask::<_, Params> {
        number_of_clusters_on_axis,
        subtree_depth: depth - 1,
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
    number_of_clusters_on_axis: usize,
    subtree_depth: usize,
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
                let data = bulk_load_recursive::<_, Params>(elements, self.subtree_depth);
                return RTreeNode::Parent(data).into();
            } else {
                // The cluster group needs to be partitioned further along the next axis
                let iterator = ClusterGroupIterator::new(
                    elements,
                    self.number_of_clusters_on_axis,
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
    let depth = shallowest_leaf_depth::<T, Params>(elements.len());
    bulk_load_recursive::<_, Params>(elements, depth)
}

/// The depth of the shallowest leaf of the tree the partitioning builds from `len` elements when
/// every cluster decides on its own whether it is a leaf (`len <= MAX_SIZE`). The sizes of the
/// clusters depend on the number of elements only, so this needs no elements, and only the few
/// distinct cluster sizes of every node are visited.
fn shallowest_leaf_depth<T, Params>(len: usize) -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    if len <= Params::MAX_SIZE {
        return 1;
    }
    let clusters_on_axis = calculate_number_of_clusters_on_axis::<T, Params>(len).max(2);
    // Sizes of the clusters along every axis, as `ClusterGroupIterator` slices them: slabs of
    // `ceil(size / clusters_on_axis)` elements and a last one with the rest.
    let mut sizes = vec![len];
    for _ in 0..<T::Envelope as Envelope>::Point::DIMENSIONS {
        let mut next = Vec::new();
        for size in sizes {
            let slab = size.div_ceil(clusters_on_axis);
            let full = size.div_ceil(slab) - 1;
            next.push(slab);
            next.push(size - full * slab);
        }
        next.sort_unstable();
        next.dedup();
        sizes = next;
    }
    1 + sizes
        .into_iter()
        .map(shallowest_leaf_depth::<T, Params>)
        .min()
        .unwrap_or(0)
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

    /// All leaves must be on the same level, for any size: the clusters of a node used to
    /// decide their depth on their own, which put leaves of e.g. 25 elements in 2D onto
    /// different levels and made later insertions panic.
    #[test]
    fn test_bulk_load_leaves_on_one_level() {
        use crate::node::{ParentNode, RTreeNode};
        use crate::params::{DefaultParams, RTreeParams};
        use crate::RStarInsertionStrategy;

        struct SmallParams;
        impl RTreeParams for SmallParams {
            const MIN_SIZE: usize = 2;
            const MAX_SIZE: usize = 4;
            const REINSERTION_COUNT: usize = 1;
            type DefaultInsertionStrategy = RStarInsertionStrategy;
        }

        fn leaf_depths<T: RTreeObject>(node: &ParentNode<T>, depth: usize, out: &mut Vec<usize>) {
            for child in &node.children {
                match child {
                    RTreeNode::Leaf(_) => out.push(depth),
                    RTreeNode::Parent(p) => leaf_depths(p, depth + 1, out),
                }
            }
        }

        fn check<P, Params>(size: usize)
        where
            P: Point<Scalar = i32> + RTreeObject,
            Params: RTreeParams,
        {
            let points = create_random_integers::<P>(size, SEED_1);
            let tree = RTree::<P, Params>::bulk_load_with_params(points);
            assert_eq!(tree.size(), size);
            let mut depths = Vec::new();
            leaf_depths(tree.root(), 0, &mut depths);
            assert!(depths.windows(2).all(|w| w[0] == w[1]), "size {size}");
        }

        let sizes = (0..1300).chain([1537, 1791, 6145, 7167, 7777, 25_000, 100_003]);
        for size in sizes {
            check::<[i32; 2], DefaultParams>(size);
            check::<[i32; 3], DefaultParams>(size);
            check::<[i32; 4], DefaultParams>(size);
            check::<[i32; 2], SmallParams>(size);
        }
    }

    #[test]
    fn test_insert_and_remove_after_bulk_load() {
        // Sizes that used to put leaves on different levels, where the inserts panicked
        for size in [25, 27, 97, 111, 385, 447] {
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
        for size in [388, 400, 444] {
            let to_f64 = |p: [i32; 3]| p.map(f64::from);
            let points: Vec<_> = create_random_integers::<[i32; 3]>(size, SEED_1)
                .into_iter()
                .map(to_f64)
                .collect();
            let more_points: Vec<_> = create_random_integers::<[i32; 3]>(size, SEED_2)
                .into_iter()
                .map(to_f64)
                .collect();
            let mut tree = RTree::bulk_load(points.clone());
            for (old, new) in points.iter().zip(&more_points) {
                assert!(tree.remove(old).is_some());
                tree.insert(*new);
            }
            assert_eq!(tree.size(), size);
            assert!(more_points.iter().all(|p| tree.contains(p)));
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
