use point::{Point, PointExt};
use mbr::MBR;

pub trait RTreeObject {
    type Point: Point;

    fn mbr(&self) -> MBR<Self::Point>;

    fn distance_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;
}


impl <P> RTreeObject for P where P: Point {
    type Point = P;

    fn mbr(&self) -> MBR<Self::Point> {
        MBR::from_point(*self)
    }

    fn distance_2(&self, point: &P) -> P::Scalar {
        <Self as PointExt>::distance_2(self, point)
    }
}
