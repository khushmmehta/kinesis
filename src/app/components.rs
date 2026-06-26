use nalgebra::{Matrix4, Point3, Scale3, UnitQuaternion};

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    position: Point3<f32>,
    rotation: UnitQuaternion<f32>,
    scale: Scale3<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Point3::origin(),
            rotation: UnitQuaternion::identity(),
            scale: Scale3::identity(),
        }
    }
}

#[allow(unused)]
impl Transform {
    pub fn as_matrix(&self) -> Matrix4<f32> {
        let mut mat = self.rotation.to_homogeneous();
        mat.fixed_view_mut::<3, 1>(0, 0)
            .component_mul_assign(&self.scale.vector.xxx());
        mat.fixed_view_mut::<3, 1>(0, 1)
            .component_mul_assign(&self.scale.vector.yyy());
        mat.fixed_view_mut::<3, 1>(0, 2)
            .component_mul_assign(&self.scale.vector.zzz());
        mat.fixed_view_mut::<3, 1>(0, 3)
            .copy_from(&self.position.coords);

        mat
    }
}
