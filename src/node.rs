use std::marker::PhantomData;
use std::fmt::{Debug, Formatter, Result};
use params::RTreeParams;
use object::RTreeObject;
use typenum::Unsigned;
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
            &RTreeNode::Leaf(ref t) => write!(f, "RTreeNode::Leaf({:?})", t),
            &RTreeNode::Parent(ref data) => write!(f, "RTreeNode::Parent({:?})", data),
        }
    }
}

impl <T, Params> Debug for ParentNodeData<T, Params>
    where T: RTreeObject + Debug, Params: RTreeParams
{
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        fmt.debug_struct("ParentNodeData")
        .field("#children", &self.children.len())
        .field("mbr", &self.mbr)
        .finish()
    }
}

pub struct ParentNodeData<T, Params>
where T: RTreeObject,
      Params: RTreeParams,
{
    pub children: Vec<RTreeNode<T, Params>>,
    pub mbr: T::Envelope,
    _params: PhantomData<Params>,

}

impl <T, Params> RTreeNode<T, Params> 
    where Params: RTreeParams,
          T: RTreeObject
{
    pub fn mbr(&self) -> T::Envelope {
        match self {
            &RTreeNode::Leaf(ref t) => t.mbr(),
            &RTreeNode::Parent(ref data) => data.mbr,
        }
    }

    pub fn is_leaf(&self) -> bool {
        match self {
            &RTreeNode::Leaf(..) => true,
            &RTreeNode::Parent(..) => false,
        }
    }
}

#[cfg(feature = "debug")]
impl <T, Params> ::std::Debug for ParentNodeData<T, Params> 
    where Params: RTreeParams,
          T: RTreeObject,
{
    fn fmt(&self, f: &mut ::std::Formatter) -> ::std::Result {
        write!(f, "Parent - {:?} - (", self.children.len())?;
        for child in &self.children {
            match child {
                &RTreeNode::Parent(ref data) => {
                    write!(f, "{:?}, ", data)?;
                }
                _ => {}
            }
        }
        write!(f, ")")
    }
}

impl <T, Params> ParentNodeData<T, Params> 
    where Params: RTreeParams,
          T: RTreeObject,
{
    pub fn new_root() -> Self {
        ParentNodeData {
            mbr: Envelope::new_empty(),
            children: Vec::with_capacity(Params::MaxSize::to_usize() + 1),
            _params: Default::default(),
        }
    }

    pub fn new_parent(children: Vec<RTreeNode<T, Params>>) -> Self {
        let mbr = mbr_for_children(&children);
        
        ParentNodeData {
            mbr: mbr,
            children: children,
            _params: Default::default(),
        }
    }

    #[cfg(any(feature = "debug", test))]
    pub fn sanity_check(&self) -> Option<usize> {
        let mut result = None;
        self.sanity_check_inner(1, &mut result);
        result
    }

    #[cfg(any(feature = "debug", test))]
    fn sanity_check_inner(&self, height: usize, leaf_height: &mut Option<usize>) {
        if height > 1 {
            let min_size = Params::MinSize::to_usize();
            assert!(self.children.len() >= min_size);
        }
        let max_size = Params::MaxSize::to_usize();
        let mut mbr = T::Envelope::new_empty();
        assert!(self.children.len() <= max_size);
        for child in &self.children {
            match child {
                &RTreeNode::Leaf(ref t) => {
                    mbr.merge(&t.mbr());
                    if let &mut Some(leaf_height) = leaf_height {
                        assert_eq!(height, leaf_height);
                    } else {
                        *leaf_height = Some(height);
                    }
                },
                &RTreeNode::Parent(ref data) => {
                    mbr.merge(&data.mbr);
                    data.sanity_check_inner(height + 1, leaf_height);
                }
            }
        }
        assert_eq!(self.mbr, mbr);
    }
}

impl <T, Params> ParentNodeData<T, Params>
        where Params: RTreeParams,
              T: RTreeObject + PartialEq {

    // pub fn update_mbr(&mut self) {
    //     let mbr = mbr_for_children(&self.children);
    //     self.mbr = mbr;
    // }
    
    pub fn contains(&self, t: &T) -> bool {
        let mut todo_list = Vec::with_capacity(20);
        todo_list.push(self);
        let t_mbr = t.mbr();
        while let Some(next) = todo_list.pop() {
            if next.mbr.contains_envelope(&t_mbr) {
                for child in next.children.iter() {
                    match child {
                        &RTreeNode::Parent(ref data) => {
                            todo_list.push(data);
                        },
                        &RTreeNode::Leaf(ref obj) => {
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

pub fn mbr_for_children<T, Params>(children: &[RTreeNode<T, Params>]) -> T::Envelope
    where T: RTreeObject,
          Params: RTreeParams
{
    let mut result = T::Envelope::new_empty();
    for child in children {
        result.merge(&child.mbr());
    }
    result
}
