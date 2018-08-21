use envelope::Envelope;
use node::{ParentNodeData, RTreeNode};
use object::RTreeObject;
use params::RTreeParams;
use point::{EuclideanPoint, Point};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LevelPartitioning {
    dimension_sizes: Vec<usize>,
    num_overflow_elements: usize,
    root_size: usize,
}

impl LevelPartitioning {
    fn new(number_of_elements: usize, max_node_size: usize, dimensions: usize) -> Self {
        // TODO: Generalize to multiple dimensions
        assert_eq!(
            dimensions, 2,
            "Bulk loading currently not supported for {} dimensional objects",
            dimensions
        );
        let root_size = Self::calculate_root_size(number_of_elements, max_node_size);
        let node_size_f = root_size as f64;
        let root = (node_size_f).sqrt();
        let d1 = root.round() as usize;
        let d2 = root_size / d1;
        let num_overflow_elements = root_size % d1;
        let dimension_sizes = vec![d1, d2];

        LevelPartitioning {
            dimension_sizes,
            num_overflow_elements,
            root_size,
        }
    }

    fn calculate_root_size(mut number_of_elements: usize, max_node_size: usize) -> usize {
        while number_of_elements > max_node_size {
            number_of_elements = div_up(number_of_elements, max_node_size);
        }
        number_of_elements
    }

    fn calculate_tree_height(mut number_of_elements: usize, max_node_size: usize) -> usize {
        if number_of_elements == 0 {
            0
        } else {
            let mut height = 1;
            while number_of_elements > max_node_size {
                number_of_elements = div_up(number_of_elements, max_node_size);
                height += 1;
            }
            height
        }
    }
}

pub fn bulk_load_with_params<T, Params>(elements: &mut [T]) -> (ParentNodeData<T, Params>, usize)
where
    T: RTreeObject + Clone,
    <T::Envelope as Envelope>::Point: EuclideanPoint,
    Params: RTreeParams,
{
    let height = LevelPartitioning::calculate_tree_height(elements.len(), Params::MAX_SIZE);
    (bulk_load_recursive::<_, Params>(elements), height)
}

fn bulk_load_recursive<T, Params>(mut elements: &mut [T]) -> ParentNodeData<T, Params>
where
    T: RTreeObject + Clone,
    <T::Envelope as Envelope>::Point: EuclideanPoint,
    Params: RTreeParams,
{
    let max_node_size = Params::MAX_SIZE;
    if elements.len() <= max_node_size {
        let children = elements.iter().cloned().map(RTreeNode::Leaf).collect();
        return ParentNodeData::new_parent(children);
    }

    let dimensions = <T::Envelope as Envelope>::Point::DIMENSIONS;
    let partition_information = LevelPartitioning::new(elements.len(), max_node_size, dimensions);

    // TODO: Generalize this to more than two dimensions
    let d0 = partition_information.dimension_sizes[0];
    let d1 = partition_information.dimension_sizes[1];
    let mut remaining = partition_information.num_overflow_elements;
    let number_of_cells = d0 * d1 + remaining;
    let cell_size = elements.len() / number_of_cells;
    let mut cell_overflow = elements.len() % number_of_cells;

    let mut children = Vec::new();
    for _ in 0..d0 {
        let mut number_of_cells_for_segment = d1;
        if remaining > 0 {
            remaining -= 1;
            number_of_cells_for_segment += 1;
        }
        let mut additional_elements = cell_overflow.min(number_of_cells_for_segment);
        cell_overflow -= additional_elements;

        let partition_size = cell_size * number_of_cells_for_segment + additional_elements;

        let mut temp = ::std::mem::replace(&mut elements, &mut []);

        let (mut current_cluster, remaining_elements) = create_cluster(temp, partition_size, 0);
        elements = remaining_elements;

        for _ in 0..number_of_cells_for_segment {
            let mut inner_cell_size = cell_size;
            if additional_elements > 0 {
                additional_elements -= 1;
                inner_cell_size += 1;
            }
            let temp2 = ::std::mem::replace(&mut current_cluster, &mut []);
            let (cell, remaining_segment) = create_cluster(temp2, inner_cell_size, 1);
            current_cluster = remaining_segment;
            children.push(RTreeNode::Parent(bulk_load_recursive(cell)));
        }
    }
    ParentNodeData::new_parent(children)
}

#[inline]
fn create_cluster<T: RTreeObject + Clone>(
    mut array: &mut [T],
    cluster_size: usize,
    dimension: usize,
) -> (&mut [T], &mut [T]) {
    let comp = |l: &T, r: &T| {
        let l_mbr = l.envelope();
        let r_mbr = r.envelope();
        l_mbr
            .center()
            .nth(dimension)
            .partial_cmp(&r_mbr.center().nth(dimension))
            .unwrap()
    };

    ::pdqselect::select_by(&mut array, cluster_size, &comp);
    array.split_at_mut(cluster_size)
}

fn div_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

#[cfg(test)]
mod test {
    use super::{div_up, LevelPartitioning};
    use rtree::RTree;
    use std::collections::HashSet;
    use testutils::create_random_integers;

    #[test]
    fn test_bulk_load() {
        let mut random_points = create_random_integers(1000, *b"dEac1difIc4tl0ns");
        let tree = RTree::bulk_load(&mut random_points);
        let set1: HashSet<_> = tree.iter().collect();
        let set2: HashSet<_> = random_points.iter().collect();
        assert_eq!(set1, set2);
        assert_eq!(tree.size(), random_points.len());
    }

    #[test]
    fn test_bulk_load_with_different_sizes() {
        for i in 0..100 {
            let mut random_points = create_random_integers(i * 7, *b"h=Xak|s0CtAh3dr4");
            let tree = RTree::bulk_load(&mut random_points);
            println!("i: {}", i);
            assert_eq!(tree.root().sanity_check(), Some(tree.height()));
        }
    }

    #[test]
    fn test_level_partitioning() {
        for m in 3..100 {
            let partitioning = LevelPartitioning::new(m, m, 2);
            assert_eq!(partitioning.dimension_sizes.len(), 2);
            let d1 = partitioning.dimension_sizes[0];
            let d2 = partitioning.dimension_sizes[1];
            assert!((d1 as i32 - d2 as i32).abs() <= 1);
            assert!(partitioning.num_overflow_elements < m);
            assert_eq!(d1 * d2 + partitioning.num_overflow_elements, m);
        }
    }

    #[test]
    fn test_calculate_root_size() {
        let m = 6;
        let sqr = 36usize;
        let cube = sqr * m;
        for num_elements in 0..=m {
            assert_eq!(
                LevelPartitioning::calculate_root_size(num_elements, m),
                num_elements
            );
        }
        for num_elements in m + 1..=sqr {
            assert_eq!(
                LevelPartitioning::calculate_root_size(num_elements, m),
                div_up(num_elements, m)
            );
        }
        for num_elements in sqr + 1..=cube {
            assert_eq!(
                LevelPartitioning::calculate_root_size(num_elements, m),
                div_up(num_elements, sqr)
            );
        }
    }

    #[test]
    fn test_calculate_tree_height() {
        let m = 6;
        let sqr = 36usize;
        let cube = sqr * m;
        assert_eq!(LevelPartitioning::calculate_tree_height(0, m), 0);
        for num_elements in 1..=m {
            assert_eq!(LevelPartitioning::calculate_tree_height(num_elements, m), 1);
        }
        for num_elements in m + 1..=sqr {
            assert_eq!(LevelPartitioning::calculate_tree_height(num_elements, m), 2);
        }
        for num_elements in sqr + 1..=cube {
            assert_eq!(LevelPartitioning::calculate_tree_height(num_elements, m), 3);
        }
    }
}
