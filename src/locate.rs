use params::RTreeParams;
use object::RTreeObject;
use rtree::RTree;
use node::RTreeNode;
use envelope::Envelope;

pub struct LocateAll<'a, T, Params>
where
    T: RTreeObject + 'a,
    Params: RTreeParams + 'a,
{
    point: T::Point,
    current_nodes: Vec<&'a RTreeNode<T, Params>>,
}

impl<'a, T, Params> LocateAll<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    pub fn new(tree: &'a RTree<T, Params>, point: T::Point) -> Self {
        let current_nodes = if tree.root().children.is_empty() {
            Vec::new()
        } else {
            tree.root()
                .children
                .iter()
                .filter(|c| c.envelope().contains_point(&point))
                .collect()
        };
        LocateAll {
            point: point,
            current_nodes: current_nodes,
        }
    }
}

impl<'a, T, Params> Iterator for LocateAll<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let point = self.point;
        if let Some(next) = self.current_nodes.pop() {
            match next {
                &RTreeNode::Leaf(ref t) => Some(t),
                &RTreeNode::Parent(ref data) => {
                    self.current_nodes.extend(
                        data.children
                            .iter()
                            .filter(|c| c.envelope().contains_point(&point)),
                    );
                    self.next()
                }
            }
        } else {
            None
        }
    }
}

pub struct LocateAllMut<'a, T, Params>
where
    T: RTreeObject + 'a,
    Params: RTreeParams + 'a,
{
    point: T::Point,
    current_nodes: Vec<&'a mut RTreeNode<T, Params>>,
}

impl<'a, T, Params> LocateAllMut<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    pub fn new(tree: &'a mut RTree<T, Params>, point: T::Point) -> Self {
        let current_nodes = if tree.root().children.is_empty() {
            Vec::new()
        } else {
            tree.root_mut()
                .children
                .iter_mut()
                .filter(|c| c.envelope().contains_point(&point))
                .collect()
        };
        LocateAllMut {
            point: point,
            current_nodes: current_nodes,
        }
    }
}

impl<'a, T, Params> Iterator for LocateAllMut<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        let point = self.point;
        if let Some(next) = self.current_nodes.pop() {
            match next {
                &mut RTreeNode::Leaf(ref mut t) => Some(t),
                &mut RTreeNode::Parent(ref mut data) => {
                    self.current_nodes.extend(
                        data.children
                            .iter_mut()
                            .filter(|c| c.envelope().contains_point(&point)),
                    );
                    self.next()
                }
            }
        } else {
            None
        }
    }
}


pub struct LocateInEnvelope<'a, T, Params>
where
    T: RTreeObject + 'a,
    Params: RTreeParams + 'a,
{
    envelope: T::Envelope,
    current_nodes: Vec<&'a RTreeNode<T, Params>>,
}

impl<'a, T, Params> LocateInEnvelope<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    pub fn new(tree: &'a RTree<T, Params>, envelope: T::Envelope) -> Self {
        let current_nodes = if tree.root().children.is_empty() {
            Vec::new()
        } else {
            tree.root()
                .children
                .iter()
                .filter(|c| c.envelope().contains_envelope(&envelope))
                .collect()
        };
        LocateInEnvelope {
            envelope: envelope,
            current_nodes: current_nodes,
        }
    }
}

impl<'a, T, Params> Iterator for LocateInEnvelope<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let envelope = self.envelope;
        if let Some(next) = self.current_nodes.pop() {
            match next {
                &RTreeNode::Leaf(ref t) => Some(t),
                &RTreeNode::Parent(ref data) => {
                    self.current_nodes.extend(
                        data.children
                            .iter()
                            .filter(|c| c.envelope().contains_envelope(&envelope)),
                    );
                    self.next()
                }
            }
        } else {
            None
        }
    }
}


pub struct LocateInEnvelopeMut<'a, T, Params>
where
    T: RTreeObject + 'a,
    Params: RTreeParams + 'a,
{
    envelope: T::Envelope,
    current_nodes: Vec<&'a mut RTreeNode<T, Params>>,
}

impl<'a, T, Params> LocateInEnvelopeMut<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    pub fn new(tree: &'a mut RTree<T, Params>, envelope: T::Envelope) -> Self {
        let current_nodes = if tree.root().children.is_empty() {
            Vec::new()
        } else {
            tree.root_mut()
                .children
                .iter_mut()
                .filter(|c| c.envelope().contains_envelope(&envelope))
                .collect()
        };
        LocateInEnvelopeMut {
            envelope: envelope,
            current_nodes: current_nodes,
        }
    }
}

impl<'a, T, Params> Iterator for LocateInEnvelopeMut<'a, T, Params>
where
    T: RTreeObject,
    Params: RTreeParams,
{
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        let envelope = self.envelope;
        if let Some(next) = self.current_nodes.pop() {
            match next {
                &mut RTreeNode::Leaf(ref mut t) => Some(t),
                &mut RTreeNode::Parent(ref mut data) => {
                    self.current_nodes.extend(
                        data.children
                            .iter_mut()
                            .filter(|c| c.envelope().contains_envelope(&envelope)),
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
        type Point = [f64; 2];
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
                let contained_rtree: Vec<_> = tree.locate_all(p).collect();
                for rect in &contained_rtree {
                    assert!(&contained_sequential.contains(rect));
                }
                assert_eq!(contained_sequential.len(), contained_rtree.len());
            }

            let contained_rtree_mut: Vec<_> = tree.locate_all_mut(p).collect();

            assert_eq!(contained_sequential.len(), contained_rtree_mut.len());

            for rect in &contained_rtree_mut {
                assert!(&contained_sequential.contains(rect));
            }
            for rect in &mut contained_sequential {
                assert!(&contained_rtree_mut.contains(&rect));
            }
        }
    }
}
