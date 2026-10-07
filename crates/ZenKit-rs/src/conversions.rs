use crate::misc::{Mat3, Mat4, Quat, U8Vec4, Vec2, Vec3, Vec4};
use ZenKitCAPI_sys::*;
use std::{
    ffi::{CStr, CString},
    str::FromStr,
};

use crate::misc::{AnimationSample, AxisAlignedBoundingBox, SoftSkinWeightEntry, TextEncoding};

#[must_use]
pub fn str_to_zk(slice: &str) -> CString {
    CString::from_str(slice).expect("Provided str should be convertible to CString")
}

pub(crate) trait ZkStringToString {
    fn to_string(self) -> String;
    fn to_string_with_encoding(self, encoding: TextEncoding) -> String;
}
impl ZkStringToString for ZkString {
    fn to_string(self) -> String {
        self.to_string_with_encoding(TextEncoding::Utf8)
    }
    fn to_string_with_encoding(self, encoding: TextEncoding) -> String {
        let bytes = unsafe { CStr::from_ptr(self) }.to_bytes();

        // Plain lossy UTF-8 keeps behavior identical to CStr::to_string_lossy
        // (encoding_rs' decode() would additionally strip a UTF-8 BOM).
        if encoding == TextEncoding::Utf8 {
            return String::from_utf8_lossy(bytes).into_owned();
        }

        let (decoded, _, _) = encoding.encoding_rs().decode(bytes);
        decoded.into_owned()
    }
}

pub(crate) trait ZkBoolToBool {
    fn to_bool(self) -> bool;
}
impl ZkBoolToBool for ZkBool {
    fn to_bool(self) -> bool {
        self != 0
    }
}

pub(crate) trait Vec2ToOwn {
    fn to_own(self) -> Vec2;
}
impl Vec2ToOwn for ZkVec2f {
    fn to_own(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

pub(crate) trait Vec3ToOwn {
    fn to_own(self) -> Vec3;
}
impl Vec3ToOwn for ZkVec3f {
    fn to_own(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

pub(crate) trait Vec4ToOwn {
    fn to_own(self) -> Vec4;
}
impl Vec4ToOwn for ZkVec4f {
    fn to_own(self) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, self.w)
    }
}

pub(crate) trait QuatToOwn {
    fn to_own(self) -> Quat;
}
impl QuatToOwn for ZkQuat {
    fn to_own(self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }
}

pub(crate) trait Mat3x3ToOwn {
    fn to_own(self) -> Mat3;
}
impl Mat3x3ToOwn for ZkMat3x3 {
    fn to_own(self) -> Mat3 {
        Mat3 {
            x_axis: self.columns[0].to_own(),
            y_axis: self.columns[1].to_own(),
            z_axis: self.columns[2].to_own(),
        }
    }
}

pub(crate) trait Mat4x4ToOwn {
    fn to_own(self) -> Mat4;
}
impl Mat4x4ToOwn for ZkMat4x4 {
    fn to_own(self) -> Mat4 {
        Mat4 {
            x_axis: self.columns[0].to_own(),
            y_axis: self.columns[1].to_own(),
            z_axis: self.columns[2].to_own(),
            w_axis: self.columns[3].to_own(),
        }
    }
}

pub(crate) trait ColorToOwn {
    fn to_own(self) -> U8Vec4;
}
impl ColorToOwn for ZkColor {
    fn to_own(self) -> U8Vec4 {
        U8Vec4::new(self.r, self.g, self.b, self.a)
    }
}

pub(crate) trait AxisAlignedBoundingBoxToOwn {
    fn to_own(self) -> AxisAlignedBoundingBox;
}
impl AxisAlignedBoundingBoxToOwn for ZkAxisAlignedBoundingBox {
    fn to_own(self) -> AxisAlignedBoundingBox {
        AxisAlignedBoundingBox {
            min: self.min.to_own(),
            max: self.max.to_own(),
        }
    }
}

pub(crate) trait SoftSkinWeightEntryToOwn {
    fn to_own(self) -> SoftSkinWeightEntry;
}

impl SoftSkinWeightEntryToOwn for ZkSoftSkinWeightEntry {
    fn to_own(self) -> SoftSkinWeightEntry {
        SoftSkinWeightEntry {
            weight: self.weight,
            position: self.position.to_own(),
            node_index: self.node_index,
        }
    }
}

pub(crate) trait AnimationSampleToOwn {
    fn to_own(self) -> AnimationSample;
}

impl AnimationSampleToOwn for ZkAnimationSample {
    fn to_own(self) -> AnimationSample {
        AnimationSample {
            position: self.position.to_own(),
            rotation: self.rotation.to_own(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::TextEncoding;

    #[test]
    fn decodes_windows_1250() {
        // 0xB3 is 'ł' in windows-1250
        let raw = CString::new(vec![0x44u8, 0xB3, 0x77]).unwrap();
        assert_eq!(
            raw.as_ptr()
                .to_string_with_encoding(TextEncoding::Windows1250),
            "Dłw"
        );
    }

    #[test]
    fn decodes_windows_1252() {
        // 0xE9 is 'é' in windows-1252
        let raw = CString::new(vec![0x43u8, 0xE9, 0x6C]).unwrap();
        assert_eq!(
            raw.as_ptr()
                .to_string_with_encoding(TextEncoding::Windows1252),
            "Cél"
        );
    }

    #[test]
    fn decodes_utf8_lossily() {
        // 0xF3 is a lone invalid UTF-8 continuation byte
        let raw = CString::new(vec![0x44u8, 0x77, 0xF3]).unwrap();
        assert_eq!(
            raw.as_ptr().to_string_with_encoding(TextEncoding::Utf8),
            "Dw\u{FFFD}"
        );
    }
}
