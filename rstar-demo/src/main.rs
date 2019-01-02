use kiss3d::camera::ArcBall;
use kiss3d::event::{Action, WindowEvent};
use kiss3d::light::Light;
use kiss3d::text::Font;
use kiss3d::window::Window;
use nalgebra::{Point2, Point3};
use rand::distributions::Uniform;
use rand::Rng;
use rstar::{root, RStarInsertionStrategy, RTree, RTreeNode, RTreeParams, AABB};

mod buttons {
    use kiss3d::event::Key;
    pub const ADD_BUTTON: Key = Key::R;
    pub const ADD_MANY_BUTTON: Key = Key::T;
}

type DemoTree = RTree<TreePointType, Params>;
type TreePointType = [f32; 3];

struct Params;
impl RTreeParams for Params {
    const MIN_SIZE: usize = 5;
    const MAX_SIZE: usize = 9;
    const REINSERTION_COUNT: usize = 3;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}

type RenderData = (Vec<LineRenderData>, Vec<PointRenderData>);
type LineRenderData = (Point3<f32>, Point3<f32>, Point3<f32>);
type PointRenderData = (Point3<f32>, Point3<f32>);

fn main() {
    let mut window = Window::new_with_size("RStar demo", 1024, 768);
    window.set_background_color(1.0, 1.0, 1.0);
    window.set_light(Light::StickToCamera);
    window.set_point_size(4.);

    let mut camera = ArcBall::new(Point3::new(3.0, 2.5, 2.0), Point3::origin());

    let points = create_random_points(50);

    let mut tree = RTree::bulk_load_with_params(points);
    let mut render_data = create_render_data_for_tree(&tree);

    while window.render_with_camera(&mut camera) {
        render_data = handle_input(&window, &mut tree).unwrap_or(render_data);
        draw_tree(&mut window, &render_data);
        draw_help(&mut window);
    }
}

pub fn create_random_points(num_points: usize) -> Vec<TreePointType> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = rand::thread_rng();
    let distribution = Uniform::new(-1.0f32, 1.0);
    for _ in 0..num_points {
        let mut new_point: TreePointType = Default::default();
        for coordinate in &mut new_point {
            *coordinate = rng.sample(&distribution);
        }
        result.push(new_point);
    }
    result
}

fn handle_input(window: &Window, tree: &mut DemoTree) -> Option<RenderData> {
    let mut points_to_add = Vec::new();
    for event in window.events().iter() {
        match event.value {
            WindowEvent::Key(buttons::ADD_BUTTON, Action::Press, _) => {
                points_to_add = create_random_points(10);
            }
            WindowEvent::Key(buttons::ADD_MANY_BUTTON, Action::Press, _) => {
                points_to_add = create_random_points(100);
            }
            _ => (),
        }
    }
    if !points_to_add.is_empty() {
        for point in points_to_add {
            tree.insert(point);
        }
        create_render_data_for_tree(tree).into()
    } else {
        None
    }
}

fn draw_tree(window: &mut Window, render_data: &RenderData) {
    let (ref line_data, ref point_data) = render_data;
    for (from, to, color) in line_data {
        window.draw_line(&from, &to, &color);
    }
    for (point, color) in point_data {
        window.draw_point(&point, &color);
    }
}

fn draw_help(window: &mut Window) {
    let font = Font::default();
    window.draw_text(
        &help_text(),
        &Point2::new(5.0, 5.0),
        60.0,
        &font,
        &Point3::new(0.0, 0.0, 0.0),
    );
}

fn help_text() -> String {
    format!(
        "{:?}: Add 10 points\n\
         {:?}: Add 100 points\n\
         LMB: Rotate view\n\
         RMB: Pan view\n\
         Mouse wheel: Zoom",
        buttons::ADD_BUTTON,
        buttons::ADD_MANY_BUTTON
    )
}

fn create_render_data_for_tree(tree: &DemoTree) -> RenderData {
    let mut vertices = Vec::new();
    let mut lines = Vec::new();
    let vertex_color = [0.0, 0.0, 1.0].into();
    let mut to_visit = vec![(root(tree), 0)];
    while let Some((cur, depth)) = to_visit.pop() {
        push_cuboid(&mut lines, get_color_for_depth(depth), &cur.envelope);
        for child in &cur.children {
            match child {
                RTreeNode::Leaf(point) => vertices.push((
                    Point3::new(point[0] as f32, point[1] as f32, point[2] as f32),
                    vertex_color,
                )),
                RTreeNode::Parent(ref data) => {
                    to_visit.push((data, depth + 1));
                }
            }
        }
    }
    (lines, vertices)
}

fn push_cuboid(
    result: &mut Vec<(Point3<f32>, Point3<f32>, Point3<f32>)>,
    color: Point3<f32>,
    envelope: &AABB<TreePointType>,
) {
    let c000: Point3<_> = envelope.lower().into();
    let c111: Point3<_> = envelope.upper().into();
    let c001 = Point3::new(c000[0], c000[1], c111[2]);
    let c010 = Point3::new(c000[0], c111[1], c000[2]);
    let c011 = Point3::new(c000[0], c111[1], c111[2]);
    let c100 = Point3::new(c111[0], c000[1], c000[2]);
    let c101 = Point3::new(c111[0], c000[1], c111[2]);
    let c110 = Point3::new(c111[0], c111[1], c000[2]);
    result.push((c000, c001, color));
    result.push((c000, c010, color));
    result.push((c000, c100, color));
    result.push((c001, c011, color));
    result.push((c001, c101, color));
    result.push((c010, c011, color));
    result.push((c010, c110, color));
    result.push((c011, c111, color));
    result.push((c100, c101, color));
    result.push((c100, c110, color));
    result.push((c101, c111, color));
    result.push((c110, c111, color));
}

fn get_color_for_depth(depth: usize) -> Point3<f32> {
    match depth {
        0 => Point3::new(0., 0., 0.),
        1 => Point3::new(0.85, 0., 0.85),
        2 => Point3::new(0., 0.85, 0.85),
        3 => Point3::new(0., 0., 0.55),
        4 => Point3::new(0.85, 0., 0.),
        _ => Point3::new(0., 0.85, 0.85),
    }
}
