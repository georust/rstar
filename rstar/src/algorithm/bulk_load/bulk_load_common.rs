use crate::{Envelope, Point, RTreeObject, RTreeParams};

pub struct SlabIterator<T: RTreeObject> {
    remaining: Vec<T>,
    slab_size: usize,
    cluster_dimension: usize,
}

impl<T: RTreeObject> SlabIterator<T> {
    pub fn new(
        elements: Vec<T>,
        number_of_clusters_on_axis: usize,
        cluster_dimension: usize,
    ) -> Self {
        let slab_size = div_up(elements.len(), number_of_clusters_on_axis);
        SlabIterator {
            remaining: elements,
            slab_size,
            cluster_dimension,
        }
    }

    pub fn cluster_dimension(&self) -> usize {
        self.cluster_dimension
    }
}

impl<T: RTreeObject> Iterator for SlabIterator<T> {
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

#[cfg(test)]
mod test {
    use super::SlabIterator;

    #[test]
    fn test_create_slabs() {
        const SIZE: usize = 374;
        const NUMBER_OF_CLUSTERS_ON_AXIS: usize = 5;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let slab_size = (elements.len()) / NUMBER_OF_CLUSTERS_ON_AXIS + 1;
        let slabs: Vec<_> = SlabIterator::new(elements, NUMBER_OF_CLUSTERS_ON_AXIS, 0).collect();
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

pub fn calculate_number_of_clusters_on_axis<T, Params>(number_of_elements: usize) -> usize
where
    T: RTreeObject,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    let depth = (number_of_elements as f32).log(m as f32).ceil() as usize;
    let n_subtree = (m as f32).powi(depth as i32 - 1);
    let remaining_clusters = (number_of_elements as f32 / n_subtree).ceil() as usize;

    let max_dimension = <T::Envelope as Envelope>::Point::DIMENSIONS;
    (remaining_clusters as f32)
        .powf(1. / max_dimension as f32)
        .ceil() as usize
}

fn div_up(dividend: usize, divisor: usize) -> usize {
    (dividend + divisor - 1) / divisor
}
