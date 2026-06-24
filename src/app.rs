mod engine;
use std::{collections::VecDeque, sync::Arc, time::Instant};

use engine::Engine;

use rayon::prelude::*;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::PhysicalKey,
    window::Window,
};

const DEQUE_SIZE: usize = 1000;

pub struct App {
    engine: Option<Engine>,
    last_time: Instant,
    frametimes: VecDeque<f32>,
}

impl App {
    pub fn new() -> Self {
        Self {
            engine: None,
            last_time: Instant::now(),
            frametimes: VecDeque::with_capacity(DEQUE_SIZE + 1),
        }
    }
}

impl ApplicationHandler<Engine> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
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

        engine.handle_egui_input(&event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => engine.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                self.frametimes
                    .push_back(self.last_time.elapsed().as_secs_f32());
                if self.frametimes.len() > DEQUE_SIZE {
                    self.frametimes.pop_front();
                }
                self.last_time = Instant::now();

                engine.update(*self.frametimes.back().unwrap_or(&0f32));

                match engine.render(
                    1000.0 * self.frametimes.par_iter().sum::<f32>() / self.frametimes.len() as f32,
                    percentile_low(&self.frametimes, 1.0),
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                engine.handle_mouse_button(button, state.is_pressed())
            }
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

fn percentile_low(frametimes: &VecDeque<f32>, percent: f32) -> f32 {
    let count = ((frametimes.len() as f32) * percent / 100.0).ceil() as usize;
    let count = count.max(1);
    let mut sorted: Vec<f32> = frametimes.clone().into();
    sorted.par_sort_unstable_by(|a, b| b.partial_cmp(a).unwrap());

    sorted[..count].par_iter().sum::<f32>() / count as f32
}
