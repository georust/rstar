use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};

use super::bulk_load_common::{calculate_number_of_clusters_on_axis, SlabIterator};

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
    let mut resulting_children = Vec::with_capacity(m);

    partition_along_axis::<_, Params>(
        &mut resulting_children,
        elements,
        number_of_clusters_on_axis,
        <T::Envelope as Envelope>::Point::DIMENSIONS,
        depth,
    );
    ParentNodeData::new_parent(resulting_children)
}

fn partition_along_axis<T, Params>(
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
        let child = bulk_load_recursive::<_, Params>(elements, depth - 1);
        result.push(RTreeNode::Parent(child));
    } else {
        for slab in SlabIterator::new(elements, number_of_clusters_on_axis, current_axis - 1) {
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
