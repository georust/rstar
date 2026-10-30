use crate::envelope::Envelope;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::{Point, PointExt, RTreeNum};

#[cfg(not(test))]
use alloc::{vec, vec::Vec};

use super::cluster_group_iterator::{
    calculate_number_of_clusters_per_node, calculate_number_of_slabs, ClusterGroupIterator,
};

/// The number of elements from which the root finds the extent of the data
const SAMPLE_SIZE: usize = 1024;

/// Builds a node with `height` levels (a leaf node has height 1) and `number_of_clusters`
/// children. Each node below it has `clusters_per_node` children.
///
/// `axes` gives the order in which the node partitions the axes. If it is `None`, the node
/// partitions the axes from the last to the first.
fn bulk_load_recursive<T, Params>(
    mut elements: Vec<T>,
    height: usize,
    number_of_clusters: usize,
    clusters_per_node: usize,
    axes: Option<Vec<usize>>,
) -> ParentNode<T>
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
    let iterator = PartitioningTask::<_, Params> {
        work_queue: vec![PartitioningState {
            current_axis: <T::Envelope as Envelope>::Point::DIMENSIONS,
            number_of_clusters,
            elements,
        }],
        subtree_height: height - 1,
        clusters_per_node,
        axes,
        _params: Default::default(),
    };
    ParentNode::new_parent(iterator.collect())
}

/// Represents a partitioning task that still needs to be done.
///
/// A partitioning iterator will take this item from its work queue and start partitioning "elements"
/// into "number_of_clusters" clusters. "current_axis" is the number of axes that are not
/// partitioned yet.
struct PartitioningState<T: RTreeObject> {
    elements: Vec<T>,
    current_axis: usize,
    number_of_clusters: usize,
}

/// Successively partitions the given elements into  cluster groups and finally into clusters.
struct PartitioningTask<T: RTreeObject, Params: RTreeParams> {
    work_queue: Vec<PartitioningState<T>>,
    subtree_height: usize,
    clusters_per_node: usize,
    axes: Option<Vec<usize>>,
    _params: core::marker::PhantomData<Params>,
}

impl<T: RTreeObject, Params: RTreeParams> Iterator for PartitioningTask<T, Params> {
    type Item = RTreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.work_queue.pop() {
            let PartitioningState {
                elements,
                current_axis,
                number_of_clusters,
            } = next;
            if number_of_clusters == 1 {
                // Partitioning finished successfully. The remaining cluster forms a new node
                let data = bulk_load_recursive::<_, Params>(
                    elements,
                    self.subtree_height,
                    self.clusters_per_node,
                    self.clusters_per_node,
                    None,
                );
                return RTreeNode::Parent(data).into();
            } else {
                // The cluster group needs to be partitioned further along the next axis
                let number_of_slabs = calculate_number_of_slabs(number_of_clusters, current_axis);
                let axis = match &self.axes {
                    Some(axes) => axes[axes.len() - current_axis],
                    None => current_axis - 1,
                };
                let iterator =
                    ClusterGroupIterator::new(elements, number_of_clusters, number_of_slabs, axis);
                self.work_queue
                    .extend(
                        iterator.map(|(slab, number_of_clusters)| PartitioningState {
                            elements: slab,
                            current_axis: current_axis - 1,
                            number_of_clusters,
                        }),
                    );
            }
        }
        None
    }
}

/// Calculates the height of the tree and the number of children of the root.
///
/// All nodes below the root have `clusters_per_node` children, and all leaf nodes hold between
/// `MIN_SIZE` and `MAX_SIZE` elements. The root has between 2 and `2 * clusters_per_node`
/// children, so that the tree gets a new level only if a root with
/// `2 * clusters_per_node` children cannot hold the elements. Of the numbers of children that
/// are possible, the root takes the number nearest to `clusters_per_node`.
fn calculate_root_layout<Params: RTreeParams>(
    number_of_elements: usize,
    clusters_per_node: usize,
) -> (usize, usize) {
    let (min_size, max_size) = (Params::MIN_SIZE, Params::MAX_SIZE);
    if number_of_elements <= max_size {
        return (1, 1);
    }
    let max_root_clusters = 2 * clusters_per_node;
    // The number of leaf nodes below each child of the root
    let mut leaves_per_cluster = 1;
    let mut height = 2;
    while number_of_elements
        > max_size
            .saturating_mul(max_root_clusters)
            .saturating_mul(leaves_per_cluster)
    {
        leaves_per_cluster *= clusters_per_node;
        height += 1;
    }
    let fewest_clusters = number_of_elements.div_ceil(max_size * leaves_per_cluster);
    let most_clusters = number_of_elements / (min_size * leaves_per_cluster);
    let number_of_clusters = clusters_per_node.min(most_clusters).max(fewest_clusters);
    (height, number_of_clusters)
}

/// Orders the axes by the extent of a sample of the elements, longest first.
///
/// Axes with the same extent keep the order from the last axis to the first.
fn calculate_axes_by_extent<T>(elements: &[T]) -> Vec<usize>
where
    T: RTreeObject,
{
    let step = (elements.len() / SAMPLE_SIZE).max(1);
    let mut centers = elements
        .iter()
        .step_by(step)
        .map(|element| element.envelope().center());
    let mut axes: Vec<usize> = (0..<T::Envelope as Envelope>::Point::DIMENSIONS)
        .rev()
        .collect();
    let Some(first) = centers.next() else {
        return axes;
    };
    let (lower, upper) = centers.fold((first.clone(), first), |(lower, upper), center| {
        (lower.min_point(&center), upper.max_point(&center))
    });
    let extent = upper.sub(&lower);
    axes.sort_by_key(|axis| core::cmp::Reverse(extent.nth(*axis).ord()));
    axes
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
    let clusters_per_node = calculate_number_of_clusters_per_node::<T, Params>();
    let (height, number_of_clusters) =
        calculate_root_layout::<Params>(elements.len(), clusters_per_node);
    // Below the root, and in a root with `clusters_per_node` children, all axes have the same
    // number of slabs. Otherwise, the longest axes get the most slabs.
    let axes = (height > 1 && number_of_clusters != clusters_per_node)
        .then(|| calculate_axes_by_extent(&elements));
    bulk_load_recursive::<_, Params>(
        elements,
        height,
        number_of_clusters,
        clusters_per_node,
        axes,
    )
}

#[cfg(test)]
mod test {
    use super::{calculate_axes_by_extent, calculate_root_layout};
    use crate::node::{ParentNode, RTreeNode};
    use crate::test_utilities::*;
    use crate::{DefaultParams, Point, RTree, RTreeObject, RTreeParams};
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

    struct OddParams;

    impl RTreeParams for OddParams {
        const MIN_SIZE: usize = 3;
        const MAX_SIZE: usize = 5;
        const REINSERTION_COUNT: usize = 1;
        type DefaultInsertionStrategy = crate::RStarInsertionStrategy;
    }

    struct LargeParams;

    impl RTreeParams for LargeParams {
        const MIN_SIZE: usize = 10;
        const MAX_SIZE: usize = 20;
        const REINSERTION_COUNT: usize = 5;
        type DefaultInsertionStrategy = crate::RStarInsertionStrategy;
    }

    /// Verify that all leaves are at the same depth, and that all nodes below the root have at
    /// least MIN_SIZE children.
    #[test]
    fn test_bulk_load_balanced() {
        let sizes = (0..600).chain((0..22).map(|i| (1.25f64.powi(i) * 600.) as usize));
        for size in sizes {
            check_balanced::<[i32; 2], DefaultParams>(size);
            check_balanced::<[i32; 3], DefaultParams>(size);
            check_balanced::<[i32; 4], DefaultParams>(size);
            check_balanced::<[i32; 2], OddParams>(size);
            check_balanced::<[i32; 3], OddParams>(size);
            check_balanced::<[i32; 2], LargeParams>(size);
            check_balanced::<[i32; 3], LargeParams>(size);
        }
    }

    fn check_balanced<P, Params>(size: usize)
    where
        P: Point<Scalar = i32> + RTreeObject + Send + Sync + Eq + Clone + Debug + Hash + 'static,
        P::Envelope: Send + Sync,
        Params: RTreeParams,
    {
        let points = create_random_integers::<P>(size, SEED_1);
        let tree = RTree::<P, Params>::bulk_load_with_params(points);
        assert_eq!(tree.size(), size);
        tree.root().sanity_check::<Params>(false);
        let mut stack: Vec<&ParentNode<P>> = vec![tree.root()];
        while let Some(node) = stack.pop() {
            for child in &node.children {
                match child {
                    RTreeNode::Leaf(_) => assert!(node.children.len() <= Params::MAX_SIZE),
                    RTreeNode::Parent(parent) => stack.push(parent),
                }
            }
        }
    }

    #[test]
    fn test_calculate_root_layout() {
        // 3 * 4^7 <= 50,000 <= 6 * 4^7: a full grid of 4^7 leaves holds the elements.
        assert_eq!(calculate_root_layout::<DefaultParams>(50_000, 4), (8, 4));
        // 6 * 4^7 < 100,000: the root takes a fifth child instead of a new level.
        assert_eq!(calculate_root_layout::<DefaultParams>(100_000, 4), (8, 5));
        // 8 * 6 * 4^6 < 200,000: the tree gets a new level.
        assert_eq!(calculate_root_layout::<DefaultParams>(200_000, 4), (9, 4));
        assert_eq!(calculate_root_layout::<DefaultParams>(6, 4), (1, 1));
        assert_eq!(calculate_root_layout::<DefaultParams>(7, 4), (2, 2));
    }

    #[test]
    fn test_calculate_axes_by_extent() {
        let points: Vec<[i32; 3]> = (0..5000).map(|i| [i % 100, i % 10, i]).collect();
        assert_eq!(calculate_axes_by_extent(&points), [2, 0, 1]);
        // Axes with the same extent keep the order from the last axis to the first.
        let points: Vec<[i32; 3]> = (0..5000).map(|i| [i, i % 10, i]).collect();
        assert_eq!(calculate_axes_by_extent(&points), [2, 0, 1]);
        let points: Vec<[i32; 3]> = (0..5000).map(|i| [i, i, i]).collect();
        assert_eq!(calculate_axes_by_extent(&points), [2, 1, 0]);
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
