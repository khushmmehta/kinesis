use na::{Matrix4, Point3, Vector3};
use nalgebra as na;
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
    amount_x: f32,
    amount_y: f32,
    amount_z: f32,
    multiplier: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    speed: f32,
    sensitivity: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_x: 0.0,
            amount_y: 0.0,
            amount_z: 0.0,
            multiplier: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, input: &winit_input_helper::WinitInputHelper) {
        let held = |key| input.key_held(key) as i32 as f32;

        self.multiplier = held(KeyCode::ShiftLeft) + 1.0;
        self.amount_x = held(KeyCode::KeyD) - held(KeyCode::KeyA);
        self.amount_y = held(KeyCode::Space) - held(KeyCode::ControlLeft);
        self.amount_z = held(KeyCode::KeyW) - held(KeyCode::KeyS);
    }

    pub fn handle_mouse(&mut self, mouse_delta: (f32, f32)) {
        self.rotate_horizontal = mouse_delta.0;
        self.rotate_vertical = mouse_delta.1;
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let pitch_sin = camera.pitch.sin();

        let pos_delta = Vector3::new(
            (yaw_cos * self.amount_z) - (yaw_sin * self.amount_x),
            pitch_sin * self.amount_z + self.amount_y,
            (yaw_sin * self.amount_z) + (yaw_cos * self.amount_x),
        );

        if let Some(dir) = pos_delta.try_normalize(0.001) {
            camera.pos += dir * self.speed * self.multiplier * dt;
        }

        camera.yaw += self.rotate_horizontal.to_radians() * self.sensitivity * dt;
        camera.pitch += -self.rotate_vertical.to_radians() * self.sensitivity * dt;

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        camera.pitch = camera.pitch.clamp(-VERTICAL_CLAMP, VERTICAL_CLAMP);
    }
}
