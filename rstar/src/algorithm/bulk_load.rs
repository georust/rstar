use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;
use crate::point::Point;
use crate::structures::node::{ParentNodeData, RTreeNode};

pub fn bulk_load<T, Params>(mut elements: &mut [T]) -> ParentNodeData<T>
where
    T: RTreeObject + Clone,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    let m = Params::MAX_SIZE;
    if elements.len() <= m {
        // Reached leaf level
        let elements: Vec<_> = elements
            .iter()
            .map(|e| RTreeNode::Leaf(e.clone()))
            .collect();
        return ParentNodeData::new_parent(elements);
    }

    let depth = (elements.len() as f32).log(m as f32).ceil() as usize;
    let n_subtree = (m as f32).powi(depth as i32 - 1);
    let remaining_clusters = (elements.len() as f32 / n_subtree).ceil() as usize;

    let num_vertical_slices = (remaining_clusters as f32).sqrt().ceil() as usize;
    let vertical_slice_num_elements =
        (elements.len() + num_vertical_slices - 1) / num_vertical_slices;
    let mut children = Vec::with_capacity(m + 1);
    create_clusters_imbalanced(&mut elements, vertical_slice_num_elements, 0);

    let num_clusters_per_slice =
        (remaining_clusters + num_vertical_slices - 1) / num_vertical_slices;
    for mut slice in elements.chunks_mut(vertical_slice_num_elements) {
        let cluster_num_elements =
            (slice.len() + num_clusters_per_slice - 1) / num_clusters_per_slice;

        create_clusters_imbalanced(&mut slice, cluster_num_elements, 1);

        for cluster in slice.chunks_mut(cluster_num_elements) {
            let child = bulk_load::<_, Params>(cluster);
            children.push(RTreeNode::Parent(child));
        }
    }
    ParentNodeData::new_parent(children)
}

fn create_clusters_imbalanced<T: RTreeObject>(
    array: &mut [T],
    cluster_size: usize,
    dimension: usize,
) {
    let comp = |l: &T, r: &T| {
        let l_mbr = l.envelope();
        let r_mbr = r.envelope();
        l_mbr
            .center()
            .nth(dimension)
            .partial_cmp(&r_mbr.center().nth(dimension))
            .unwrap()
    };

    let mut cur = 0;
    while cur <= array.len() {
        ::pdqselect::select_by(&mut array[cur..], cluster_size, &comp);
        cur += cluster_size;
    }
}

pub fn bulk_load_with_params<T, Params>(elements: &mut [T]) -> ParentNodeData<T>
where
    T: RTreeObject + Clone,
    <T::Envelope as Envelope>::Point: Point,
    Params: RTreeParams,
{
    bulk_load::<_, Params>(elements)
}

#[cfg(test)]
mod test {
    use crate::rtree::RTree;
    use crate::test_utilities::{create_random_integers, SEED_1};
    use std::collections::HashSet;

    #[test]
    fn test_bulk_load() {
        let mut random_points = create_random_integers(1000, SEED_1);
        let tree = RTree::bulk_load(&mut random_points);
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = random_points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), random_points.len());
    }

    #[test]
    fn test_bulk_load_with_different_sizes() {
        for i in 0..100 {
            let mut random_points = create_random_integers(i * 7, SEED_1);
            RTree::bulk_load(&mut random_points);
        }
    }
}
