use crate::algorithm::selection_functions::SelectionFunction;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use crate::params::RTreeParams;

/// Default removal strategy to remove elements from an r-tree. A [trait.RemovalFunction]
/// specifies which elements shall be removed.
///
/// The algorithm descends the tree to the leaf level, using the given removal function
/// (see [trait.SelectionFunc]). Then, the removal function defines which leaf node shall be
/// removed. Once the first node is found, the process stops and the element is removed and
/// returned.
///
/// If a tree node becomes empty by the removal, it is also removed from its parent node.
pub fn remove<T, Params, R>(node: &mut ParentNode<T>, removal_function: &R) -> Option<T>
where
    T: RTreeObject,
    Params: RTreeParams,
    R: SelectionFunction<T>,
{
    let mut result = None;
    if removal_function.should_unpack_parent(&node.envelope) {
        let mut removal_index = None;
        for (index, child) in node.children.iter_mut().enumerate() {
            match child {
                RTreeNode::Parent(ref mut data) => {
                    result = remove::<_, Params, _>(data, removal_function);
                    if result.is_some() {
                        if data.children.is_empty() {
                            // Mark child for removal if it has become empty
                            removal_index = Some(index);
                        }
                        break;
                    }
                }
                RTreeNode::Leaf(ref b) => {
                    if removal_function.should_unpack_leaf(b) {
                        // Mark leaf for removal if should be removed
                        removal_index = Some(index);
                        break;
                    }
                }
            }
        }
        // Perform the actual removal outside of the self.children borrow
        if let Some(removal_index) = removal_index {
            let child = node.children.swap_remove(removal_index);
            if result.is_none() {
                if let RTreeNode::Leaf(t) = child {
                    result = Some(t);
                } else {
                    unreachable!("This is a bug.");
                }
            }
        }
    }
    if result.is_some() {
        // Update the envelope, it may have become smaller
        node.envelope = crate::node::envelope_for_children(&node.children);
    }
    result
}

#[cfg(test)]
mod test {
    use crate::point::PointExt;
    use crate::primitives::Line;
    use crate::test_utilities::{create_random_points, create_random_rectangles, SEED_1, SEED_2};
    use crate::RTree;

    #[test]
    fn test_remove_and_insert() {
        const SIZE: usize = 1000;
        let points = create_random_points(SIZE, SEED_1);
        let later_insertions = create_random_points(SIZE, SEED_2);
        let mut tree = RTree::bulk_load(points.clone());
        for (point_to_remove, point_to_add) in points.iter().zip(later_insertions.iter()) {
            assert!(tree.remove_at_point(point_to_remove).is_some());
            tree.insert(*point_to_add);
        }
        assert_eq!(tree.size(), SIZE);
        assert!(points.iter().all(|p| !tree.contains(p)));
        assert!(later_insertions.iter().all(|p| tree.contains(p)));
        for point in &later_insertions {
            assert!(tree.remove_at_point(point).is_some());
        }
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn test_remove_and_insert_rectangles() {
        const SIZE: usize = 1000;
        let initial_rectangles = create_random_rectangles(SIZE, SEED_1);
        let new_rectangles = create_random_rectangles(SIZE, SEED_2);
        let mut tree = RTree::bulk_load(initial_rectangles.clone());

        for (rectangle_to_remove, rectangle_to_add) in
            initial_rectangles.iter().zip(new_rectangles.iter())
        {
            assert!(tree.remove(rectangle_to_remove).is_some());
            tree.insert(*rectangle_to_add);
        }
        assert_eq!(tree.size(), SIZE);
        assert!(initial_rectangles.iter().all(|p| !tree.contains(p)));
        assert!(new_rectangles.iter().all(|p| tree.contains(p)));
        for rectangle in &new_rectangles {
            assert!(tree.contains(rectangle));
        }
        for rectangle in &initial_rectangles {
            assert!(!tree.contains(rectangle));
        }
        for rectangle in &new_rectangles {
            assert!(tree.remove(rectangle).is_some());
        }
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn test_remove_at_point() {
        let points = create_random_points(1000, SEED_1);
        let mut tree = RTree::bulk_load(points.clone());
        for point in &points {
            let size_before_removal = tree.size();
            assert!(tree.remove_at_point(point).is_some());
            assert!(tree.remove_at_point(&[1000.0, 1000.0]).is_none());
            assert_eq!(size_before_removal - 1, tree.size());
        }
    }

    #[test]
    fn test_remove() {
        let points = create_random_points(1000, SEED_1);
        let offsets = create_random_points(1000, SEED_2);
        let scaled = offsets.iter().map(|p| p.mul(0.05));
        let edges: Vec<_> = points
            .iter()
            .zip(scaled)
            .map(|(from, offset)| Line::new(*from, from.add(&offset)))
            .collect();
        let mut tree = RTree::bulk_load(edges.clone());
        for edge in &edges {
            let size_before_removal = tree.size();
            assert!(tree.remove(edge).is_some());
            assert!(tree.remove(edge).is_none());
            assert_eq!(size_before_removal - 1, tree.size());
        }
    }
}
