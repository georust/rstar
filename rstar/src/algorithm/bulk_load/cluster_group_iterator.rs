use crate::{Envelope, RTreeObject};

#[cfg(not(test))]
use alloc::vec::Vec;

/// Partitions elements into groups of clusters along a specific axis.
///
/// A 'cluster' refers to a set of elements that will finally form an rtree node. The clusters
/// are distributed evenly among the groups and the elements evenly among the clusters, so the
/// sizes of any two clusters differ by one element at most.
pub struct ClusterGroupIterator<T: RTreeObject> {
    remaining: Vec<T>,
    number_of_elements: usize,
    number_of_clusters: usize,
    number_of_groups: usize,
    remaining_groups: usize,
    pub cluster_dimension: usize,
}

impl<T: RTreeObject> ClusterGroupIterator<T> {
    pub fn new(
        elements: Vec<T>,
        number_of_clusters: usize,
        number_of_groups: usize,
        cluster_dimension: usize,
    ) -> Self {
        debug_assert!(number_of_groups <= number_of_clusters);
        debug_assert!(number_of_clusters <= elements.len());
        ClusterGroupIterator {
            number_of_elements: elements.len(),
            remaining: elements,
            number_of_clusters,
            number_of_groups,
            remaining_groups: number_of_groups,
            cluster_dimension,
        }
    }

    /// The number of clusters in the first `groups` groups.
    fn clusters_before(&self, groups: usize) -> usize {
        groups * self.number_of_clusters / self.number_of_groups
    }
}

impl<T: RTreeObject> Iterator for ClusterGroupIterator<T> {
    /// The elements of a group and the number of clusters to partition them into.
    type Item = (Vec<T>, usize);

    fn next(&mut self) -> Option<Self::Item> {
        // Groups are split off from the end
        let group = self.remaining_groups.checked_sub(1)?;
        self.remaining_groups = group;
        let clusters_before = self.clusters_before(group);
        let clusters = self.clusters_before(group + 1) - clusters_before;
        if group == 0 {
            let mut last = ::core::mem::take(&mut self.remaining);
            // self.remaining retains its full original capacity across iterations
            // (drain doesn't shrink), so the final slab needs shrinking.
            last.shrink_to_fit();
            return (last, clusters).into();
        }
        let slab_axis = self.cluster_dimension;
        let partition_point = clusters_before * self.number_of_elements / self.number_of_clusters;
        // Partition so that the slab elements end up at the tail.
        T::Envelope::partition_envelopes(slab_axis, &mut self.remaining, partition_point);
        // Drain from the end into a new Vec with exact capacity.
        // self.remaining keeps its allocation for reuse on the next iteration.
        let slab = self.remaining.drain(partition_point..).collect::<Vec<_>>();
        (slab, clusters).into()
    }
}

#[cfg(test)]
mod test {
    use super::ClusterGroupIterator;

    #[test]
    fn test_cluster_group_iterator() {
        const SIZE: usize = 374;
        const NUMBER_OF_CLUSTERS: usize = 13;
        const NUMBER_OF_GROUPS: usize = 5;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slabs: Vec<_> =
            ClusterGroupIterator::new(elements, NUMBER_OF_CLUSTERS, NUMBER_OF_GROUPS, 0).collect();
        assert_eq!(slabs.len(), NUMBER_OF_GROUPS);
        let mut total_size = 0;
        let mut total_clusters = 0;
        let mut min_element_for_last_slab = i32::MAX;
        for (slab, clusters) in &slabs {
            // Every group holds two or three clusters of 28 or 29 elements
            assert!((2..=3).contains(clusters));
            assert!(slab.len() >= clusters * (SIZE / NUMBER_OF_CLUSTERS));
            assert!(slab.len() <= clusters * SIZE.div_ceil(NUMBER_OF_CLUSTERS));
            total_size += slab.len();
            total_clusters += clusters;
            let current_min = slab.iter().min_by_key(|point| point[0]).unwrap();
            assert!(current_min[0] < min_element_for_last_slab);
            min_element_for_last_slab = current_min[0];
        }
        assert_eq!(total_size, SIZE);
        assert_eq!(total_clusters, NUMBER_OF_CLUSTERS);
    }

    /// Verify that slabs produced by ClusterGroupIterator don't retain
    /// excessive capacity from the parent Vec.
    #[test]
    fn test_cluster_group_iterator_no_excess_capacity() {
        const SIZE: usize = 10_000;
        const NUMBER_OF_GROUPS: usize = 5;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slabs: Vec<_> =
            ClusterGroupIterator::new(elements, NUMBER_OF_GROUPS, NUMBER_OF_GROUPS, 0).collect();

        for (i, (slab, _)) in slabs.iter().enumerate() {
            let ratio = slab.capacity() as f64 / slab.len() as f64;
            assert!(
                ratio <= 2.0,
                "slab {i} has excessive capacity: len={}, cap={}, ratio={ratio:.1}x",
                slab.len(),
                slab.capacity(),
            );
        }
    }
}
