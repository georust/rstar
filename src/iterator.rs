use object::RTreeObject;
use params::RTreeParams;
use node::{RTreeNode};
use rtree::RTree;
use typenum::Unsigned;

pub struct RTreeIterator<'a, T, Params> 
    where T: RTreeObject + 'a,
          Params: RTreeParams + 'a,
{
    path: Vec<&'a RTreeNode<T, Params>>,
}

impl <'a, T, Params> RTreeIterator<'a, T, Params> 
    where T: RTreeObject,
          Params: RTreeParams
{
    pub fn new(tree: &'a RTree<T, Params>) -> Self {
        let mut path = Vec::with_capacity(Params::MaxSize::to_usize() * 4);
        path.extend(tree.root().children.iter());
        println!("path: {:?}", path);
        RTreeIterator {
            path: path,
        }
    }
}

impl <'a, T, Params> Iterator for RTreeIterator<'a, T, Params>
    where T: RTreeObject + 'a,
          Params: RTreeParams + 'a,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        while let Some(next) = self.path.pop() {
            match next {
                &RTreeNode::Parent(ref data) => {
                    self.path.extend(data.children.iter());
                },
                &RTreeNode::Leaf(ref t) => { return Some(t) },
            }
        }
        None
    }
}

pub struct RTreeIteratorMut<'a, T, Params> 
    where T: RTreeObject + 'a,
          Params: RTreeParams + 'a,
{
    path: Vec<&'a mut RTreeNode<T, Params>>,
}

impl <'a, T, Params> RTreeIteratorMut<'a, T, Params> 
    where T: RTreeObject,
          Params: RTreeParams
{
    pub fn new(tree: &'a mut RTree<T, Params>) -> Self {
        let mut path = Vec::with_capacity(Params::MaxSize::to_usize() * 4);
        path.extend(tree.root_mut().children.iter_mut());
        RTreeIteratorMut {
            path: path,
        }
    }
}

impl <'a, T, Params> Iterator for RTreeIteratorMut<'a, T, Params>
    where T: RTreeObject + 'a,
          Params: RTreeParams + 'a,
{
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        while let Some(next) = self.path.pop() {
            match next {
                &mut RTreeNode::Parent(ref mut data) => {
                    self.path.extend(data.children.iter_mut());
                },
                &mut RTreeNode::Leaf(ref mut t) => { return Some(t) },
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use testutils::create_random_points;
    use rtree::RTree;

    #[test]
    fn test_iteration() {
        const NUM_POINTS: usize = 1000;
        let points = create_random_points(NUM_POINTS,
            [921545, 22305, 2004822, 142567]);
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
