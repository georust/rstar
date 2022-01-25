use crate::envelope::Envelope;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;

use alloc::{vec, vec::Vec};

#[allow(unused_imports)] // Import is required when building without std
use num_traits::Float;

use super::cluster_group_iterator::{calculate_number_of_clusters_on_axis, ClusterGroupIterator};

fn bulk_load_recursive<T, Params>(elements: Vec<T>, depth: usize) -> ParentNode<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    if elements.len() <= m {
        // Reached leaf level
        let elements: Vec<_> = elements.into_iter().map(RTreeNode::Leaf).collect();
        return ParentNode::new_parent(elements);
    }
    let number_of_clusters_on_axis =
        calculate_number_of_clusters_on_axis::<T, Params>(elements.len());

    let iterator = PartitioningTask::<_, Params> {
        number_of_clusters_on_axis,
        depth,
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
    depth: usize,
    number_of_clusters_on_axis: usize,
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
                let data = bulk_load_recursive::<_, Params>(elements, self.depth - 1);
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
    let m = Params::MAX_SIZE;
    let depth = (elements.len() as f32).log(m as f32).ceil() as usize;
    bulk_load_recursive::<_, Params>(elements, depth)
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
}
