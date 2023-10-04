#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(offset_of)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access
)]

mod app;
mod buffer;
mod context;
mod render;
mod util;
mod vertex;

use winit::event_loop::EventLoop;

use app::App;

fn main() {
    let event_loop = EventLoop::new();
    let window = App::init_window(&event_loop);
    let app = App::new(&window);
    app.run(event_loop, window);
}