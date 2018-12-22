use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};

pub fn bulk_load<T, Params>(elements: Vec<T>) -> ParentNodeData<T>
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

    let depth = (elements.len() as f32).log(m as f32).ceil() as usize;
    let n_subtree = (m as f32).powi(depth as i32 - 1);
    let remaining_clusters = (elements.len() as f32 / n_subtree).ceil() as usize;
    let num_vertical_slices = (remaining_clusters as f32).sqrt().ceil() as usize;
    let vertical_slice_num_elements =
        (elements.len() + num_vertical_slices - 1) / num_vertical_slices;
    let mut children = Vec::with_capacity(m + 1);
    for slice in create_clusters(elements, vertical_slice_num_elements, 0) {
        let num_clusters_per_slice =
            (remaining_clusters + num_vertical_slices - 1) / num_vertical_slices;
        let cluster_num_elements =
            (slice.len() + num_clusters_per_slice - 1) / num_clusters_per_slice;

        for cluster in create_clusters(slice, cluster_num_elements, 1) {
            let child = bulk_load::<_, Params>(cluster);
            children.push(RTreeNode::Parent(child));
        }
    }
    ParentNodeData::new_parent(children)
}

struct ClusterIterator<T: RTreeObject> {
    remaining: Vec<T>,
    cluster_size: usize,
    cluster_dimension: usize,
}

fn create_clusters<T>(
    elements: Vec<T>,
    cluster_size: usize,
    cluster_dimension: usize,
) -> impl Iterator<Item = Vec<T>>
where
    T: RTreeObject,
{
    ClusterIterator {
        remaining: elements,
        cluster_size,
        cluster_dimension,
    }
}

impl<T> Iterator for ClusterIterator<T>
where
    T: RTreeObject,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.remaining.len() {
            0 => None,
            len if len <= self.cluster_size => {
                ::std::mem::replace(&mut self.remaining, vec![]).into()
            }
            _ => {
                let cluster_dimension = self.cluster_dimension;
                let comp = |l: &T, r: &T| {
                    let l_mbr = l.envelope();
                    let r_mbr = r.envelope();
                    l_mbr
                        .center()
                        .nth(cluster_dimension)
                        .partial_cmp(&r_mbr.center().nth(cluster_dimension))
                        .unwrap()
                };
                ::pdqselect::select_by(&mut self.remaining, self.cluster_size, &comp);

                let off_split = self.remaining.split_off(self.cluster_size);
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
    bulk_load::<_, Params>(elements)
}

#[cfg(test)]
mod test {
    use super::create_clusters;
    use crate::rtree::RTree;
    use crate::test_utilities::{create_random_integers, SEED_1};
    use std::collections::HashSet;

    #[test]
    fn test_create_clusters() {
        const SIZE: usize = 374;
        const CLUSTER_SIZE: usize = 10;
        let elements: Vec<_> = (0..SIZE as i32).map(|i| [-i, -i]).collect();
        let clusters: Vec<_> = create_clusters(elements, CLUSTER_SIZE, 0).collect();
        assert_eq!(clusters.len(), (SIZE + CLUSTER_SIZE) / CLUSTER_SIZE);
        for cluster in &clusters[0..clusters.len() - 1] {
            assert_eq!(cluster.len(), CLUSTER_SIZE);
        }
        let mut total_size = 0;
        let mut max_element_for_last_cluster = i32::min_value();
        for cluster in &clusters {
            total_size += cluster.len();
            let current_max = cluster.iter().max_by_key(|point| point[0]).unwrap();
            assert!(current_max[0] > max_element_for_last_cluster);
            max_element_for_last_cluster = current_max[0];
        }
        assert_eq!(total_size, SIZE);
    }

    #[test]
    fn test_bulk_load_small() {
        let random_points = create_random_integers(50, SEED_1);
        let tree = RTree::bulk_load(random_points.clone());
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = random_points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), random_points.len());
    }

    #[test]
    fn test_bulk_load() {
        let random_points = create_random_integers(1000, SEED_1);
        let tree = RTree::bulk_load(random_points.clone());
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = random_points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), random_points.len());
    }

    #[test]
    fn test_bulk_load_with_different_sizes() {
        for i in 0..100 {
            let random_points = create_random_integers(i * 7, SEED_1);
            RTree::bulk_load(random_points);
        }
    }
}
