use crate::PointExt;
use geo_types;

type LonLatFloat = f32;

#[derive(PartialEq, Clone, Copy, Debug, Default)]
struct GeoPoint(geo_types::Point<LonLatFloat>);

impl GeoPoint {
    const RE: f64 = 6378.137; // equatorial radius in km
    const FE: f64 = 1.0 / 298.257223563; // flattening
    const E2: f64 = Self::FE * (2.0 - Self::FE);

    pub fn new(x: LonLatFloat, y: LonLatFloat) -> Self {
        GeoPoint(geo_types::Point::new(x, y))
    }

    fn x(&self) -> LonLatFloat {
        self.0.x()
    }

    fn y(&self) -> LonLatFloat {
        self.0.y()
    }

    fn long_diff(a: LonLatFloat, b: LonLatFloat) -> LonLatFloat {
        let threesixty = 360.;
        let diff = a - b;
        diff - ((diff / threesixty).round() * threesixty)
    }

    fn multipliers_for_lonlat_converstion(latitude: LonLatFloat) -> (LonLatFloat, LonLatFloat) {
        let one = 1.;
        let e2 = Self::E2 as LonLatFloat;

        // Curvature formulas from https://en.wikipedia.org/wiki/Earth_radius#Meridional
        let coslat = latitude.to_radians().cos();
        let w2 = one / (one - e2 * (one - coslat * coslat));
        let w = w2.sqrt();

        // multipliers for converting longitude and latitude degrees into distance
        let dkx = w * coslat; // based on normal radius of curvature
        let dky = w * w2 * (one - e2); // based on meridonal radius of curvature

        Self::calculate_multipliers(dkx, dky)
    }

    fn calculate_multipliers(dkx: LonLatFloat, dky: LonLatFloat) -> (LonLatFloat, LonLatFloat) {
        let re = Self::RE as LonLatFloat;
        let mul = (1000. as LonLatFloat).to_radians() * re;
        let kx = mul * dkx;
        let ky = mul * dky;
        (kx, ky)
    }
}

impl PointExt for GeoPoint {
    fn sub(&self, other: &Self) -> Self {
        let (kx, ky) = Self::multipliers_for_lonlat_converstion(other.y());
        let dx = Self::long_diff(other.x(), self.x()) * kx;
        let dy = (other.y() - self.y()) * ky;
        Self::new(dx, dy)
    }
}

impl crate::Point for GeoPoint {
    type Scalar = LonLatFloat;

    const DIMENSIONS: usize = 2;

    fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self {
        GeoPoint::new(generator(0), generator(1))
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        match index {
            0 => self.0 .0.x,
            1 => self.0 .0.y,
            _ => unreachable!(),
        }
    }
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.0 .0.x,
            1 => &mut self.0 .0.y,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GeoPoint;
    use crate::RTree;

    #[test]
    fn test_rtree() {
        let p1_lat = 55.574778_f32;
        let p1_long = 38.057070;
        let p2_lat = 55.634198_f32;
        let p2_long = 38.087196;

        let q_lat = 55.61;
        let q_long = 38.06;

        let mut tree = RTree::new();
        tree.insert(GeoPoint::new(p1_long, p1_lat));
        tree.insert(GeoPoint::new(p2_long, p2_lat));
        let nn = tree.nearest_neighbor(&GeoPoint::new(q_long, q_lat));
        println!("{nn:?}");
    }
}
