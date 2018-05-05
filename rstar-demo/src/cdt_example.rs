// Copyright 2017 The Spade Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use graphics::{RenderData};
use spade::delaunay::{ConstrainedDelaunayTriangulation};
use spade::kernels::FloatKernel;
use cgmath::{Point2};
use glium::{DisplayBuild};
use glium::glutin::{Event, ElementState, MouseButton};
use glium::glutin::VirtualKeyCode;
use rand::Rng;

pub type Cdt = ConstrainedDelaunayTriangulation<Point2<f64>, FloatKernel>;

pub fn run() {

    let display = ::glium::glutin::WindowBuilder::new()
        .with_dimensions(800, 800)
        .with_title("CDT Demo".to_string())
        .build_glium()
        .unwrap();

    let mut cdt = Cdt::new();

    let mut render_data = RenderData::new(&display);

    let mut last_point = Point2::new(0., 0.);
    let mut last_handle = None;

    println!("CDT Demo");
    print_help();
    loop {
        let events: Vec<_> = display.poll_events().collect();

        let mut dirty = false;
        for event in events.into_iter() {
            match event {
                Event::Refresh => render_data.draw(&display),
                Event::Closed => return,
                Event::KeyboardInput(ElementState::Pressed, _, Some(key)) => {
                    match key {
                        VirtualKeyCode::Escape => return,
                        VirtualKeyCode::H => {
                            print_help();
                        },
                        VirtualKeyCode::A | VirtualKeyCode::B => {
                            // Insert some random points
                            let num = if key == VirtualKeyCode::A { 10usize } else { 100 };
                            let mut rng = ::rand::thread_rng();
                            
                            let seed = rng.gen();
                            let new_points = ::random_points_with_seed(num, seed);
                            for point in new_points.into_iter() {
                                cdt.insert(point);
                            }
                            render_data.update_cdt_buffers(&display, &cdt);
                            dirty = true;
                        },
                        VirtualKeyCode::D => {
                            let nn = cdt.nearest_neighbor(&last_point).map(|p| p.fix());
                            if let Some(handle) = nn {
                                cdt.remove(handle);
                                render_data.update_cdt_buffers(&display, &cdt);
                                let selection = get_selected_vertices(&cdt, last_point);
                                render_data.update_selection(&display, &selection);
                                render_data.update_selection_lines(&display, &vec![]);
                                last_handle = None;
                                dirty = true;
                            }
                        },
                        _ => (),
                    }
                },
                Event::MouseInput(ElementState::Pressed, MouseButton::Left) => {
                    cdt.insert(last_point);
                    render_data.update_cdt_buffers(&display, &cdt);
                    dirty = true;
                },
                Event::MouseInput(ElementState::Pressed, MouseButton::Right) => {
                    let nn = cdt.nearest_neighbor(&last_point).map(|p| p.fix());
                    if let Some(handle) = nn {
                        if let Some(last) = last_handle {
                            if cdt.can_add_constraint(last, handle) {
                                cdt.add_constraint(last, handle);
                                render_data.update_cdt_buffers(&display, &cdt);
                            }
                            last_handle = None;
                            render_data.update_selection_lines(&display, &vec![]);
                            dirty = true;
                        } else {
                            last_handle = Some(handle);
                        }
                    }
                },
                Event::MouseMoved(x, y) => {
                    let (w, h) = display.get_framebuffer_dimensions();
                    // Transform x, y into the range [-1 , 1]
                    let y = h as i32 - y;
                    let x = (x as f64 / w as f64) * 2. - 1.;
                    let y = (y as f64 / h as f64) * 2. - 1.;
                    last_point = Point2::new(x, y);
                    let selection = get_selected_vertices(&cdt, last_point);
                    render_data.update_selection(&display, &selection);
                    if let Some(last_handle) = last_handle {
                        let highlight_line = vec![
                            *cdt.vertex(last_handle), last_point];
                        render_data.update_selection_lines(
                            &display, &highlight_line);
                    }
                    dirty = true;
                },
                _ => (),
            }
        }
        if dirty {
            render_data.draw(&display);
        }
    }
}

fn get_selected_vertices(cdt: &Cdt, point: Point2<f64>) -> Vec<Point2<f64>> {    
    let mut points = Vec::new();
    points.extend(cdt.nearest_neighbor(&point).map(|p| (*p).clone()));
    points
}

fn print_help() {
    println!("H - print this help dialog");
    println!("A - add 10 random points.");
    println!("B - add 100 random points.");
    println!("D - delete closest point.");
    println!("--------------------------");
    println!("Left click: Add single point.");
    println!("Right click: start / end adding a constraint.");
    println!();
}
