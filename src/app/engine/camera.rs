use na::{Matrix4, Point3, Vector3};
use nalgebra::{self as na, Vector2};
use winit::keyboard::KeyCode;

pub struct Camera {
    pub pos: Point3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

const VERTICAL_CLAMP: f32 = 80f32.to_radians();

impl Camera {
    pub fn new(
        pos: Point3<f32>,
        yaw: f32,
        pitch: f32,
        aspect: f32,
        fovy: f32,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            pos,
            yaw: yaw.to_radians(),
            pitch: pitch.to_radians(),
            aspect,
            fovy,
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        let direction =
            Vector3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize();

        Matrix4::new_perspective(self.aspect, self.fovy, self.znear, self.zfar)
            * Matrix4::look_at_rh(&self.pos, &(self.pos + direction), &Vector3::y())
    }
}

pub struct CameraController {
    amount: Vector3<f32>,
    multiplier: f32,
    rotation_delta: Vector2<f32>,
    speed: f32,
    sensitivity: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount: Vector3::zeros(),
            multiplier: 0.0,
            rotation_delta: Vector2::zeros(),
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, input: &winit_input_helper::WinitInputHelper) {
        let held = |key| input.key_held(key) as i32 as f32;

        self.multiplier = held(KeyCode::ShiftLeft) + 1.0;
        self.amount = Vector3::new(
            held(KeyCode::KeyD) - held(KeyCode::KeyA),
            held(KeyCode::Space) - held(KeyCode::ControlLeft),
            held(KeyCode::KeyW) - held(KeyCode::KeyS),
        );
    }

    pub fn handle_mouse(&mut self, mouse_delta: (f32, f32)) {
        self.rotation_delta = Vector2::new(mouse_delta.0, mouse_delta.1);
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let pitch_sin = camera.pitch.sin();

        let pos_delta = Vector3::new(
            (yaw_cos * self.amount.z) - (yaw_sin * self.amount.x),
            pitch_sin * self.amount.z + self.amount.y,
            (yaw_sin * self.amount.z) + (yaw_cos * self.amount.x),
        );

        if let Some(dir) = pos_delta.try_normalize(0.001) {
            camera.pos += dir * self.speed * self.multiplier * dt;
        }

        camera.yaw += self.rotation_delta.x.to_radians() * self.sensitivity * dt;
        camera.pitch += -self.rotation_delta.y.to_radians() * self.sensitivity * dt;

        self.rotation_delta = Vector2::zeros();

        camera.pitch = camera.pitch.clamp(-VERTICAL_CLAMP, VERTICAL_CLAMP);
    }
}
