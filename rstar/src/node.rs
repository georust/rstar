use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;

#[derive(Debug)]
pub enum RTreeNode<T>
where
    T: RTreeObject,
{
    Leaf(T),
    Parent(ParentNodeData<T>),
}

#[derive(Debug)]
pub struct ParentNodeData<T>
where
    T: RTreeObject,
{
    pub children: Vec<RTreeNode<T>>,
    pub envelope: T::Envelope,
}

impl<T> RTreeNode<T>
where
    T: RTreeObject,
{
    pub fn envelope(&self) -> T::Envelope {
        match self {
            RTreeNode::Leaf(ref t) => t.envelope(),
            RTreeNode::Parent(ref data) => data.envelope,
        }
    }

    pub fn is_leaf(&self) -> bool {
        match self {
            RTreeNode::Leaf(..) => true,
            RTreeNode::Parent(..) => false,
        }
    }
}

impl<T> ParentNodeData<T>
where
    T: RTreeObject,
{
    pub fn new_root<Params>() -> Self
    where
        Params: RTreeParams,
    {
        ParentNodeData {
            envelope: Envelope::new_empty(),
            children: Vec::with_capacity(Params::MAX_SIZE + 1),
        }
    }

    pub fn new_parent(children: Vec<RTreeNode<T>>) -> Self {
        let envelope = envelope_for_children(&children);

        ParentNodeData { envelope, children }
    }

    pub fn sanity_check<Params>(&self) -> Option<usize>
    where
        Params: RTreeParams,
    {
        if self.children.is_empty() {
            Some(0)
        } else {
            let mut result = None;
            self.sanity_check_inner::<Params>(1, &mut result);
            result
        }
    }

    fn sanity_check_inner<Params>(&self, height: usize, leaf_height: &mut Option<usize>)
    where
        Params: RTreeParams,
    {
        if height > 1 {
            let min_size = Params::MIN_SIZE;
            assert!(self.children.len() >= min_size);
        }
        let max_size = Params::MAX_SIZE;
        let mut envelope = T::Envelope::new_empty();
        assert!(self.children.len() <= max_size);
        for child in &self.children {
            match child {
                RTreeNode::Leaf(ref t) => {
                    envelope.merge(&t.envelope());
                    if let Some(ref leaf_height) = leaf_height {
                        assert_eq!(height, *leaf_height);
                    } else {
                        *leaf_height = Some(height);
                    }
                }
                RTreeNode::Parent(ref data) => {
                    envelope.merge(&data.envelope);
                    data.sanity_check_inner::<Params>(height + 1, leaf_height);
                }
            }
        }
        assert_eq!(self.envelope, envelope);
    }
}

impl<T> ParentNodeData<T>
where
    T: RTreeObject + PartialEq,
{
    pub fn contains(&self, t: &T) -> bool {
        let mut todo_list = Vec::with_capacity(20);
        todo_list.push(self);
        let t_envelope = t.envelope();
        while let Some(next) = todo_list.pop() {
            if next.envelope.contains_envelope(&t_envelope) {
                for child in &next.children {
                    match child {
                        RTreeNode::Parent(ref data) => {
                            todo_list.push(data);
                        }
                        RTreeNode::Leaf(ref obj) => {
                            if obj == t {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

pub fn envelope_for_children<T>(children: &[RTreeNode<T>]) -> T::Envelope
where
    T: RTreeObject,
{
    let mut result = T::Envelope::new_empty();
    for child in children {
        result.merge(&child.envelope());
    }
    result
}
