use crate::{Envelope, Point, RTreeObject, RTreeParams};

use alloc::{vec, vec::Vec};

#[allow(unused_imports)] // Import is required when building without std
use num_traits::Float;

/// Partitions elements into groups of clusters along a specific axis.
pub struct ClusterGroupIterator<T: RTreeObject> {
    remaining: Vec<T>,
    slab_size: usize,
    pub cluster_dimension: usize,
}

impl<T: RTreeObject> ClusterGroupIterator<T> {
    pub fn new(
        elements: Vec<T>,
        number_of_clusters_on_axis: usize,
        cluster_dimension: usize,
    ) -> Self {
        let slab_size = div_up(elements.len(), number_of_clusters_on_axis);
        ClusterGroupIterator {
            remaining: elements,
            slab_size,
            cluster_dimension,
        }
    }
}

impl<T: RTreeObject> Iterator for ClusterGroupIterator<T> {
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.remaining.len() {
            0 => None,
            len if len <= self.slab_size => {
                ::core::mem::replace(&mut self.remaining, vec![]).into()
            }
            _ => {
                let slab_axis = self.cluster_dimension;
                T::Envelope::partition_envelopes(slab_axis, &mut self.remaining, self.slab_size);
                let off_split = self.remaining.split_off(self.slab_size);
                ::core::mem::replace(&mut self.remaining, off_split).into()
            }
        }
    }
}

/// Calculates the desired number of clusters on any axis
///
/// A 'cluster' refers to a set of elements that will finally form an rtree node.
pub fn calculate_number_of_clusters_on_axis<T, Params>(number_of_elements: usize) -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let max_size = Params::MAX_SIZE as f32;
    // The depth of the resulting tree, assuming all leaf nodes will be filled up to MAX_SIZE
    let depth = (number_of_elements as f32).log(max_size).ceil() as usize;
    // The number of elements each subtree will hold
    let n_subtree = (max_size as f32).powi(depth as i32 - 1);
    // How many clusters will this node contain
    let number_of_clusters = (number_of_elements as f32 / n_subtree).ceil();

    let max_dimension = <T::Envelope as Envelope>::Point::DIMENSIONS as f32;
    // Try to split all clusters among all dimensions as evenly as possible by taking the nth root.
    number_of_clusters.powf(1. / max_dimension).ceil() as usize
}

fn div_up(dividend: usize, divisor: usize) -> usize {
    (dividend + divisor - 1) / divisor
}

#[cfg(test)]
mod test {
    use super::ClusterGroupIterator;

    #[test]
    fn test_cluster_group_iterator() {
        const SIZE: usize = 374;
        const NUMBER_OF_CLUSTERS_ON_AXIS: usize = 5;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slab_size = (elements.len()) / NUMBER_OF_CLUSTERS_ON_AXIS + 1;
        let slabs: Vec<_> =
            ClusterGroupIterator::new(elements, NUMBER_OF_CLUSTERS_ON_AXIS, 0).collect();
        assert_eq!(slabs.len(), NUMBER_OF_CLUSTERS_ON_AXIS);
        for slab in &slabs[0..slabs.len() - 1] {
            assert_eq!(slab.len(), slab_size);
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
}
