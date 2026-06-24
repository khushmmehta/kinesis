use nalgebra::{Matrix4, Point3, Vector3, Vector4};
use winit::{dpi::PhysicalPosition, event::MouseScrollDelta, keyboard::KeyCode};

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
    scroll: f32,
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
            scroll: 0.0,
            multiplier: 1.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, key: KeyCode, pressed: bool) -> bool {
        let amount = if pressed { 1.0 } else { 0.0 };

        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.amount_forward = amount;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.amount_backward = amount;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.amount_left = amount;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.amount_right = amount;
                true
            }
            KeyCode::Space => {
                self.amount_up = amount;
                true
            }
            KeyCode::ControlLeft => {
                self.amount_down = amount;
                true
            }
            KeyCode::ShiftLeft => {
                self.multiplier = amount + 1.0;
                true
            }
            _ => false,
        }
    }

    pub fn handle_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn handle_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            MouseScrollDelta::LineDelta(_, scroll) => -scroll * 0.5,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => -*scroll as f32,
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = camera.pitch.sin_cos();

        let forward = Vector3::new(yaw_cos, pitch_sin, yaw_sin).normalize();
        let right = Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();

        camera.pos += forward
            * (self.amount_forward - self.amount_backward)
            * self.speed
            * self.multiplier
            * dt;
        camera.pos +=
            right * (self.amount_right - self.amount_left) * self.speed * self.multiplier * dt;

        let scrollward =
            Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        camera.pos += scrollward * self.scroll * self.speed * self.sensitivity * dt;
        self.scroll = 0.0;

        camera.pos.y += (self.amount_up - self.amount_down) * self.speed * self.multiplier * dt;

        camera.yaw += self.rotate_horizontal.to_radians() * self.sensitivity * dt;
        camera.pitch += -self.rotate_vertical.to_radians() * self.sensitivity * dt;

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        camera.pitch = camera.pitch.clamp(-VERTICAL_CLAMP, VERTICAL_CLAMP);
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Plane {
    normal: nalgebra::Vector4<f32>,
}

impl Plane {
    fn from_column_matrix(column: nalgebra::Matrix4x1<f32>) -> Plane {
        Plane {
            normal: column / column.xyz().norm(),
        }
    }

    fn from_column_view(column: nalgebra::MatrixView4x1<f32>) -> Plane {
        Plane {
            normal: column / column.xyz().norm(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Frustum {
    near_plane: Plane,
    far_plane: Plane,
    top_plane: Plane,
    bottom_plane: Plane,
    right_plane: Plane,
    left_plane: Plane,
}

impl Frustum {
    pub(crate) fn create_from_camera_projection(&mut self, view_proj: &nalgebra::Matrix4<f32>) {
        let c1 = view_proj.row(0).transpose();
        let c2 = view_proj.row(1).transpose();
        let c3 = view_proj.row(2).transpose();
        let c4 = view_proj.row(3).transpose();

        self.near_plane = Plane::from_column_matrix(c4 + c3);
        self.far_plane = Plane::from_column_matrix(c4 - c3);
        self.top_plane = Plane::from_column_matrix(c4 - c2);
        self.bottom_plane = Plane::from_column_matrix(c4 + c2);
        self.right_plane = Plane::from_column_matrix(c4 - c1);
        self.left_plane = Plane::from_column_matrix(c4 + c1);
    }
}

pub trait BoundingVolume {
    fn is_in_frustum(
        &self,
        camera_frustum: &Frustum,
        transform: &crate::app::engine::InstanceRaw,
    ) -> bool;

    fn is_in_plane(&self, plane: &Plane) -> bool;
}

pub struct CustomAABB {
    center: nalgebra::Vector3<f32>,
    extents: nalgebra::Vector3<f32>,
}

impl CustomAABB {
    pub fn new(center: nalgebra::Vector3<f32>, extents: nalgebra::Vector3<f32>) -> Self {
        Self { center, extents }
    }

    #[allow(unused)]
    fn new_min_max(min: &nalgebra::Vector3<f32>, max: &nalgebra::Vector3<f32>) -> Self {
        let center = (min + max) * 0.5;
        Self {
            center,
            extents: max - center,
        }
    }
}

impl BoundingVolume for CustomAABB {
    fn is_in_frustum(
        &self,
        camera_frustum: &Frustum,
        transform: &crate::app::engine::InstanceRaw,
    ) -> bool {
        let global_center = (transform.model * self.center.push(1.0)).xyz();

        let right = transform.model.column(0).xyz() * self.extents.x;
        let up = transform.model.column(1).xyz() * self.extents.y;
        let forward = transform.model.column(2).xyz() * self.extents.z;

        let new_i = right.x.abs() + up.x.abs() + forward.x.abs();
        let new_j = right.y.abs() + up.y.abs() + forward.y.abs();
        let new_k = right.z.abs() + up.z.abs() + forward.z.abs();

        let transformed = CustomAABB {
            center: global_center,
            extents: nalgebra::Vector3::new(new_i, new_j, new_k),
        };

        transformed.is_in_plane(&camera_frustum.near_plane)
            && transformed.is_in_plane(&camera_frustum.far_plane)
            && transformed.is_in_plane(&camera_frustum.left_plane)
            && transformed.is_in_plane(&camera_frustum.right_plane)
            && transformed.is_in_plane(&camera_frustum.top_plane)
            && transformed.is_in_plane(&camera_frustum.bottom_plane)
    }

    fn is_in_plane(&self, plane: &Plane) -> bool {
        let projected_radius = self.extents.cross(&plane.normal.xyz()).abs().sum();
        let signed_distance = plane.normal.dot(&self.center.push(1.0)) + plane.normal.w;

        -projected_radius < signed_distance
    }
}
