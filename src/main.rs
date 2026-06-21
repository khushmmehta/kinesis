mod app;
use app::App;
use winit::event_loop::EventLoop;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    env_logger::init();

    let event_loop = EventLoop::with_user_event().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
