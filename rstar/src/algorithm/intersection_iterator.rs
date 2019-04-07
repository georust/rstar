use crate::node::ParentNode;
use crate::Envelope;
use crate::RTreeNode;
use crate::RTreeNode::*;
use crate::RTreeObject;

pub struct IntersectionIterator<'a, T>
where
    T: RTreeObject,
{
    todo_list: Vec<(&'a RTreeNode<T>, &'a RTreeNode<T>)>,
}

impl<'a, T> IntersectionIterator<'a, T>
where
    T: RTreeObject,
{
    pub(crate) fn new(root1: &'a ParentNode<T>, root2: &'a ParentNode<T>) -> Self {
        let mut todo_list = Vec::new();
        extend_with_parent_intersections(root1, root2, &mut todo_list);
        IntersectionIterator { todo_list }
    }
}

impl<'a, T> Iterator for IntersectionIterator<'a, T>
where
    T: RTreeObject,
{
    type Item = (&'a T, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.todo_list.pop() {
            if !next.0.envelope().intersects(&next.1.envelope()) {
                continue;
            }
            match next {
                (Leaf(t1), Leaf(t2)) => return Some((&t1, &t2)),
                (leaf @ Leaf(_), Parent(p)) | (Parent(p), leaf @ Leaf(_)) => {
                    self.todo_list.extend(p.children.iter().map(|c| (c, leaf)));
                }
                (Parent(p1), Parent(p2)) => {
                    extend_with_parent_intersections(p1, p2, &mut self.todo_list)
                }
            }
        }
        None
    }
}

fn extend_with_parent_intersections<'a, T: RTreeObject>(
    parent1: &'a ParentNode<T>,
    parent2: &'a ParentNode<T>,
    result: &mut Vec<((&'a RTreeNode<T>, &'a RTreeNode<T>))>,
) {
    for child1 in &parent1.children {
        if child1.envelope().intersects(&parent2.envelope) {
            for child2 in &parent2.children {
                result.push((child1, child2))
            }
        }
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
