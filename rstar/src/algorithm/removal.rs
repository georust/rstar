use crate::envelope::Envelope;
use crate::structures::node::{ParentNodeData, RTreeNode};
use crate::object::{PointDistance, RTreeObject};
use crate::params::RTreeParams;
use crate::algorithm::selection_functions::SelectionFunc;
use crate::Point;

/// Specifies if an element should be removed.
///
/// During removal, the r-tree is traversed until a leaf node is found, using a
/// specific [trait.SelectionFunc]. However, not all leafs found by the selection
/// function are desireable for removal. This trait specifies which elements are
/// to be removed in a removal operation.
pub trait RemovalFunction<T>: SelectionFunc<T>
where
    T: RTreeObject,
{
    /// Returns if a found leaf element should be removed or may remain in the
    /// r-tree. Returning `true` marks the element for removal.
    fn should_be_removed(&self, removal_candidate: &T) -> bool;
}

pub struct RemoveWithDistanceFunction<T>
where
    T: PointDistance,
{
    distance_2: <<T::Envelope as Envelope>::Point as Point>::Scalar,
    point: <T::Envelope as Envelope>::Point,
}

impl<T> Clone for RemoveWithDistanceFunction<T>
where
    T: PointDistance,
{
    fn clone(&self) -> Self {
        RemoveWithDistanceFunction { ..*self }
    }
}

impl<T> RemoveWithDistanceFunction<T>
where
    T: PointDistance,
{
    pub fn new(
        point: <T::Envelope as Envelope>::Point,
        distance_2: <<T::Envelope as Envelope>::Point as Point>::Scalar,
    ) -> Self {
        RemoveWithDistanceFunction { point, distance_2 }
    }
}

impl<T> SelectionFunc<T> for RemoveWithDistanceFunction<T>
where
    T: PointDistance,
{
    type ContainmentUnit = <T::Envelope as Envelope>::Point;

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }
}

impl<T> RemovalFunction<T> for RemoveWithDistanceFunction<T>
where
    T: PointDistance,
{
    fn should_be_removed(&self, removal_candidate: &T) -> bool {
        removal_candidate.distance_2(&self.point) <= self.distance_2
    }
}

/// A [trait.RemovalFunction] that only marks elements for removal whose envelope
/// contains a specific point.
pub struct RemoveAtPointFunction<T>
where
    T: PointDistance,
{
    point: <T::Envelope as Envelope>::Point,
}

impl<T> Clone for RemoveAtPointFunction<T>
where
    T: PointDistance,
{
    fn clone(&self) -> Self {
        RemoveAtPointFunction { ..*self }
    }
}

impl<T> RemoveAtPointFunction<T>
where
    T: PointDistance,
{
    pub fn new(point: <T::Envelope as Envelope>::Point) -> Self {
        RemoveAtPointFunction { point }
    }
}

impl<T> SelectionFunc<T> for RemoveAtPointFunction<T>
where
    T: PointDistance,
{
    type ContainmentUnit = <T::Envelope as Envelope>::Point;

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }
}

impl<T> RemovalFunction<T> for RemoveAtPointFunction<T>
where
    T: PointDistance,
{
    fn should_be_removed(&self, removal_candidate: &T) -> bool {
        removal_candidate.contains_point(&self.point)
    }
}

/// A removal function that only marks elements equal (`==`) to a
/// given element for removal.
pub struct RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq + 'a,
{
    /// Only elements equal to this object will be removed.
    object_to_remove: &'a T,
}

impl<'a, T> Clone for RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    fn clone(&self) -> Self {
        RemoveEqualsFunction { ..*self }
    }
}

impl<'a, T> RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    pub fn new(object_to_remove: &'a T) -> Self {
        RemoveEqualsFunction { object_to_remove }
    }
}

impl<'a, T> SelectionFunc<T> for RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    type ContainmentUnit = &'a T;

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_envelope(&self.object_to_remove.envelope())
    }
}

impl<'a, T> RemovalFunction<T> for RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    fn should_be_removed(&self, removal_candidate: &T) -> bool {
        removal_candidate == self.object_to_remove
    }
}

/// Default removal strategy to remove elements from an r-tree. A [trait.RemovalFunction]
/// specifies which elements shall be removed.
///
/// The algorithm descends the tree to the leaf level, using the given removal function
/// (see [trait.SelectionFunc]). Then, the removal function defines which leaf node shall be
/// removed. Once the first node is found, the process stops and the element is removed and
/// returned.
///
/// If a tree node becomes empty by the removal, it is also removed from its parent node.
pub fn remove<T, Params, R>(node: &mut ParentNodeData<T>, removal_function: &R) -> Option<T>
where
    T: RTreeObject,
    Params: RTreeParams,
    R: RemovalFunction<T>,
{
    let mut result = None;
    if removal_function.is_contained_in(&node.envelope) {
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
                    if removal_function.should_be_removed(b) {
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
        node.envelope = crate::structures::node::envelope_for_children(&node.children);
    }
    result
}

#[cfg(test)]
mod test {
    use crate::point::PointExt;
    use crate::primitives::SimpleEdge;
    use crate::test_utilities::{create_random_points, create_random_rectangles};
    use crate::RTree;

    #[test]
    fn test_remove_and_insert() {
        const SIZE: usize = 1000;
        let mut points = create_random_points(SIZE, *b"r(ConCe)tr4tio/s");
        let later_insertions = create_random_points(SIZE, *b"S3n7iW=ntaL)s|nG");
        let mut tree = RTree::bulk_load(&mut points);
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
        let mut initial_rectangles = create_random_rectangles(SIZE, *b"r(ConCe)tr4tio/s");
        let new_rectangles = create_random_rectangles(SIZE, *b"S3n7iW=ntaL)s|nG");
        let mut tree = RTree::bulk_load(&mut initial_rectangles);

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
        let mut points = create_random_points(1000, *b"0v3rS?sc)l|nI'-d");
        let mut tree = RTree::bulk_load(&mut points);
        for point in &points {
            let size_before_removal = tree.size();
            assert!(tree.remove_at_point(point).is_some());
            assert!(tree.remove_at_point(&[1000.0, 1000.0]).is_none());
            assert_eq!(size_before_removal - 1, tree.size());
        }
    }

    #[test]
    fn test_remove() {
        let points = create_random_points(1000, *b"rem0T3Cont)o|:ng");
        let offsets = create_random_points(1000, *b"h?eMot-7hom3te)5");
        let scaled = offsets.iter().map(|p| p.mul(0.05));
        let mut edges: Vec<_> = points
            .iter()
            .zip(scaled)
            .map(|(from, offset)| SimpleEdge::new(*from, from.add(&offset)))
            .collect();
        let mut tree = RTree::bulk_load(&mut edges);
        for edge in &edges {
            let size_before_removal = tree.size();
            assert!(tree.remove(edge).is_some());
            assert!(tree.remove(edge).is_none());
            assert_eq!(size_before_removal - 1, tree.size());
        }
    }
}
