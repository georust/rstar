use crate::node::ParentNode;
use crate::Envelope;
use crate::RTreeNode;
use crate::RTreeNode::*;
use crate::RTreeObject;

use alloc::vec::Vec;

#[cfg(doc)]
use crate::RTree;

/// Iterator returned by [`RTree::intersection_candidates_with_other_tree`].
pub struct IntersectionIterator<'a, T, U = T>
where
    T: RTreeObject,
    U: RTreeObject,
{
    todo_list: Vec<(&'a RTreeNode<T>, &'a RTreeNode<U>)>,
}

impl<'a, T, U> IntersectionIterator<'a, T, U>
where
    T: RTreeObject,
    U: RTreeObject<Envelope = T::Envelope>,
{
    pub(crate) fn new(root1: &'a ParentNode<T>, root2: &'a ParentNode<U>) -> Self {
        let mut intersections = IntersectionIterator {
            todo_list: Vec::new(),
        };
        intersections.add_intersecting_children(root1, root2);
        intersections
    }

    fn push_if_intersecting(&mut self, node1: &'a RTreeNode<T>, node2: &'a RTreeNode<U>) {
        if node1.envelope().intersects(&node2.envelope()) {
            self.todo_list.push((node1, node2));
        }
    }

    fn add_intersecting_children(
        &mut self,
        parent1: &'a ParentNode<T>,
        parent2: &'a ParentNode<U>,
    ) {
        if !parent1.envelope().intersects(&parent2.envelope()) {
            return;
        }
        let children1 = parent1
            .children()
            .iter()
            .filter(|c1| c1.envelope().intersects(&parent2.envelope()));

        for child1 in children1 {
            let children2 = parent2
                .children()
                .iter()
                .filter(|c2| c2.envelope().intersects(&parent1.envelope()));

            for child2 in children2 {
                self.push_if_intersecting(child1, child2);
            }
        }
    }
}

impl<'a, T, U> Iterator for IntersectionIterator<'a, T, U>
where
    T: RTreeObject,
    U: RTreeObject<Envelope = T::Envelope>,
{
    type Item = (&'a T, &'a U);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.todo_list.pop() {
            match next {
                (Leaf(t1), Leaf(t2)) => return Some((&t1, &t2)),
                (leaf @ Leaf(_), Parent(p)) => {
                    p.children()
                        .iter()
                        .for_each(|c| self.push_if_intersecting(leaf, c));
                }
                (Parent(p), leaf @ Leaf(_)) => {
                    p.children()
                        .iter()
                        .for_each(|c| self.push_if_intersecting(c, leaf));
                }
                (Parent(p1), Parent(p2)) => {
                    self.add_intersecting_children(p1, p2);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use crate::test_utilities::*;
    use crate::{Envelope, RTree, RTreeObject};

    #[test]
    fn test_intersection_between_trees() {
        let rectangles1 = create_random_rectangles(100, SEED_1);
        let rectangles2 = create_random_rectangles(42, SEED_2);

        let mut intersections_brute_force = Vec::new();
        for rectangle1 in &rectangles1 {
            for rectangle2 in &rectangles2 {
                if rectangle1.envelope().intersects(&rectangle2.envelope()) {
                    intersections_brute_force.push((rectangle1, rectangle2));
                }
            }
        }

        let tree1 = RTree::bulk_load(rectangles1.clone());
        let tree2 = RTree::bulk_load(rectangles2.clone());
        let mut intersections_from_trees = tree1
            .intersection_candidates_with_other_tree(&tree2)
            .collect::<Vec<_>>();

        intersections_brute_force.sort_by(|a, b| a.partial_cmp(b).unwrap());
        intersections_from_trees.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(intersections_brute_force, intersections_from_trees);
    }

    #[test]
    fn test_trivial_intersections() {
        let points1 = create_random_points(1000, SEED_1);
        let points2 = create_random_points(2000, SEED_2);
        let tree1 = RTree::bulk_load(points1);
        let tree2 = RTree::bulk_load(points2);

        assert_eq!(
            tree1
                .intersection_candidates_with_other_tree(&tree2)
                .count(),
            0
        );
        assert_eq!(
            tree1
                .intersection_candidates_with_other_tree(&tree1)
                .count(),
            tree1.size()
        );
    }
}
