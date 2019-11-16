//! Contains primitives ready for insertion into an r-tree.

mod line;
mod rectangle;
mod point_with_data;

pub use self::line::Line;
pub use self::rectangle::Rectangle;
pub use self::point_with_data::PointWithData;