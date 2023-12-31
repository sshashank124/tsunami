#![allow(incomplete_features)]
#![feature(adt_const_params, generic_const_exprs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access
)]

mod app;
mod data;
mod gpu;
mod input;
mod render;
mod util;

use std::env;

use winit::event_loop::EventLoop;

use app::App;

fn main() {
    let gltf_file = env::args().nth(1).expect("Please specify a gltf file");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let window = App::window_builder()
        .build(&event_loop)
        .expect("Failed to create window");
    let app = App::new(&window, &gltf_file);
    app.run(event_loop);
}
