use object::RTreeObject;
use envelope::Envelope;

pub trait SelectionFunc<T>: Clone
where
    T: RTreeObject,
{
    type ContainmentUnit;
    fn new(containment_unit: Self::ContainmentUnit) -> Self;
    fn is_contained_in(&self, envelope: &T::Envelope) -> bool;
}

pub struct SelectInEnvelopeFunc<T>
where
    T: RTreeObject,
{
    envelope: T::Envelope,
}

impl<T> Clone for SelectInEnvelopeFunc<T>
where
    T: RTreeObject,
{
    fn clone(&self) -> Self {
        SelectInEnvelopeFunc {
            envelope: self.envelope,
        }
    }
}

impl<T> SelectionFunc<T> for SelectInEnvelopeFunc<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = T::Envelope;

    fn new(containment_unit: T::Envelope) -> Self {
        SelectInEnvelopeFunc {
            envelope: containment_unit,
        }
    }

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_envelope(&self.envelope)
    }
}

pub struct SelectAtPointFunc<T>
where
    T: RTreeObject,
{
    point: <T::Envelope as Envelope>::Point,
}

impl<T> Clone for SelectAtPointFunc<T>
where
    T: RTreeObject,
{
    fn clone(&self) -> Self {
        SelectAtPointFunc {
            point: self.point,
        }
    }
}

impl<T> SelectionFunc<T> for SelectAtPointFunc<T>
where
    T: RTreeObject,
{
    type ContainmentUnit = <T::Envelope as Envelope>::Point;

    fn new(containment_unit: <T::Envelope as Envelope>::Point) -> Self {
        SelectAtPointFunc {
            point: containment_unit,
        }
    }

    fn is_contained_in(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }
}

#[derive(Clone)]
pub struct SelectAllFunc;

impl<T> SelectionFunc<T> for SelectAllFunc
where
    T: RTreeObject,
{
    type ContainmentUnit = ();

    fn new(_: ()) -> Self {
        SelectAllFunc
    }

    fn is_contained_in(&self, _: &T::Envelope) -> bool {
        true
    }
}
