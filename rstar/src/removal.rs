use envelope::Envelope;
use node::{ParentNodeData, RTreeNode};
use object::RTreeObject;
use params::RTreeParams;
use selection_functions::SelectionFunc;

pub trait RemovalFunction<T>: SelectionFunc<T>
where
    T: RTreeObject,
{
    fn should_be_removed(&self, removal_candidate: &T) -> bool;
}

pub struct RemoveAtPointFunction<T>
where
    T: RTreeObject,
{
    point: <T::Envelope as Envelope>::Point,
}

impl<T> Clone for RemoveAtPointFunction<T>
where
    T: RTreeObject,
{
    fn clone(&self) -> Self {
        RemoveAtPointFunction { ..*self }
    }
}

impl<T> SelectionFunc<T> for RemoveAtPointFunction<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = <T::Envelope as Envelope>::Point;

    fn new(containment_unit: Self::ContainmentUnit) -> Self {
        RemoveAtPointFunction {
            point: containment_unit,
        }
    }

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }
}

impl<T> RemovalFunction<T> for RemoveAtPointFunction<T>
where
    T: RTreeObject,
{
    fn should_be_removed(&self, removal_candidate: &T) -> bool {
        removal_candidate.envelope().contains_point(&self.point)
    }
}

pub struct RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq + 'a,
{
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

impl<'a, T> SelectionFunc<T> for RemoveEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    type ContainmentUnit = &'a T;

    fn new(containment_unit: Self::ContainmentUnit) -> Self {
        RemoveEqualsFunction {
            object_to_remove: containment_unit,
        }
    }

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
                            removal_index = Some(index);
                        }
                        break;
                    }
                }
                RTreeNode::Leaf(ref b) => {
                    if removal_function.should_be_removed(b) {
                        removal_index = Some(index);
                        break;
                    }
                }
            }
        }
        if let Some(removal_index) = removal_index {
            let child = node.children.swap_remove(removal_index);
            if result.is_none() {
                if let RTreeNode::Leaf(t) = child {
                    result = Some(t);
                } else {
                    // This should not be possible
                    panic!("This is a bug");
                }
            }
        }
    }
    if result.is_some() {
        node.envelope = ::node::envelope_for_children(&node.children);
    }
    result
}

#[cfg(test)]
mod test {
    use point::PointExt;
    use primitives::SimpleEdge;
    use rtree::RTree;
    use test_utilities::create_random_points;

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
