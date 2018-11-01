use crate::aabb::AABB;
use crate::envelope::Envelope;
use crate::point::{EuclideanPoint, Point, PointExt};

pub trait RTreeObject {
    type Envelope: Envelope;

    fn envelope(&self) -> Self::Envelope;
}

pub trait PointDistance: RTreeObject {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar;
}

impl<P> RTreeObject for P
where
    P: EuclideanPoint,
{
    type Envelope = AABB<P>;

    fn envelope(&self) -> AABB<P> {
        AABB::from_point(*self)
    }
}

impl<P> PointDistance for P
where
    P: EuclideanPoint,
{
    fn distance_2(&self, point: &P) -> P::Scalar {
        <Self as PointExt>::distance_2(self, point)
    }
}
