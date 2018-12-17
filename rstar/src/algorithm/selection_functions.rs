use crate::envelope::Envelope;
use crate::object::PointDistance;
use crate::object::RTreeObject;
use crate::structures::node::RTreeNode;

pub trait SelectionFunction<T>
where
    T: RTreeObject,
{
    type ContainmentUnit;
    fn should_unpack_parent(&self, envelope: &T::Envelope) -> bool;

    fn should_unpack_leaf(&self, leaf: &T) -> bool {
        self.should_unpack_parent(&leaf.envelope())
    }

    fn should_unpack_node(&self, node: &RTreeNode<T>) -> bool {
        match node {
            RTreeNode::Parent(ref data) => self.should_unpack_parent(&data.envelope),
            RTreeNode::Leaf(ref t) => self.should_unpack_leaf(t),
        }
    }
}

pub struct SelectInEnvelopeFunction<T>
where
    T: RTreeObject,
{
    envelope: T::Envelope,
}

impl<T> SelectInEnvelopeFunction<T>
where
    T: RTreeObject,
{
    pub fn new(envelope: T::Envelope) -> Self {
        SelectInEnvelopeFunction { envelope }
    }
}

impl<T> SelectionFunction<T> for SelectInEnvelopeFunction<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = T::Envelope;

    fn should_unpack_parent(&self, parent_envelope: &T::Envelope) -> bool {
        self.envelope.intersects(parent_envelope)
    }

    fn should_unpack_leaf(&self, leaf: &T) -> bool {
        self.envelope.contains_envelope(&leaf.envelope())
    }
}

pub struct SelectInEnvelopeFuncIntersecting<T>
where
    T: RTreeObject,
{
    envelope: T::Envelope,
}

impl<T> SelectInEnvelopeFuncIntersecting<T>
where
    T: RTreeObject,
{
    pub fn new(envelope: T::Envelope) -> Self {
        SelectInEnvelopeFuncIntersecting { envelope }
    }
}

impl<T> SelectionFunction<T> for SelectInEnvelopeFuncIntersecting<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = T::Envelope;

    fn should_unpack_parent(&self, envelope: &T::Envelope) -> bool {
        self.envelope.intersects(&envelope)
    }
}

pub struct SelectAllFunc;

impl<T> SelectionFunction<T> for SelectAllFunc
where
    T: RTreeObject,
{
    type ContainmentUnit = ();

    fn should_unpack_parent(&self, _: &T::Envelope) -> bool {
        true
    }
}

/// A [trait.SelectionFunction] that only selects elements whose envelope
/// contains a specific point.
pub struct SelectAtPointFunction<T>
where
    T: RTreeObject,
{
    point: <T::Envelope as Envelope>::Point,
}

impl<T> SelectAtPointFunction<T>
where
    T: PointDistance,
{
    pub fn new(point: <T::Envelope as Envelope>::Point) -> Self {
        SelectAtPointFunction { point }
    }
}

impl<T> SelectionFunction<T> for SelectAtPointFunction<T>
where
    T: PointDistance,
{
    type ContainmentUnit = <T::Envelope as Envelope>::Point;

    fn should_unpack_parent(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }

    fn should_unpack_leaf(&self, leaf: &T) -> bool {
        leaf.contains_point(&self.point)
    }
}

/// A selection function that only chooses elements equal (`==`) to a
/// given element
pub struct SelectEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq + 'a,
{
    /// Only elements equal to this object will be removed.
    object_to_remove: &'a T,
}

impl<'a, T> SelectEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    pub fn new(object_to_remove: &'a T) -> Self {
        SelectEqualsFunction { object_to_remove }
    }
}

impl<'a, T> SelectionFunction<T> for SelectEqualsFunction<'a, T>
where
    T: RTreeObject + PartialEq,
{
    type ContainmentUnit = &'a T;

    fn should_unpack_parent(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_envelope(&self.object_to_remove.envelope())
    }

    fn should_unpack_leaf(&self, leaf: &T) -> bool {
        leaf == self.object_to_remove
    }
}
