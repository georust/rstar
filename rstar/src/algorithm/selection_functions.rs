use crate::envelope::Envelope;
use crate::object::RTreeObject;

pub trait SelectionFunc<T> : Clone
where
    T: RTreeObject,
{
    type ContainmentUnit;
    fn is_contained_in(&self, envelope: &T::Envelope) -> bool;
}

pub struct SelectInEnvelopeFunc<T>
where
    T: RTreeObject,
{
    envelope: T::Envelope,
}

impl<T> Clone for SelectInEnvelopeFunc<T> where T: RTreeObject
{
    fn clone(&self) -> Self {
        SelectInEnvelopeFunc {
            envelope: self.envelope
        }
    }
}

impl<T> SelectInEnvelopeFunc<T>
where
    T: RTreeObject,
{
    pub fn new(envelope: T::Envelope) -> Self {
        SelectInEnvelopeFunc { envelope }
    }
}

impl<T> SelectionFunc<T> for SelectInEnvelopeFunc<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = T::Envelope;

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_envelope(&self.envelope)
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

impl<T> SelectionFunc<T> for SelectInEnvelopeFuncIntersecting<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = T::Envelope;

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        self.envelope.intersects(&envelope)
    }
}

impl <T> Clone for SelectInEnvelopeFuncIntersecting<T> where T: RTreeObject
{
    fn clone(&self) -> Self {
        SelectInEnvelopeFuncIntersecting {
            envelope: self.envelope
        }
    }
}

pub struct SelectAtPointFunc<T>
where
    T: RTreeObject,
{
    point: <T::Envelope as Envelope>::Point,
}

impl<T> SelectAtPointFunc<T>
where
    T: RTreeObject,
{
    pub fn new(point: <T::Envelope as Envelope>::Point) -> Self {
        SelectAtPointFunc { point }
    }
}

impl<T> SelectionFunc<T> for SelectAtPointFunc<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = <T::Envelope as Envelope>::Point;

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }
}

impl<T> Clone for SelectAtPointFunc<T>
where T: RTreeObject
{
    fn clone(&self) -> Self {
        SelectAtPointFunc {
            point: self.point
        }
    }
}

#[derive(Clone)]
pub struct SelectAllFunc;

impl<T> SelectionFunc<T> for SelectAllFunc
where
    T: RTreeObject,
{
    type ContainmentUnit = ();

    fn is_contained_in(&self, _: &T::Envelope) -> bool {
        true
    }
}
