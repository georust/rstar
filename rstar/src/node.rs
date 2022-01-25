use crate::envelope::Envelope;
use crate::object::RTreeObject;
use crate::params::RTreeParams;

use alloc::vec::Vec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(
        serialize = "T: Serialize, T::Envelope: Serialize",
        deserialize = "T: Deserialize<'de>, T::Envelope: Deserialize<'de>"
    ))
)]

/// An internal tree node.
///
/// For most applications, using this type should not be required.
pub enum RTreeNode<T>
where
    T: RTreeObject,
{
    /// A leaf node, only containing the r-tree object
    Leaf(T),
    /// A parent node containing several child nodes
    Parent(ParentNode<T>),
}

/// Represents an internal parent node.
///
/// For most applications, using this type should not be required. Allows read access to this
/// node's envelope and its children.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParentNode<T>
where
    T: RTreeObject,
{
    pub(crate) children: Vec<RTreeNode<T>>,
    pub(crate) envelope: T::Envelope,
}

impl<T> RTreeObject for RTreeNode<T>
where
    T: RTreeObject,
{
    type Envelope = T::Envelope;

    fn envelope(&self) -> Self::Envelope {
        match self {
            RTreeNode::Leaf(ref t) => t.envelope(),
            RTreeNode::Parent(ref data) => data.envelope,
        }
    }
}

#[doc(hidden)]
impl<T> RTreeNode<T>
where
    T: RTreeObject,
{
    pub fn is_leaf(&self) -> bool {
        match self {
            RTreeNode::Leaf(..) => true,
            RTreeNode::Parent(..) => false,
        }
    }
}

impl<T> ParentNode<T>
where
    T: RTreeObject,
{
    /// Returns this node's children
    pub fn children(&self) -> &[RTreeNode<T>] {
        &self.children
    }

    /// Returns the smallest envelope that encompasses all children.
    pub fn envelope(&self) -> T::Envelope {
        self.envelope
    }

    pub(crate) fn new_root<Params>() -> Self
    where
        Params: RTreeParams,
    {
        ParentNode {
            envelope: Envelope::new_empty(),
            children: Vec::with_capacity(Params::MAX_SIZE + 1),
        }
    }

    pub(crate) fn new_parent(children: Vec<RTreeNode<T>>) -> Self {
        let envelope = envelope_for_children(&children);

        ParentNode { envelope, children }
    }

    #[cfg(test)]
    pub fn sanity_check<Params>(&self, check_max_size: bool) -> Option<usize>
    where
        Params: RTreeParams,
    {
        if self.children.is_empty() {
            Some(0)
        } else {
            let mut result = None;
            self.sanity_check_inner::<Params>(check_max_size, 1, &mut result);
            result
        }
    }

    #[cfg(test)]
    fn sanity_check_inner<Params>(
        &self,
        check_max_size: bool,
        height: usize,
        leaf_height: &mut Option<usize>,
    ) where
        Params: RTreeParams,
    {
        if height > 1 {
            let min_size = Params::MIN_SIZE;
            assert!(self.children.len() >= min_size);
        }
        let mut envelope = T::Envelope::new_empty();
        if check_max_size {
            let max_size = Params::MAX_SIZE;
            assert!(self.children.len() <= max_size);
        }

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
                    data.sanity_check_inner::<Params>(check_max_size, height + 1, leaf_height);
                }
            }
        }
        assert_eq!(self.envelope, envelope);
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
