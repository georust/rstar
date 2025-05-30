use crate::object::PointDistance;
use crate::object::RTreeObject;
use crate::{envelope::Envelope, object::Distance};
use core::ops::Deref;

/// An [RTreeObject] that is a possibly short-lived reference to another object.
///
/// Sometimes it can be useful to build an [RTree] that does not own its constituent
/// objects but references them from elsewhere. Wrapping the bare references with this
/// combinator makes this possible.
///
/// **Note:** the wrapper implements [RTreeObject] and referenced object `T` can be
/// accessed via an implementation of `Deref<Target=T>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef<'a, T: RTreeObject> {
    inner: &'a T,
}

impl<T: RTreeObject> RTreeObject for ObjectRef<'_, T> {
    type Envelope = T::Envelope;

    fn envelope(&self) -> Self::Envelope {
        self.inner.envelope()
    }
}

impl<T: PointDistance> PointDistance for ObjectRef<'_, T> {
    fn distance_2(&self, point: &<Self::Envelope as Envelope>::Point) -> Distance<Self> {
        self.inner.distance_2(point)
    }

    fn contains_point(&self, p: &<Self::Envelope as Envelope>::Point) -> bool {
        self.inner.contains_point(p)
    }

    fn distance_2_if_less_or_equal(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
        max_distance_2: Distance<Self>,
    ) -> Option<Distance<Self>> {
        self.inner
            .distance_2_if_less_or_equal(point, max_distance_2)
    }
}

impl<'a, T: RTreeObject> ObjectRef<'a, T> {
    /// Create a new [ObjectRef] struct using the object.
    pub fn new(inner: &'a T) -> Self {
        Self { inner }
    }
}

impl<T: RTreeObject> Deref for ObjectRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}
