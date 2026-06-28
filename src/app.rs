mod engine;
use std::{collections::VecDeque, sync::Arc};

use engine::Engine;

use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    keyboard::KeyCode, window::Window,
};

const DEQUE_SIZE: usize = 1000;

pub struct App {
    helper: winit_input_helper::WinitInputHelper,
    engine: Option<Engine>,
    frametimes: VecDeque<f32>,
    frametimes_scratch: Vec<f32>,
}

impl App {
    pub fn new() -> Self {
        Self {
            helper: winit_input_helper::WinitInputHelper::new(),
            engine: None,
            frametimes: VecDeque::with_capacity(DEQUE_SIZE + 1),
            frametimes_scratch: Vec::with_capacity(DEQUE_SIZE),
        }
    }
}

impl ApplicationHandler<Engine> for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.helper.step();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.engine = Some(pollster::block_on(Engine::new(window)).unwrap());
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.helper.process_device_event(&event);
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

        if let WindowEvent::Resized(size) = event {
            engine.resize(size.width, size.height)
        }
        if self.helper.process_window_event(&event) {
            engine.update(*self.frametimes.back().unwrap_or(&0f32));
            let avg = if self.frametimes.is_empty() {
                0.0
            } else {
                1000.0 * self.frametimes.iter().sum::<f32>() / self.frametimes.len() as f32
            };
            match engine.render(
                avg,
                percentile_low(&self.frametimes, &mut self.frametimes_scratch, 1.0),
            ) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("{e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let engine = match &mut self.engine {
            Some(canvas) => canvas,
            None => return,
        };

        if self.helper.close_requested() {
            event_loop.exit();
        }

        if self.helper.mouse_held(winit::event::MouseButton::Right) {
            engine
                .camera_controller
                .handle_mouse(self.helper.mouse_diff());
        }

        engine.camera_controller.process_keyboard(&self.helper);
        if self.helper.key_pressed(KeyCode::Escape) {
            event_loop.exit();
        }

        self.helper.end_step();

        self.frametimes
            .push_back(self.helper.delta_time().unwrap_or_default().as_secs_f32());
        if self.frametimes.len() > DEQUE_SIZE {
            self.frametimes.pop_front();
        }
    }
}

fn percentile_low(frametimes: &VecDeque<f32>, scratch: &mut Vec<f32>, percent: f32) -> f32 {
    if frametimes.is_empty() {
        return 0.0;
    }
    scratch.clear();
    scratch.extend(frametimes.iter().copied());
    let count = ((frametimes.len() as f32) * percent / 100.0).ceil() as usize;
    let count = count.max(1).min(scratch.len());

    scratch.select_nth_unstable_by(count - 1, |a, b| b.partial_cmp(a).unwrap());
    scratch[..count].iter().sum::<f32>() / count as f32
}
