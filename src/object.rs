use point::{Point, PointExt};
use envelope::Envelope;
use mbr::MBR;

pub trait RTreeObject: ::std::fmt::Debug {
    type Point: Point;
    type Envelope: Envelope<Point=Self::Point>;

    fn mbr(&self) -> Self::Envelope;

    fn distance_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;
}


impl <P> RTreeObject for P where P: Point {
    type Point = P;
    type Envelope = MBR<P>;

    fn mbr(&self) -> MBR<Self::Point> {
        MBR::from_point(*self)
    }

    fn distance_2(&self, point: &P) -> P::Scalar {
        <Self as PointExt>::distance_2(self, point)
    }
}
