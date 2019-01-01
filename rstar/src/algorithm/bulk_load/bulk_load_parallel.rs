use super::bulk_load_common::{calculate_number_of_clusters_on_axis, ClusterGroupIterator};
use super::bulk_load_sequential::bulk_load_sequential;
use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};
use std::sync::mpsc::{channel, Sender};
use threadpool::ThreadPool;

/// Packs all given elements into a single RTree parent node
///
/// The root's child nodes are calculated in parallel on several threads. Each thread performs sequential bulk loading.
/// This coarsely grained work distribution may not always achieve best thread utilization but minimizes
///  synchronization overhead.
pub fn bulk_load_parallel<T, Params>(elements: Vec<T>) -> ParentNodeData<T>
where
    T: RTreeObject + Send + Sync + 'static,
    T::Envelope: Send + Sync,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    if elements.len() <= Params::MAX_SIZE {
        // Partitioning the root doesn't make sense if it has only leafs.
        bulk_load_sequential::<_, Params>(elements)
    } else {
        let (result_channel, receiver) = channel();
        let expected_number_of_children =
            partition_root_in_parallel::<_, Params>(elements, &result_channel);
        ParentNodeData::new_parent(receiver.iter().take(expected_number_of_children).collect())
    }
}

enum PartitioningWorkItem<T: RTreeObject + Send + Sync> {
    CreatePartitions {
        elements: Vec<T>,
        current_axis: usize,
    },
    // This work item consists of a (costly) call of `.next()`.
    // Creating partition groups can be time consuming as it requires a selection algorithm.
    CreatePartitionGroups(ClusterGroupIterator<T>),
}

/// This method is similar to the sequentially performing partitioning iterator. It sends all
/// resulting children over a result channel.
/// The method returns the number of children the root will be split into.
fn partition_root_in_parallel<T, Params>(
    elements: Vec<T>,
    result_channel: &Sender<RTreeNode<T>>,
) -> usize
where
    T: RTreeObject + Send + Sync + 'static,
    T::Envelope: Send + Sync + 'static,
    Params: RTreeParams,
{
    let pool = ThreadPool::default();
    let number_of_clusters_on_axis =
        calculate_number_of_clusters_on_axis::<T, Params>(elements.len());

    let mut expected_children = 0;
    let mut queue = vec![PartitioningWorkItem::CreatePartitions {
        elements,
        current_axis: <T::Envelope as Envelope>::Point::DIMENSIONS,
    }];
    while let Some(next) = queue.pop() {
        match next {
            PartitioningWorkItem::CreatePartitions {
                elements,
                current_axis,
            } => {
                if current_axis == 0 {
                    let result_channel_copy = result_channel.clone();
                    pool.execute(move || {
                        // All spawned sub tasks perform the loading sequentially to minimize
                        // synchronization overhead
                        let data = bulk_load_sequential::<_, Params>(elements);
                        result_channel_copy.send(RTreeNode::Parent(data)).unwrap();
                    });
                    expected_children += 1;
                } else {
                    let slab_iterator = ClusterGroupIterator::new(
                        elements,
                        number_of_clusters_on_axis,
                        current_axis - 1,
                    );
                    queue.push(PartitioningWorkItem::CreatePartitionGroups(slab_iterator));
                }
            }
            PartitioningWorkItem::CreatePartitionGroups(mut iter) => {
                if let Some(slab) = iter.next() {
                    let current_axis = iter.cluster_dimension;
                    queue.push(PartitioningWorkItem::CreatePartitionGroups(iter));
                    // In order to start working in parallel as soon as possible, a partitioning task should be
                    // put onto the work stack last.
                    queue.push(PartitioningWorkItem::CreatePartitions {
                        elements: slab,
                        current_axis,
                    });
                }
            }
        }
    }
    expected_children
}
