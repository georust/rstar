use super::bulk_load_common::{calculate_number_of_clusters_on_axis, SlabIterator};
use super::bulk_load_sequential::bulk_load_sequential;
use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};
use std::sync::mpsc::{channel, Sender};
use threadpool::ThreadPool;

pub fn bulk_load_parallel<T, Params>(elements: Vec<T>) -> ParentNodeData<T>
where
    T: RTreeObject + Send + Sync + 'static,
    T::Envelope: Send + Sync,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let max_size = Params::MAX_SIZE;
    if elements.len() <= max_size {
        // Reached leaf level
        let elements: Vec<_> = elements.into_iter().map(RTreeNode::Leaf).collect();
        return ParentNodeData::new_parent(elements);
    }
    let number_of_clusters_on_axis =
        calculate_number_of_clusters_on_axis::<T, Params>(elements.len());

    let initial_state = PartitioningState::CreatePartitions {
        elements,
        current_axis: <T::Envelope as Envelope>::Point::DIMENSIONS,
    };

    let (sender, receiver) = channel();

    let mut iterator = PartitioningIterator::<_, Params> {
        queue: vec![initial_state],
        number_of_clusters_on_axis,
        sender,
        pool: Default::default(),
        _params: Default::default(),
    };

    let expected = iterator.partition_along_axis();
    ParentNodeData::new_parent(receiver.iter().take(expected).collect())
}

enum PartitioningState<T: RTreeObject + Send + Sync> {
    CreatePartitions {
        elements: Vec<T>,
        current_axis: usize,
    },
    CreateSlabs(SlabIterator<T>),
}

struct PartitioningIterator<T: RTreeObject + Send + Sync, Params: RTreeParams> {
    queue: Vec<PartitioningState<T>>,
    number_of_clusters_on_axis: usize,
    sender: Sender<RTreeNode<T>>,
    pool: ThreadPool,
    _params: std::marker::PhantomData<Params>,
}

impl<T, Params> PartitioningIterator<T, Params>
where
    T: RTreeObject + Send + Sync + 'static,
    T::Envelope: Send + Sync,
    Params: RTreeParams,
{
    fn partition_along_axis(&mut self) -> usize {
        let mut expected_children = 0;
        while let Some(next) = self.queue.pop() {
            match next {
                PartitioningState::CreatePartitions {
                    elements,
                    current_axis,
                } => {
                    if current_axis == 0 {
                        let sender_copy = self.sender.clone();
                        self.pool.execute(move || {
                            let data = bulk_load_sequential::<_, Params>(elements);
                            sender_copy.send(RTreeNode::Parent(data)).unwrap();
                        });
                        expected_children += 1;
                    } else {
                        let slab_iterator = SlabIterator::new(
                            elements,
                            self.number_of_clusters_on_axis,
                            current_axis - 1,
                        );
                        self.queue
                            .push(PartitioningState::CreateSlabs(slab_iterator));
                    }
                }
                PartitioningState::CreateSlabs(mut iter) => {
                    if let Some(slab) = iter.next() {
                        let current_axis = iter.cluster_dimension();
                        self.queue.push(PartitioningState::CreateSlabs(iter));
                        self.queue.push(PartitioningState::CreatePartitions {
                            elements: slab,
                            current_axis,
                        });
                    }
                }
            }
        }
        expected_children
    }
}
