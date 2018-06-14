use std::marker::PhantomData;
use std::fmt::{Debug, Formatter, Result};
use params::RTreeParams;
use object::RTreeObject;
use envelope::Envelope;

pub enum RTreeNode<T, Params> 
    where T: RTreeObject,
          Params: RTreeParams,
{
    Leaf(T),
    Parent(ParentNodeData<T, Params>),
}

impl <T, Params> Debug for RTreeNode<T, Params>
    where T: RTreeObject + Debug, Params: RTreeParams
{
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            RTreeNode::Leaf(ref t) => write!(f, "RTreeNode::Leaf({:?})", t),
            RTreeNode::Parent(ref data) => write!(f, "RTreeNode::Parent({:?})", data),
        }
    }
}

impl <T, Params> Debug for ParentNodeData<T, Params>
    where T: RTreeObject + Debug, Params: RTreeParams
{
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        fmt.debug_struct("ParentNodeData")
        .field("#children", &self.children.len())
        .field("envelope", &self.envelope)
        .finish()
    }
}

pub struct ParentNodeData<T, Params>
where T: RTreeObject,
      Params: RTreeParams,
{
    pub children: Vec<RTreeNode<T, Params>>,
    pub envelope: T::Envelope,
    _params: PhantomData<Params>,

}

impl <T, Params> RTreeNode<T, Params> 
    where Params: RTreeParams,
          T: RTreeObject
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

impl <T, Params> ParentNodeData<T, Params> 
    where Params: RTreeParams,
          T: RTreeObject,
{
    pub fn new_root() -> Self {
        ParentNodeData {
            envelope: Envelope::new_empty(),
            children: Vec::with_capacity(Params::MAX_SIZE + 1),
            _params: Default::default(),
        }
    }

    pub fn new_parent(children: Vec<RTreeNode<T, Params>>) -> Self {
        let envelope = envelope_for_children(&children);
        
        ParentNodeData {
            envelope,
            children,
            _params: Default::default(),
        }
    }

    pub fn sanity_check(&self) -> Option<usize> {
        let mut result = None;
        self.sanity_check_inner(1, &mut result);
        result
    }

    fn sanity_check_inner(&self, height: usize, leaf_height: &mut Option<usize>) {
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
                },
                RTreeNode::Parent(ref data) => {
                    envelope.merge(&data.envelope);
                    data.sanity_check_inner(height + 1, leaf_height);
                }
            }
        }
        assert_eq!(self.envelope, envelope);
    }
}

impl <T, Params> ParentNodeData<T, Params>
        where Params: RTreeParams,
              T: RTreeObject + PartialEq {

    // pub fn update_envelope(&mut self) {
    //     let envelope = envelope_for_children(&self.children);
    //     self.envelope = envelope;
    // }
    
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
                        },
                        RTreeNode::Leaf(ref obj) => {
                            if obj == t {
                                return true;
                            }
                        },
                    }
                }
            }
        }
        false
    }
}

pub fn envelope_for_children<T, Params>(children: &[RTreeNode<T, Params>]) -> T::Envelope
    where T: RTreeObject,
          Params: RTreeParams
{
    let mut result = T::Envelope::new_empty();
    for child in children {
        result.merge(&child.envelope());
    }
    result
}
