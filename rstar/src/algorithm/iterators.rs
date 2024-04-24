use crate::algorithm::selection_functions::*;
use crate::node::{ParentNode, RTreeNode};
use crate::object::RTreeObject;
use core::ops::ControlFlow;

#[cfg(doc)]
use crate::RTree;

use smallvec::SmallVec;

pub use super::intersection_iterator::IntersectionIterator;
pub use super::removal::{DrainIterator, IntoIter};

/// Iterator returned by [`RTree::locate_all_at_point`].
pub type LocateAllAtPoint<'a, T> = SelectionIterator<'a, T, SelectAtPointFunction<T>>;
/// Iterator returned by [`RTree::locate_all_at_point_mut`].
pub type LocateAllAtPointMut<'a, T> = SelectionIteratorMut<'a, T, SelectAtPointFunction<T>>;

/// Iterator returned by [`RTree::locate_in_envelope`].
pub type LocateInEnvelope<'a, T> = SelectionIterator<'a, T, SelectInEnvelopeFunction<T>>;
/// Iterator returned by [`RTree::locate_in_envelope_mut`].
pub type LocateInEnvelopeMut<'a, T> = SelectionIteratorMut<'a, T, SelectInEnvelopeFunction<T>>;

/// Iterator returned by [`RTree::locate_in_envelope_intersecting`].
pub type LocateInEnvelopeIntersecting<'a, T> =
    SelectionIterator<'a, T, SelectInEnvelopeFuncIntersecting<T>>;
/// Iterator returned by [`RTree::locate_in_envelope_intersecting_mut`].
pub type LocateInEnvelopeIntersectingMut<'a, T> =
    SelectionIteratorMut<'a, T, SelectInEnvelopeFuncIntersecting<T>>;

/// Iterator returned by [`RTree::iter`].
pub type RTreeIterator<'a, T> = SelectionIterator<'a, T, SelectAllFunc>;
/// Iterator returned by [`RTree::iter_mut`].
pub type RTreeIteratorMut<'a, T> = SelectionIteratorMut<'a, T, SelectAllFunc>;

/// Iterator returned by [`RTree::locate_within_distance`].
pub type LocateWithinDistanceIterator<'a, T> =
    SelectionIterator<'a, T, SelectWithinDistanceFunction<T>>;

/// Iterator returned by `RTree::locate_*` methods.
pub struct SelectionIterator<'a, T, Func>
where
    T: RTreeObject + 'a,
    Func: SelectionFunction<T>,
{
    func: Func,
    current_nodes: SmallVec<[&'a RTreeNode<T>; 24]>,
}

impl<'a, T, Func> SelectionIterator<'a, T, Func>
where
    T: RTreeObject,
    Func: SelectionFunction<T>,
{
    pub(crate) fn new(root: &'a ParentNode<T>, func: Func) -> Self {
        let current_nodes = if func.should_unpack_parent(&root.envelope()) {
            root.children.iter().collect()
        } else {
            SmallVec::new()
        };

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

/// Internal iteration variant of [`SelectionIterator`]
pub fn select_nodes<'a, T, Func, V, B>(
    root: &'a ParentNode<T>,
    func: &Func,
    visitor: &mut V,
) -> ControlFlow<B>
where
    T: RTreeObject,
    Func: SelectionFunction<T>,
    V: FnMut(&'a T) -> ControlFlow<B>,
{
    fn inner<'a, T, Func, V, B>(
        parent: &'a ParentNode<T>,
        func: &Func,
        visitor: &mut V,
    ) -> ControlFlow<B>
    where
        T: RTreeObject,
        Func: SelectionFunction<T>,
        V: FnMut(&'a T) -> ControlFlow<B>,
    {
        for node in parent.children.iter() {
            match node {
                RTreeNode::Leaf(ref t) => {
                    if func.should_unpack_leaf(t) {
                        visitor(t)?;
                    }
                }
                RTreeNode::Parent(ref data) => {
                    if func.should_unpack_parent(&data.envelope()) {
                        inner(data, func, visitor)?;
                    }
                }
            }
        }

        ControlFlow::Continue(())
    }

    if func.should_unpack_parent(&root.envelope()) {
        inner(root, func, visitor)?;
    }

    ControlFlow::Continue(())
}

/// Iterator type returned by `RTree::locate_*_mut` methods.
pub struct SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject + 'a,
    Func: SelectionFunction<T>,
{
    func: Func,
    current_nodes: SmallVec<[&'a mut RTreeNode<T>; 32]>,
}

impl<'a, T, Func> SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject,
    Func: SelectionFunction<T>,
{
    pub(crate) fn new(root: &'a mut ParentNode<T>, func: Func) -> Self {
        let current_nodes = if func.should_unpack_parent(&root.envelope()) {
            root.children.iter_mut().collect()
        } else {
            SmallVec::new()
        };

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

/// Internal iteration variant of [`SelectionIteratorMut`]
pub fn select_nodes_mut<'a, T, Func, V, B>(
    root: &'a mut ParentNode<T>,
    func: &Func,
    visitor: &mut V,
) -> ControlFlow<B>
where
    T: RTreeObject,
    Func: SelectionFunction<T>,
    V: FnMut(&'a mut T) -> ControlFlow<B>,
{
    fn inner<'a, T, Func, V, B>(
        parent: &'a mut ParentNode<T>,
        func: &Func,
        visitor: &mut V,
    ) -> ControlFlow<B>
    where
        T: RTreeObject,
        Func: SelectionFunction<T>,
        V: FnMut(&'a mut T) -> ControlFlow<B>,
    {
        for node in parent.children.iter_mut() {
            match node {
                RTreeNode::Leaf(ref mut t) => {
                    if func.should_unpack_leaf(t) {
                        visitor(t)?;
                    }
                }
                RTreeNode::Parent(ref mut data) => {
                    if func.should_unpack_parent(&data.envelope()) {
                        inner(data, func, visitor)?;
                    }
                }
            }
        }

        ControlFlow::Continue(())
    }

    if func.should_unpack_parent(&root.envelope()) {
        inner(root, func, visitor)?;
    }

    ControlFlow::Continue(())
}

#[cfg(test)]
mod test {
    use crate::aabb::AABB;
    use crate::envelope::Envelope;
    use crate::object::RTreeObject;
    use crate::rtree::RTree;
    use crate::test_utilities::{create_random_points, create_random_rectangles, SEED_1};
    use crate::SelectionFunction;

    #[test]
    fn test_root_node_is_not_always_unpacked() {
        struct SelectNoneFunc {}

        impl SelectionFunction<[i32; 2]> for SelectNoneFunc {
            fn should_unpack_parent(&self, _: &AABB<[i32; 2]>) -> bool {
                false
            }
        }

        let mut tree = RTree::bulk_load(vec![[0i32, 0]]);

        let mut elements = tree.locate_with_selection_function(SelectNoneFunc {});
        assert!(elements.next().is_none());
        drop(elements);

        let mut elements = tree.locate_with_selection_function_mut(SelectNoneFunc {});
        assert!(elements.next().is_none());
    }

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

    #[test]
    fn test_locate_within_distance_on_empty_tree() {
        let tree: RTree<[i64; 3]> = RTree::new();

        tree.locate_within_distance([0, 0, 0], 10);
    }
}
