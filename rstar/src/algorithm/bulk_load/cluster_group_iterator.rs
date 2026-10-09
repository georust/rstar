use crate::{Envelope, Point, RTreeObject, RTreeParams};
use smallvec::SmallVec;

#[cfg(not(test))]
use alloc::vec::Vec;

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
        let slab_size = elements.len().div_ceil(number_of_clusters_on_axis);
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
                let mut last = ::core::mem::take(&mut self.remaining);
                // self.remaining retains its full original capacity across iterations
                // (drain doesn't shrink), so the final slab needs shrinking.
                last.shrink_to_fit();
                last.into()
            }
            len => {
                let slab_axis = self.cluster_dimension;
                let partition_point = len - self.slab_size;
                // Partition so that the slab elements end up at the tail.
                T::Envelope::partition_envelopes(slab_axis, &mut self.remaining, partition_point);
                // Drain from the end into a new Vec with exact capacity.
                // self.remaining keeps its allocation for reuse on the next iteration.
                self.remaining
                    .drain(partition_point..)
                    .collect::<Vec<_>>()
                    .into()
            }
        }
    }
}

/// Calculates how many clusters a node holding `number_of_elements` elements
/// should be split into.
///
/// A 'cluster' refers to a set of elements that will finally form an rtree node.
/// The result is the node's fan-out and is therefore capped at `MAX_SIZE`.
pub fn calculate_number_of_clusters<Params>(number_of_elements: usize) -> usize
where
    Params: RTreeParams,
{
    let max_size = Params::MAX_SIZE as f32;
    // The depth of the resulting tree, assuming all leaf nodes will be filled up to MAX_SIZE
    let depth = (number_of_elements as f32).log(max_size).ceil() as i32;
    // The number of elements each subtree will hold
    let n_subtree = max_size.powi(depth - 1);
    // How many clusters will this node contain
    let number_of_clusters = (number_of_elements as f32 / n_subtree).ceil();

    // `number_of_clusters` cannot exceed `MAX_SIZE` mathematically, but it is
    // computed in `f32` and is clamped here to stay robust against rounding.
    (number_of_clusters as usize).clamp(2, Params::MAX_SIZE)
}

/// Distributes a node's fan-out over the axes, returning how many cuts to make
/// on each one.
///
/// The OMT paper is written for two dimensional data and cuts `sqrt(N)` slabs
/// on each of the two axes. Taking the same number of cuts `k` on every axis
/// generalises badly: the node ends up with `k ^ DIMENSIONS` children, which
/// exceeds `MAX_SIZE` as soon as there are more than two dimensions -- with the
/// default parameters a three dimensional bulk load produced nodes with 8
/// children and a six dimensional one nodes with 64.
///
/// Cuts are therefore handed out one at a time, always to the axis that has
/// been cut least so far, and only while the resulting product stays within
/// `MAX_SIZE`. Axes that receive no cut are simply not partitioned, which is
/// what keeps the fan-out bounded in high dimensions.
pub fn calculate_cuts_per_axis<T, Params>(
    number_of_elements: usize,
    first_axis: usize,
) -> SmallVec<[usize; 8]>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let dimensions = <T::Envelope as Envelope>::Point::DIMENSIONS;
    let max_size = Params::MAX_SIZE;
    let target = calculate_number_of_clusters::<Params>(number_of_elements);

    let mut cuts = SmallVec::from_elem(1usize, dimensions);
    let mut fan_out = 1usize;
    while fan_out < target {
        // Pick the least-cut axis whose next cut keeps the fan-out within
        // `MAX_SIZE`; stop when no axis can be cut any further. Ties are broken
        // by starting the scan at `first_axis`, which is what rotates the cut
        // axes from level to level.
        let mut chosen: Option<usize> = None;
        for offset in 0..dimensions {
            let axis = (first_axis + offset) % dimensions;
            if fan_out / cuts[axis] * (cuts[axis] + 1) > max_size {
                continue;
            }
            if chosen.is_none_or(|best| cuts[axis] < cuts[best]) {
                chosen = Some(axis);
            }
        }
        let Some(axis) = chosen else { break };
        fan_out = fan_out / cuts[axis] * (cuts[axis] + 1);
        cuts[axis] += 1;
    }
    cuts
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
        let mut min_element_for_last_slab = i32::MAX;
        for slab in &slabs {
            total_size += slab.len();
            let current_min = slab.iter().min_by_key(|point| point[0]).unwrap();
            assert!(current_min[0] < min_element_for_last_slab);
            min_element_for_last_slab = current_min[0];
        }
        assert_eq!(total_size, SIZE);
    }

    /// Verify that slabs produced by ClusterGroupIterator don't retain
    /// excessive capacity from the parent Vec.
    #[test]
    fn test_cluster_group_iterator_no_excess_capacity() {
        const SIZE: usize = 10_000;
        const NUMBER_OF_CLUSTERS_ON_AXIS: usize = 5;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slabs: Vec<_> =
            ClusterGroupIterator::new(elements, NUMBER_OF_CLUSTERS_ON_AXIS, 0).collect();

        for (i, slab) in slabs.iter().enumerate() {
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
