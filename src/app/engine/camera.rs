use na::{Matrix4, Point3, Vector3};
use nalgebra::{self as na, Perspective3, Vector2};
use wgpu::util::DeviceExt;
use winit::keyboard::KeyCode;

pub struct Camera {
    position: Point3<f32>,
    yaw: f32,
    pitch: f32,
    projection: Perspective3<f32>,
}

const VERTICAL_CLAMP: f32 = 80f32.to_radians();

impl Camera {
    pub fn resize(&mut self, width: u32, height: u32) {
        self.projection.set_aspect(width as f32 / height as f32);
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        let direction =
            Vector3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize();

        self.projection.as_matrix()
            * Matrix4::look_at_rh(&self.position, &(self.position + direction), &Vector3::y())
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
            camera.position += dir * self.speed * self.multiplier * dt;
        }

        camera.yaw += self.rotation_delta.x.to_radians() * self.sensitivity * dt;
        camera.pitch += -self.rotation_delta.y.to_radians() * self.sensitivity * dt;

        self.rotation_delta = Vector2::zeros();

        camera.pitch = camera.pitch.clamp(-VERTICAL_CLAMP, VERTICAL_CLAMP);
    }
}

pub struct Builder {
    position: Point3<f32>,
    yaw: f32,
    pitch: f32,
    width: u32,
    height: u32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            position: Point3::origin(),
            yaw: 0.0,
            pitch: 0.0,
            width: 0,
            height: 0,
            fovy: 0.0,
            znear: 0.0,
            zfar: 0.0,
        }
    }

    pub fn position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = Point3::new(x, y, z);

        self
    }

    pub fn rotation(mut self, yaw: f32, pitch: f32) -> Self {
        self.yaw = yaw.to_radians();
        self.pitch = pitch.to_radians();

        self
    }

    pub fn perspective(
        mut self,
        width: u32,
        height: u32,
        fovy: f32,
        znear: f32,
        zfar: f32,
    ) -> Self {
        self.width = width;
        self.height = height;
        self.fovy = fovy;
        self.znear = znear;
        self.zfar = zfar;

        self
    }

    pub fn build(self) -> Camera {
        Camera {
            position: self.position,
            yaw: self.yaw,
            pitch: self.pitch,
            projection: Perspective3::new(
                self.width as f32 / self.height as f32,
                self.fovy,
                self.znear,
                self.zfar,
            ),
        }
    }
}

pub struct CameraGPU {
    buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl CameraGPU {
    pub fn new(device: &wgpu::Device, camera: &Camera) -> (Self, wgpu::BindGroupLayout) {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera.calc_matrix()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        (Self { buffer, bind_group }, bind_group_layout)
    }

    pub fn update_buffer(&mut self, queue: &wgpu::Queue, camera: &Camera) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[camera.calc_matrix()]),
        );
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Plane {
    normal: Vector4<f32>,
}

impl Plane {
    fn from_column_matrix(column: Matrix4x1<f32>) -> Plane {
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
    pub(crate) fn create_from_camera_projection(&mut self, view_proj: &Matrix4<f32>) {
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
        &mut self,
        camera_frustum: &Frustum,
        transform: &crate::app::engine::InstanceRaw,
    ) -> bool;

    fn is_in_plane(&self, plane: &Plane) -> bool;
}

pub struct SphereVolume {
    center: Vector3<f32>,
    radius: f32,
}

impl SphereVolume {
    pub fn new(center: Vector3<f32>, radius: f32) -> Self {
        Self { center, radius }
    }
}

impl BoundingVolume for SphereVolume {
    fn is_in_frustum(
        &mut self,
        camera_frustum: &Frustum,
        transform: &crate::app::engine::InstanceRaw,
    ) -> bool {
        let global_center = (transform.model * self.center.push(1.0)).xyz();

        self.center = global_center;

        self.is_in_plane(&camera_frustum.near_plane)
            && self.is_in_plane(&camera_frustum.far_plane)
            && self.is_in_plane(&camera_frustum.left_plane)
            && self.is_in_plane(&camera_frustum.right_plane)
            && self.is_in_plane(&camera_frustum.top_plane)
            && self.is_in_plane(&camera_frustum.bottom_plane)
    }

    fn is_in_plane(&self, plane: &Plane) -> bool {
        plane.normal.dot(&self.center.push(1.0)) + plane.normal.w > -self.radius
    }
}

pub struct CustomAABB {
    center: Vector3<f32>,
    extents: Vector3<f32>,
}

impl CustomAABB {
    pub fn new(center: Vector3<f32>, extents: Vector3<f32>) -> Self {
        Self { center, extents }
    }

    #[allow(unused)]
    fn new_min_max(min: &Vector3<f32>, max: &Vector3<f32>) -> Self {
        let center = (min + max) * 0.5;
        Self {
            center,
            extents: max - center,
        }
    }
}

impl BoundingVolume for CustomAABB {
    fn is_in_frustum(
        &mut self,
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

        self.center = global_center;
        self.extents = Vector3::new(new_i, new_j, new_k);

        self.is_in_plane(&camera_frustum.near_plane)
            && self.is_in_plane(&camera_frustum.far_plane)
            && self.is_in_plane(&camera_frustum.left_plane)
            && self.is_in_plane(&camera_frustum.right_plane)
            && self.is_in_plane(&camera_frustum.top_plane)
            && self.is_in_plane(&camera_frustum.bottom_plane)
    }

    fn is_in_plane(&self, plane: &Plane) -> bool {
        let projected_radius = self.extents.cross(&plane.normal.xyz()).abs().sum();
        let signed_distance = plane.normal.dot(&self.center.push(1.0)) + plane.normal.w;

        -projected_radius < signed_distance
    }
}
