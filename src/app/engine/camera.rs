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
