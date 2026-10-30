use crate::{Envelope, Point, RTreeObject, RTreeParams};

#[cfg(not(test))]
use alloc::vec::Vec;

/// Partitions elements into slabs along a specific axis.
///
/// The elements are divided into `number_of_clusters` clusters of equal size (to within one
/// element). Each slab holds a run of consecutive clusters, and the clusters are divided among the
/// slabs as equally as possible. The iterator yields each slab with its number of clusters.
pub struct ClusterGroupIterator<T: RTreeObject> {
    remaining: Vec<T>,
    number_of_elements: usize,
    number_of_clusters: usize,
    number_of_slabs: usize,
    next_slab: usize,
    pub cluster_dimension: usize,
}

impl<T: RTreeObject> ClusterGroupIterator<T> {
    pub fn new(
        elements: Vec<T>,
        number_of_clusters: usize,
        number_of_slabs: usize,
        cluster_dimension: usize,
    ) -> Self {
        ClusterGroupIterator {
            number_of_elements: elements.len(),
            remaining: elements,
            number_of_clusters,
            number_of_slabs,
            next_slab: number_of_slabs,
            cluster_dimension,
        }
    }

    /// The index of the first cluster in a slab.
    fn first_cluster(&self, slab: usize) -> usize {
        let share = self.number_of_clusters / self.number_of_slabs;
        let extra = self.number_of_clusters % self.number_of_slabs;
        slab * share + slab.min(extra)
    }

    /// The index of the first element in a cluster.
    fn first_element(&self, cluster: usize) -> usize {
        let n = self.number_of_elements;
        let k = self.number_of_clusters;
        (n / k) * cluster + (n % k) * cluster / k
    }
}

impl<T: RTreeObject> Iterator for ClusterGroupIterator<T> {
    type Item = (Vec<T>, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_slab == 0 {
            return None;
        }
        self.next_slab -= 1;
        let first_cluster = self.first_cluster(self.next_slab);
        let number_of_clusters = self.first_cluster(self.next_slab + 1) - first_cluster;
        if self.next_slab == 0 {
            let mut last = ::core::mem::take(&mut self.remaining);
            // self.remaining retains its full original capacity across iterations
            // (drain doesn't shrink), so the final slab needs shrinking.
            last.shrink_to_fit();
            return Some((last, number_of_clusters));
        }
        let partition_point = self.first_element(first_cluster);
        // Partition so that the slab elements end up at the tail.
        T::Envelope::partition_envelopes(
            self.cluster_dimension,
            &mut self.remaining,
            partition_point,
        );
        // Drain from the end into a new Vec with exact capacity.
        // self.remaining keeps its allocation for reuse on the next iteration.
        let slab = self.remaining.drain(partition_point..).collect();
        Some((slab, number_of_clusters))
    }
}

/// Calculates the number of slabs on the axis that is partitioned next.
///
/// The clusters are divided among the `remaining_axes` axes as evenly as possible: the result
/// is the smallest number of slabs whose power `remaining_axes` is not less than
/// `number_of_clusters`.
pub fn calculate_number_of_slabs(number_of_clusters: usize, remaining_axes: usize) -> usize {
    let mut number_of_slabs = 1;
    while (number_of_slabs as u32)
        .checked_pow(remaining_axes as u32)
        .is_some_and(|clusters| (clusters as usize) < number_of_clusters)
    {
        number_of_slabs += 1;
    }
    number_of_slabs
}

/// Calculates the number of children of each node below the root.
///
/// Each axis is partitioned into the same number of slabs, which is the largest number whose
/// power `DIMENSIONS` is not more than `MAX_SIZE`, and at least two. The result can be
/// larger than `MAX_SIZE`, but it is not smaller than `MIN_SIZE`.
pub fn calculate_number_of_clusters_per_node<T, Params>() -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let dimensions = <T::Envelope as Envelope>::Point::DIMENSIONS as u32;
    let slabs_per_axis_to_power = |slabs: usize| (slabs as u32).checked_pow(dimensions);
    let mut slabs_per_axis = 2;
    while slabs_per_axis_to_power(slabs_per_axis + 1)
        .is_some_and(|clusters| clusters as usize <= Params::MAX_SIZE)
    {
        slabs_per_axis += 1;
    }
    let clusters = slabs_per_axis_to_power(slabs_per_axis).map_or(usize::MAX, |c| c as usize);
    clusters.max(Params::MIN_SIZE)
}

#[cfg(test)]
mod test {
    use super::{calculate_number_of_slabs, ClusterGroupIterator};

    #[test]
    fn test_cluster_group_iterator() {
        const SIZE: usize = 374;
        const NUMBER_OF_CLUSTERS: usize = 7;
        const NUMBER_OF_SLABS: usize = 3;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slabs: Vec<_> =
            ClusterGroupIterator::new(elements, NUMBER_OF_CLUSTERS, NUMBER_OF_SLABS, 0).collect();
        assert_eq!(slabs.len(), NUMBER_OF_SLABS);
        // The slabs come from the tail, so the first slab holds the fewest clusters.
        let clusters: Vec<_> = slabs.iter().map(|(_, clusters)| *clusters).collect();
        assert_eq!(clusters, [2, 2, 3]);
        let mut total_size = 0;
        let mut min_element_for_last_slab = i32::MAX;
        for (slab, clusters) in &slabs {
            // Each cluster holds 53 or 54 elements.
            assert!(slab.len() >= clusters * (SIZE / NUMBER_OF_CLUSTERS));
            assert!(slab.len() <= clusters * SIZE.div_ceil(NUMBER_OF_CLUSTERS));
            total_size += slab.len();
            let current_min = slab.iter().min_by_key(|point| point[0]).unwrap();
            assert!(current_min[0] < min_element_for_last_slab);
            min_element_for_last_slab = current_min[0];
        }
        assert_eq!(total_size, SIZE);
    }

    #[test]
    fn test_calculate_number_of_slabs() {
        assert_eq!(calculate_number_of_slabs(1, 2), 1);
        assert_eq!(calculate_number_of_slabs(2, 2), 2);
        assert_eq!(calculate_number_of_slabs(4, 2), 2);
        assert_eq!(calculate_number_of_slabs(5, 2), 3);
        assert_eq!(calculate_number_of_slabs(8, 3), 2);
        assert_eq!(calculate_number_of_slabs(9, 3), 3);
        assert_eq!(calculate_number_of_slabs(5, 1), 5);
    }

    /// Verify that slabs produced by ClusterGroupIterator don't retain
    /// excessive capacity from the parent Vec.
    #[test]
    fn test_cluster_group_iterator_no_excess_capacity() {
        const SIZE: usize = 10_000;
        const NUMBER_OF_CLUSTERS: usize = 5;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slabs: Vec<_> =
            ClusterGroupIterator::new(elements, NUMBER_OF_CLUSTERS, NUMBER_OF_CLUSTERS, 0)
                .collect();

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
