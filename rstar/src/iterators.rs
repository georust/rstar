use params::RTreeParams;
use object::RTreeObject;
use rtree::RTree;
use node::RTreeNode;
use selection_funcs::{SelectAllFunc, SelectAtPointFunc, SelectionFunc, SelectInEnvelopeFunc};

pub type LocateAllAtPoint<'a, T, Params> = SelectionIterator<'a, T, Params, SelectAtPointFunc<T>>;
pub type LocateAllAtPointMut<'a, T, Params> =
    SelectionIteratorMut<'a, T, Params, SelectAtPointFunc<T>>;
pub type LocateInEnvelope<'a, T, Params> =
    SelectionIterator<'a, T, Params, SelectInEnvelopeFunc<T>>;
pub type LocateInEnvelopeMut<'a, T, Params> =
    SelectionIteratorMut<'a, T, Params, SelectInEnvelopeFunc<T>>;
pub type RTreeIterator<'a, T, Params> = SelectionIterator<'a, T, Params, SelectAllFunc>;
pub type RTreeIteratorMut<'a, T, Params> = SelectionIteratorMut<'a, T, Params, SelectAllFunc>;

pub struct SelectionIterator<'a, T, Params, Func>
where
    T: RTreeObject + 'a,
    Params: RTreeParams + 'a,
    Func: SelectionFunc<T>,
{
    func: Func,
    current_nodes: Vec<&'a RTreeNode<T, Params>>,
}
impl<'a, T, Params, Func> SelectionIterator<'a, T, Params, Func>
where
    T: RTreeObject,
    Params: RTreeParams,
    Func: SelectionFunc<T>,
{
    pub fn new(tree: &'a RTree<T, Params>, containment_unit: Func::ContainmentUnit) -> Self {
        let func = Func::new(containment_unit);
        SelectionIterator {
            func: func.clone(),
            current_nodes: tree.root()
                .children
                .iter()
                .filter(|c| func.is_contained_in(&c.envelope()))
                .collect(),
        }
    }
}

impl<'a, T, Params, Func> Iterator for SelectionIterator<'a, T, Params, Func>
where
    T: RTreeObject,
    Params: RTreeParams,
    Func: SelectionFunc<T>,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        while let Some(next) = self.current_nodes.pop() {
            if self.func.is_contained_in(&next.envelope()) {
                match next {
                    &RTreeNode::Leaf(ref t) => return Some(t),
                    &RTreeNode::Parent(ref data) => self.current_nodes.extend(&data.children),
                }
            }
        }
        return None;
    }
}

pub struct SelectionIteratorMut<'a, T, Params, Func>
where
    T: RTreeObject + 'a,
    Params: RTreeParams + 'a,
    Func: SelectionFunc<T>,
{
    func: Func,
    current_nodes: Vec<&'a mut RTreeNode<T, Params>>,
}

impl<'a, T, Params, Func> SelectionIteratorMut<'a, T, Params, Func>
where
    T: RTreeObject,
    Params: RTreeParams,
    Func: SelectionFunc<T>,
{
    pub fn new(tree: &'a mut RTree<T, Params>, containment_unit: Func::ContainmentUnit) -> Self {
        let func = Func::new(containment_unit);
        SelectionIteratorMut {
            func: func.clone(),
            current_nodes: tree.root_mut()
                .children
                .iter_mut()
                .filter(|c| func.is_contained_in(&c.envelope()))
                .collect(),
        }
    }
}

impl<'a, T, Params, Func> Iterator for SelectionIteratorMut<'a, T, Params, Func>
where
    T: RTreeObject,
    Params: RTreeParams,
    Func: SelectionFunc<T>,
{
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        let func = self.func.clone();
        if let Some(next) = self.current_nodes.pop() {
            match next {
                &mut RTreeNode::Leaf(ref mut t) => Some(t),
                &mut RTreeNode::Parent(ref mut data) => {
                    self.current_nodes.extend(
                        data.children
                            .iter_mut()
                            .filter(|c| func.is_contained_in(&c.envelope())),
                    );
                    self.next()
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use testutils::create_random_points;
    use aabb::AABB;
    use rtree::RTree;
    use envelope::Envelope;
    use object::RTreeObject;

    #[derive(PartialEq, Clone)]
    struct TestRectangle {
        aabb: AABB<[f64; 2]>,
    }

    impl RTreeObject for TestRectangle {
        type Envelope = AABB<[f64; 2]>;

        fn envelope(&self) -> Self::Envelope {
            self.aabb
        }
    }

    #[test]
    fn test_locate_all() {
        const NUM_POINTS: usize = 400;
        let points = create_random_points(NUM_POINTS, [231, 22912, 399939, 922931]);
        let mut tree = RTree::new();
        let mut aabb_list = Vec::new();
        for ps in points.as_slice().windows(2) {
            let rectangle = TestRectangle {
                aabb: AABB::from_points(ps),
            };
            tree.insert(rectangle.clone());
            aabb_list.push(rectangle)
        }

        let query_points = create_random_points(10, [59123, 312331, 23235, 123678]);
        for p in &query_points {
            let mut contained_sequential: Vec<_> = aabb_list
                .iter()
                .filter(|aabb| aabb.aabb.contains_point(p))
                .cloned()
                .collect();
            {
                let contained_rtree: Vec<_> = tree.locate_all_at_point(p).collect();
                for rect in &contained_rtree {
                    assert!(&contained_sequential.contains(rect));
                }
                assert_eq!(contained_sequential.len(), contained_rtree.len());
            }

            let contained_rtree_mut: Vec<_> = tree.locate_all_at_point_mut(p).collect();

            assert_eq!(contained_sequential.len(), contained_rtree_mut.len());

            for rect in &contained_rtree_mut {
                assert!(&contained_sequential.contains(rect));
            }
            for rect in &mut contained_sequential {
                assert!(&contained_rtree_mut.contains(&rect));
            }
        }
    }

    #[test]
    fn test_iteration() {
        const NUM_POINTS: usize = 1000;
        let points = create_random_points(NUM_POINTS, [921545, 22305, 2004822, 142567]);
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
}
