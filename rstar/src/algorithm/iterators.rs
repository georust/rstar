use crate::algorithm::selection_functions::*;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;

pub type LocateAllAtPoint<'a, T> = SelectionIterator<'a, T, SelectAtPointFunction<T>>;
pub type LocateAllAtPointMut<'a, T> = SelectionIteratorMut<'a, T, SelectAtPointFunction<T>>;
pub type LocateInEnvelope<'a, T> = SelectionIterator<'a, T, SelectInEnvelopeFunction<T>>;
pub type LocateInEnvelopeMut<'a, T> = SelectionIteratorMut<'a, T, SelectInEnvelopeFunction<T>>;
pub type LocateInEnvelopeIntersecting<'a, T> =
    SelectionIterator<'a, T, SelectInEnvelopeFuncIntersecting<T>>;
pub type LocateInEnvelopeIntersectingMut<'a, T> =
    SelectionIteratorMut<'a, T, SelectInEnvelopeFuncIntersecting<T>>;
pub type RTreeIterator<'a, T> = SelectionIterator<'a, T, SelectAllFunc>;
pub type RTreeIteratorMut<'a, T> = SelectionIteratorMut<'a, T, SelectAllFunc>;
pub type LocateWithinDistanceIterator<'a, T> =
    SelectionIterator<'a, T, SelectWithinDistanceFunction<T>>;

pub struct SelectionIterator<'a, T, Func>
where
    T: RTreeObject + 'a,
    Func: SelectionFunction<T>,
{
    func: Func,
    current_nodes: Vec<&'a RTreeNode<T>>,
}

impl<'a, T, Func> SelectionIterator<'a, T, Func>
where
    T: RTreeObject,
    Func: SelectionFunction<T>,
{
    pub fn new(root: &'a ParentNode<T>, func: Func) -> Self {
        let should_unpack_node = |node: &&RTreeNode<T>| match node {
            RTreeNode::Leaf(ref t) => func.should_unpack_leaf(t),
            RTreeNode::Parent(ref data) => func.should_unpack_parent(&data.envelope),
        };
        let current_nodes: Vec<_> = root.children.iter().filter(should_unpack_node).collect();
        SelectionIterator {
            func,
            current_nodes,
        }
    }
}

impl<'a, T, Func> Iterator for SelectionIterator<'a, T, Func>
where
    T: RTreeObject,

    Func: SelectionFunction<T>,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        while let Some(next) = self.current_nodes.pop() {
            match next {
                RTreeNode::Leaf(ref t) => {
                    if self.func.should_unpack_leaf(t) {
                        return Some(t);
                    }
                }
                RTreeNode::Parent(ref data) => {
                    if self.func.should_unpack_parent(&data.envelope) {
                        self.current_nodes.extend(&data.children);
                    }
                }
            }
        }
        None
    }
}

pub struct SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject + 'a,
    Func: SelectionFunction<T>,
{
    func: Func,
    current_nodes: Vec<&'a mut RTreeNode<T>>,
}

impl<'a, T, Func> SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject,
    Func: SelectionFunction<T>,
{
    pub fn new(root: &'a mut ParentNode<T>, func: Func) -> Self {
        let should_unpack_node = |node: &&mut RTreeNode<T>| match node {
            RTreeNode::Leaf(ref t) => func.should_unpack_leaf(t),
            RTreeNode::Parent(ref data) => func.should_unpack_parent(&data.envelope),
        };
        let current_nodes = root.children.iter_mut().filter(should_unpack_node).collect();
        SelectionIteratorMut {
            func,
            current_nodes,
        }
    }
}

impl<'a, T, Func> Iterator for SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject,

    Func: SelectionFunction<T>,
{
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        while let Some(next) = self.current_nodes.pop() {
            match next {
                RTreeNode::Leaf(ref mut t) => {
                    if self.func.should_unpack_leaf(t) {
                        return Some(t);
                    }
                }
                RTreeNode::Parent(ref mut data) => {
                    if self.func.should_unpack_parent(&data.envelope) {
                        self.current_nodes.extend(&mut data.children);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use crate::aabb::AABB;
    use crate::envelope::Envelope;
    use crate::object::RTreeObject;
    use crate::rtree::RTree;
    use crate::test_utilities::{create_random_points, create_random_rectangles, SEED_1};

    #[test]
    fn test_locate_all() {
        const NUM_RECTANGLES: usize = 400;
        let rectangles = create_random_rectangles(NUM_RECTANGLES, SEED_1);
        let tree = RTree::bulk_load(rectangles.clone());

        let query_points = create_random_points(20, SEED_1);

        for p in &query_points {
            let contained_sequential: Vec<_> = rectangles
                .iter()
                .filter(|rectangle| rectangle.envelope().contains_point(p))
                .cloned()
                .collect();

            let contained_rtree: Vec<_> = tree.locate_all_at_point(p).cloned().collect();

            contained_sequential
                .iter()
                .all(|r| contained_rtree.contains(r));
            contained_rtree
                .iter()
                .all(|r| contained_sequential.contains(r));
        }
    }

    #[test]
    fn test_locate_in_envelope() {
        let points = create_random_points(100, SEED_1);
        let tree = RTree::bulk_load(points.clone());
        let envelope = AABB::from_corners([0.5, 0.5], [1.0, 1.0]);
        let contained_in_envelope: Vec<_> = points
            .iter()
            .filter(|point| envelope.contains_point(point))
            .cloned()
            .collect();
        let len = contained_in_envelope.len();
        assert!(10 < len && len < 90, "unexpected point distribution");
        let located: Vec<_> = tree.locate_in_envelope(&envelope).cloned().collect();
        assert_eq!(len, located.len());
        for point in &contained_in_envelope {
            assert!(located.contains(point));
        }
    }

    #[test]
    fn test_locate_with_selection_func() {
        use crate::SelectionFunction;

        struct SelectLeftOfZeroPointFiveFunc;

        impl SelectionFunction<[f64; 2]> for SelectLeftOfZeroPointFiveFunc {
            fn should_unpack_parent(&self, parent_envelope: &AABB<[f64; 2]>) -> bool {
                parent_envelope.lower()[0] < 0.5 || parent_envelope.upper()[0] < 0.5
            }

            fn should_unpack_leaf(&self, child: &[f64; 2]) -> bool {
                child[0] < 0.5
            }
        }

        let func = SelectLeftOfZeroPointFiveFunc;

        let points = create_random_points(100, SEED_1);
        let tree = RTree::bulk_load(points.clone());
        let iterative_count = points
            .iter()
            .filter(|leaf| func.should_unpack_leaf(leaf))
            .count();
        let selected = tree
            .locate_with_selection_function(func)
            .collect::<Vec<_>>();

        assert_eq!(iterative_count, selected.len());
        assert!(iterative_count > 20); // Make sure that we do test something interesting
        for point in &selected {
            assert!(point[0] < 0.5);
        }
    }

    #[test]
    fn test_iteration() {
        const NUM_POINTS: usize = 1000;
        let points = create_random_points(NUM_POINTS, SEED_1);
        let mut tree = RTree::new();
        for p in &points {
            tree.insert(*p);
        }
        let mut count = 0usize;
        for p in tree.iter() {
            assert!(points.iter().any(|q| q == p));
            count += 1;
        }
        assert_eq!(count, NUM_POINTS);
        count = 0;
        for p in tree.iter_mut() {
            assert!(points.iter().any(|q| q == p));
            count += 1;
        }
        assert_eq!(count, NUM_POINTS);
        for p in &points {
            assert!(tree.iter().any(|q| q == p));
            assert!(tree.iter_mut().any(|q| q == p));
        }
    }

    #[test]
    fn test_locate_within_distance() {
        use crate::primitives::Line;

        let points = create_random_points(100, SEED_1);
        let tree = RTree::bulk_load(points.clone());
        let circle_radius_2 = 0.3;
        let circle_origin = [0.2, 0.6];
        let contained_in_circle: Vec<_> = points
            .iter()
            .filter(|point| Line::new(circle_origin, **point).length_2() <= circle_radius_2)
            .cloned()
            .collect();
        let located: Vec<_> = tree
            .locate_within_distance(circle_origin, circle_radius_2)
            .cloned()
            .collect();

        assert_eq!(located.len(), contained_in_circle.len());
        for point in &contained_in_circle {
            assert!(located.contains(point));
        }
    }
}
