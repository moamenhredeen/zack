mod app;

use crate::app::App;
use winit::{
    event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, window::Window
};

fn main() {
    match EventLoop::new() {
        Ok(event_loop) => {
            event_loop.set_control_flow(ControlFlow::Wait);

            let mut app = App::default();
            match event_loop.run_app(&mut app)  {
                Ok(_) => { /* nothing todo */ },
                Err(err) => {
                    println!("error: {}", err);
                },
            }
        },
        Err(err) => println!("something went wrong {}", err),
    }

}
