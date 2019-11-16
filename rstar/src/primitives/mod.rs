//! Contains primitives ready for insertion into an r-tree.

mod line;
mod point_with_data;
mod rectangle;

pub use self::line::Line;
pub use self::point_with_data::PointWithData;
pub use self::rectangle::Rectangle;
