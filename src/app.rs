mod engine;
use std::{sync::Arc, time::Instant};

use engine::Engine;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::Window,
};

pub struct App {
    engine: Option<Engine>,
    last_time: Instant,
    i: std::time::Duration,
    polled_frametime: u32,
}

impl App {
    pub fn new() -> Self {
        Self {
            engine: None,
            last_time: Instant::now(),
            i: std::time::Duration::ZERO,
            polled_frametime: 0,
        }
    }
}

impl ApplicationHandler<Engine> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.engine = Some(pollster::block_on(Engine::new(window)).unwrap());
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut event: Engine) {
        self.engine = Some(event);
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let engine = match &mut self.engine {
            Some(canvas) => canvas,
            None => return,
        };

        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event
            && engine.mouse_pressed
        {
            engine.camera_controller.handle_mouse(dx, dy);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let engine = match &mut self.engine {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => engine.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let dt = self.last_time.elapsed();
                self.last_time = Instant::now();
                engine.update(dt);

                self.i += dt;
                if self.i.as_millis() > 200 {
                    self.polled_frametime = dt.as_millis() as u32;
                    self.i = std::time::Duration::ZERO;
                }

                match engine.render(self.polled_frametime) {
                    Ok(_) => {}
                    Err(e) => {
                        // Log the error and exit gracefully
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                engine.handle_mouse_button(button, state.is_pressed())
            }
            WindowEvent::MouseWheel { delta, .. } => engine.handle_mouse_scroll(&delta),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => engine.handle_key(event_loop, code, state.is_pressed()),
            _ => {}
        }
    }
}
