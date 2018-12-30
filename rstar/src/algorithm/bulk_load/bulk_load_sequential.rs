use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};

use super::bulk_load_common::{calculate_number_of_clusters_on_axis, ClusterGroupIterator};

fn bulk_load_recursive<T, Params>(elements: Vec<T>, depth: usize) -> ParentNodeData<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    if elements.len() <= m {
        // Reached leaf level
        let elements: Vec<_> = elements.into_iter().map(RTreeNode::Leaf).collect();
        return ParentNodeData::new_parent(elements);
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
    ParentNodeData::new_parent(iterator.collect())
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
    _params: std::marker::PhantomData<Params>,
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
pub fn bulk_load_sequential<T, Params>(elements: Vec<T>) -> ParentNodeData<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    let depth = (elements.len() as f32).log(m as f32).ceil() as usize;
    bulk_load_recursive::<_, Params>(elements, depth)
}
