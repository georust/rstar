use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};

pub fn bulk_load<T, Params>(elements: Vec<T>, depth: usize) -> ParentNodeData<T>
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
    let n_subtree = (m as f32).powi(depth as i32 - 1);
    let remaining_clusters = (elements.len() as f32 / n_subtree).ceil() as usize;

    let max_dimension = <T::Envelope as Envelope>::Point::DIMENSIONS;
    let number_of_clusters_on_axis = (remaining_clusters as f32)
        .powf(1. / max_dimension as f32)
        .ceil() as usize;
    let mut resulting_children = Vec::with_capacity(m + 1);

    let start_state = PartitioningState {
        elements,
        current_axis: max_dimension,
    };

    resulting_children.extend(PartitioningIterator::<_, Params> {
        queue: vec![start_state],
        depth,
        number_of_clusters_on_axis,
        _params: Default::default(),
    });

    ParentNodeData::new_parent(resulting_children)
}

struct SlabIterator<T: RTreeObject> {
    remaining: Vec<T>,
    slab_size: usize,
    cluster_dimension: usize,
}

fn create_slabs<T>(
    elements: Vec<T>,
    slab_size: usize,
    cluster_dimension: usize,
) -> impl Iterator<Item = Vec<T>>
where
    T: RTreeObject,
{
    SlabIterator {
        remaining: elements,
        slab_size,
        cluster_dimension,
    }
}

struct PartitioningState<T: RTreeObject> {
    elements: Vec<T>,
    current_axis: usize,
}

struct PartitioningIterator<T: RTreeObject, Params: RTreeParams> {
    queue: Vec<PartitioningState<T>>,
    depth: usize,
    number_of_clusters_on_axis: usize,
    _params: std::marker::PhantomData<Params>,
}

impl<T: RTreeObject, Params: RTreeParams> Iterator for PartitioningIterator<T, Params> {
    type Item = RTreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.queue.pop() {
            let PartitioningState {
                elements,
                current_axis,
            } = next;
            if current_axis == 0 {
                let data = bulk_load::<_, Params>(elements, self.depth - 1);
                return RTreeNode::Parent(data).into();
            } else {
                let slab_size = div_up(elements.len(), self.number_of_clusters_on_axis);
                self.queue
                    .extend(
                        create_slabs(elements, slab_size, current_axis - 1).map(|slab| {
                            PartitioningState {
                                elements: slab,
                                current_axis: current_axis - 1,
                            }
                        }),
                    );
            }
        }
        None
    }
}

/* fn partition_along_axis<T, Params>(
    result: &mut Vec<RTreeNode<T>>,
    elements: Vec<T>,
    number_of_clusters_on_axis: usize,
    current_axis: usize,
    depth: usize,
) where
    T: RTreeObject,
    Params: RTreeParams,
{
    if current_axis == 0 {
        let child = bulk_load::<_, Params>(elements, depth - 1);
        result.push(RTreeNode::Parent(child));
    } else {
        let slab_size = div_up(elements.len(), number_of_clusters_on_axis);
        for slab in create_slabs(elements, slab_size, current_axis - 1) {
            partition_along_axis::<_, Params>(
                result,
                slab,
                number_of_clusters_on_axis,
                current_axis - 1,
                depth,
            );
        }
    }
}
 */
fn div_up(dividend: usize, divisor: usize) -> usize {
    (dividend + divisor - 1) / divisor
}

impl<T> Iterator for SlabIterator<T>
where
    T: RTreeObject,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.remaining.len() {
            0 => None,
            len if len <= self.slab_size => ::std::mem::replace(&mut self.remaining, vec![]).into(),
            _ => {
                let slab_axis = self.cluster_dimension;
                T::Envelope::partition_envelopes(slab_axis, &mut self.remaining, self.slab_size);
                let off_split = self.remaining.split_off(self.slab_size);
                ::std::mem::replace(&mut self.remaining, off_split).into()
            }
        }
    }
}

pub fn bulk_load_with_params<T, Params>(elements: Vec<T>) -> ParentNodeData<T>
where
    T: RTreeObject,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    let depth = (elements.len() as f32).log(m as f32).ceil() as usize;
    bulk_load::<_, Params>(elements, depth)
}

#[cfg(test)]
mod test {
    use super::create_slabs;
    use crate::test_utilities::{create_random_integers, SEED_1};
    use crate::{Point, RTree};
    use std::collections::HashSet;

    #[test]
    fn test_create_slabs() {
        const SIZE: usize = 374;
        const SLAB_SIZE: usize = 10;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slabs: Vec<_> = create_slabs(elements, SLAB_SIZE, 0).collect();
        assert_eq!(slabs.len(), (SIZE + SLAB_SIZE) / SLAB_SIZE);
        for slab in &slabs[0..slabs.len() - 1] {
            assert_eq!(slab.len(), SLAB_SIZE);
        }
        let mut total_size = 0;
        let mut max_element_for_last_slab = i32::min_value();
        for slab in &slabs {
            total_size += slab.len();
            let current_max = slab.iter().max_by_key(|point| point[0]).unwrap();
            assert!(current_max[0] > max_element_for_last_slab);
            max_element_for_last_slab = current_max[0];
        }
        assert_eq!(total_size, SIZE);
    }

    #[test]
    fn test_bulk_load_small() {
        let random_points = create_random_integers::<[i32; 2]>(50, SEED_1);
        let tree = RTree::bulk_load(random_points.clone());
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = random_points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), random_points.len());
    }

    #[test]
    fn test_bulk_load() {
        let random_points = create_random_integers::<[i32; 2]>(1000, SEED_1);
        let tree = RTree::bulk_load(random_points.clone());
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = random_points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), random_points.len());
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
        P: Point<Scalar = i32> + Eq + std::hash::Hash,
    {
        let random_points = create_random_integers::<P>(size, SEED_1);
        let expected: HashSet<_> = random_points.iter().cloned().collect();
        let tree = RTree::bulk_load(random_points);
        let actual: HashSet<_> = tree.iter().cloned().collect();
        assert_eq!(actual, expected);
        assert_eq!(tree.size(), size);
    }
}
