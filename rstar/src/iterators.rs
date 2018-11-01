use node::RTreeNode;
use object::RTreeObject;
use params::RTreeParams;
use rtree::RTree;
use selection_functions::{SelectAllFunc, SelectAtPointFunc, SelectInEnvelopeFunc, SelectionFunc};

pub type LocateAllAtPoint<'a, T> = SelectionIterator<'a, T, SelectAtPointFunc<T>>;
pub type LocateAllAtPointMut<'a, T> = SelectionIteratorMut<'a, T, SelectAtPointFunc<T>>;
pub type LocateInEnvelope<'a, T> = SelectionIterator<'a, T, SelectInEnvelopeFunc<T>>;
pub type LocateInEnvelopeMut<'a, T> = SelectionIteratorMut<'a, T, SelectInEnvelopeFunc<T>>;
pub type RTreeIterator<'a, T> = SelectionIterator<'a, T, SelectAllFunc>;
pub type RTreeIteratorMut<'a, T> = SelectionIteratorMut<'a, T, SelectAllFunc>;

pub struct SelectionIterator<'a, T, Func>
where
    T: RTreeObject + 'a,
    Func: SelectionFunc<T>,
{
    func: Func,
    current_nodes: Vec<&'a RTreeNode<T>>,
}
impl<'a, T, Func> SelectionIterator<'a, T, Func>
where
    T: RTreeObject,
    Func: SelectionFunc<T>,
{
    pub fn new<Params>(tree: &'a RTree<T, Params>, containment_unit: Func::ContainmentUnit) -> Self
    where
        Params: RTreeParams,
    {
        let func = Func::new(containment_unit);
        SelectionIterator {
            func: func.clone(),
            current_nodes: tree
                .root()
                .children
                .iter()
                .filter(|c| func.is_contained_in(&c.envelope()))
                .collect(),
        }
    }
}

impl<'a, T, Func> Iterator for SelectionIterator<'a, T, Func>
where
    T: RTreeObject,

    Func: SelectionFunc<T>,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        while let Some(next) = self.current_nodes.pop() {
            if self.func.is_contained_in(&next.envelope()) {
                match next {
                    RTreeNode::Leaf(ref t) => return Some(t),
                    RTreeNode::Parent(ref data) => self.current_nodes.extend(&data.children),
                }
            }
        }
        None
    }
}

pub struct SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject + 'a,
    Func: SelectionFunc<T>,
{
    func: Func,
    current_nodes: Vec<&'a mut RTreeNode<T>>,
}

impl<'a, T, Func> SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject,
    Func: SelectionFunc<T>,
{
    pub fn new<Params>(tree: &'a mut RTree<T, Params>, containment_unit: Func::ContainmentUnit) -> Self
    where
        Params: RTreeParams,
    {
        let func = Func::new(containment_unit);
        SelectionIteratorMut {
            func: func.clone(),
            current_nodes: tree
                .root_mut()
                .children
                .iter_mut()
                .filter(|c| func.is_contained_in(&c.envelope()))
                .collect(),
        }
    }
}

impl<'a, T, Func> Iterator for SelectionIteratorMut<'a, T, Func>
where
    T: RTreeObject,

    Func: SelectionFunc<T>,
{
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        let func = self.func.clone();
        if let Some(next) = self.current_nodes.pop() {
            match next {
                RTreeNode::Leaf(ref mut t) => Some(t),
                RTreeNode::Parent(ref mut data) => {
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
    use aabb::AABB;
    use envelope::Envelope;
    use object::RTreeObject;
    use rtree::RTree;
    use test_utilities::create_random_points;

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
        let points = create_random_points(NUM_POINTS, *b"pt=rylOgr/PHi,al");
        let mut tree = RTree::new();
        let mut aabb_list = Vec::new();
        for ps in points.as_slice().windows(2) {
            let rectangle = TestRectangle {
                aabb: AABB::from_points(ps),
            };
            tree.insert(rectangle.clone());
            aabb_list.push(rectangle)
        }

        let query_points = create_random_points(10, *b"pO5tp2r;xysMa1!y");
        for p in &query_points {
            let mut contained_sequential: Vec<_> = aabb_list
                .iter()
                .filter(|aabb| aabb.aabb.contains_point(p))
                .cloned()
                .collect();
            {
                let contained_rtree: Vec<_> = tree.locate_all_at_point(p).collect();
                for rectangle in &contained_rtree {
                    assert!(&contained_sequential.contains(rectangle));
                }
                assert_eq!(contained_sequential.len(), contained_rtree.len());
            }

            let contained_rtree_mut: Vec<_> = tree.locate_all_at_point_mut(p).collect();

            assert_eq!(contained_sequential.len(), contained_rtree_mut.len());

            for rectangle in &contained_rtree_mut {
                assert!(&contained_sequential.contains(rectangle));
            }
            for rectangle in &mut contained_sequential {
                assert!(&contained_rtree_mut.contains(&rectangle));
            }
        }
    }

    #[test]
    fn test_iteration() {
        const NUM_POINTS: usize = 1000;
        let points = create_random_points(NUM_POINTS, *b"di5syMmeTriCa1ly");
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
