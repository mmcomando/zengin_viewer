// TODO move to loaders module
use bevy::math::*;

pub(crate) trait ZenginVec2ToGlam {
    fn to_glam(self) -> Vec2;
}
impl ZenginVec2ToGlam for zen_kit_rs::misc::Vec2 {
    fn to_glam(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

pub(crate) trait ZenginVec3ToGlam {
    fn to_glam(self) -> Vec3;
}
impl ZenginVec3ToGlam for zen_kit_rs::misc::Vec3 {
    fn to_glam(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

pub(crate) trait ZenginVec4ToGlam {
    fn to_glam(self) -> Vec4;
}
impl ZenginVec4ToGlam for zen_kit_rs::misc::Vec4 {
    fn to_glam(self) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, self.w)
    }
}

pub(crate) trait ZenginQuatToGlam {
    fn to_glam(self) -> Quat;
}
impl ZenginQuatToGlam for zen_kit_rs::misc::Quat {
    fn to_glam(self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }
}

pub(crate) trait ZenginMat3ToGlam {
    fn to_glam(self) -> Mat3;
}
impl ZenginMat3ToGlam for zen_kit_rs::misc::Mat3 {
    fn to_glam(self) -> Mat3 {
        Mat3::from_cols(
            self.x_axis.to_glam(),
            self.y_axis.to_glam(),
            self.z_axis.to_glam(),
        )
    }
}

pub(crate) trait ZenginMat4ToGlam {
    fn to_glam(self) -> Mat4;
}
impl ZenginMat4ToGlam for zen_kit_rs::misc::Mat4 {
    fn to_glam(self) -> Mat4 {
        Mat4::from_cols(
            self.x_axis.to_glam(),
            self.y_axis.to_glam(),
            self.z_axis.to_glam(),
            self.w_axis.to_glam(),
        )
    }
}
