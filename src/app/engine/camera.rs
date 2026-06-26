use nalgebra::{Matrix4, Point3, Vector3};
use winit::keyboard::KeyCode;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: Matrix4<f32>,
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Matrix4::identity(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera, projection: &Projection) {
        self.view_proj = projection.calc_matrix() * camera.calc_matrix();
    }
}

pub struct Camera {
    pub pos: Point3<f32>,
    pub yaw: f32,
    pub pitch: f32,
}

const VERTICAL_CLAMP: f32 = 80f32.to_radians();

impl Camera {
    pub fn new(pos: Point3<f32>, yaw: f32, pitch: f32) -> Self {
        Self {
            pos,
            yaw: yaw.to_radians(),
            pitch: pitch.to_radians(),
        }
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        let direction =
            Vector3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize();

        Matrix4::look_at_rh(&self.pos, &(self.pos + direction), &Vector3::y())
    }
}

pub struct Projection {
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Projection {
    pub fn new(width: u32, height: u32, fovy: f32, znear: f32, zfar: f32) -> Self {
        Self {
            aspect: width as f32 / height as f32,
            fovy: fovy.to_radians(),
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        Matrix4::new_perspective(self.aspect, self.fovy, self.znear, self.zfar)
    }
}

pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    multiplier: f32,
    speed: f32,
    sensitivity: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            multiplier: 1.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, input: &winit_input_helper::WinitInputHelper) {
        let held = |key| input.key_held(key) as i32 as f32;

        self.amount_forward = held(KeyCode::KeyW);
        self.amount_backward = held(KeyCode::KeyS);
        self.amount_left = held(KeyCode::KeyA);
        self.amount_right = held(KeyCode::KeyD);
        self.amount_up = held(KeyCode::Space);
        self.amount_down = held(KeyCode::ControlLeft);
        self.multiplier = held(KeyCode::ShiftLeft) + 1.0;
    }

    pub fn handle_mouse(&mut self, mouse_delta: (f32, f32)) {
        self.rotate_horizontal = mouse_delta.0;
        self.rotate_vertical = mouse_delta.1;
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let pitch_sin = camera.pitch.sin();

        let forward = Vector3::new(yaw_cos, pitch_sin, yaw_sin).normalize();
        let right = Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();

        camera.pos += forward
            * (self.amount_forward - self.amount_backward)
            * self.speed
            * self.multiplier
            * dt;
        camera.pos +=
            right * (self.amount_right - self.amount_left) * self.speed * self.multiplier * dt;

        camera.pos.y += (self.amount_up - self.amount_down) * self.speed * self.multiplier * dt;

        camera.yaw += self.rotate_horizontal.to_radians() * self.sensitivity * dt;
        camera.pitch += -self.rotate_vertical.to_radians() * self.sensitivity * dt;

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        camera.pitch = camera.pitch.clamp(-VERTICAL_CLAMP, VERTICAL_CLAMP);
    }
}
