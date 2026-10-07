use ZenKitCAPI_sys::*;

/// Text encoding of strings stored inside game data files.
///
/// Gothic script-side strings (e.g. cutscene messages) are stored in a legacy
/// single-byte code page that depends on the game language edition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextEncoding {
    /// UTF-8, lossily decoded (default, matches the historical behavior).
    Utf8,
    /// Windows-1252, used by western European game editions.
    Windows1252,
    /// Windows-1250, used by central European game editions (e.g. Polish, Czech).
    Windows1250,
}

impl TextEncoding {
    pub(crate) fn encoding_rs(self) -> &'static encoding_rs::Encoding {
        match self {
            Self::Utf8 => encoding_rs::UTF_8,
            Self::Windows1252 => encoding_rs::WINDOWS_1252,
            Self::Windows1250 => encoding_rs::WINDOWS_1250,
        }
    }
}

// Maybe usage of std::time::SystemTime would be better?
#[derive(Debug, Clone, Copy)]
pub struct FileTime(i64);

#[derive(Debug, Clone, Copy)]
pub struct U8Vec4 {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub w: u8,
}
impl U8Vec4 {
    pub fn new(x: u8, y: u8, z: u8, w: u8) -> Self {
        Self { x, y, z, w }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl Vec3 {
    const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}
impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}
impl Quat {
    pub fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Mat3 {
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub z_axis: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct Mat4 {
    pub x_axis: Vec4,
    pub y_axis: Vec4,
    pub z_axis: Vec4,
    pub w_axis: Vec4,
}

impl FileTime {
    pub fn from(num: i64) -> Self {
        Self(num)
    }
    pub fn as_number(&self) -> i64 {
        self.0
    }
}

pub struct AxisAlignedBoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug, Clone)]
pub struct SoftSkinWeightEntry {
    pub weight: f32,
    pub position: Vec3,
    pub node_index: u8,
}

impl Default for SoftSkinWeightEntry {
    fn default() -> Self {
        SoftSkinWeightEntry {
            weight: 0.0,
            position: Vec3::ZERO,
            node_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnimationSample {
    pub position: Vec3,
    pub rotation: Quat,
}

pub type GameVersion = ZkGameVersion;
pub type FightAiMove = ZkFightAiMove;
pub type DamageType = ZkDamageType;
pub type NpcInstanceAttribute = ZkNpcInstanceAttribute;
pub type NpcInstanceTalent = ZkNpcInstanceTalent;
pub type NpcInstanceType = ZkNpcInstanceType;
pub type NpcInstanceFlag = ZkNpcInstanceFlag;
pub type NpcInstanceNameSlot = ZkNpcInstanceNameSlot;
pub type NpcInstanceMissionSlot = ZkNpcInstanceMissionSlot;
pub type ItemInstanceFlag = ZkItemInstanceFlag;
pub type ItemInstanceConditionSlot = ZkItemInstanceConditionSlot;
pub type ItemInstanceStateSlot = ZkItemInstanceStateSlot;
pub type ItemInstanceTextSlot = ZkItemInstanceTextSlot;
pub type MenuFlag = ZkMenuFlag;
pub type MenuItemType = ZkMenuItemType;
pub type MenuItemFlag = ZkMenuItemFlag;
pub type MusicTransitionEffect = ZkMusicTransitionEffect;
pub type MusicTransitionType = ZkMusicTransitionType;
pub type Whence = ZkWhence;
pub type MaterialGroup = ZkMaterialGroup;
pub type AnimationMapping = ZkAnimationMapping;
pub type WaveMode = ZkWaveMode;
pub type WaveSpeed = ZkWaveSpeed;
pub type AlphaFunction = ZkAlphaFunction;
pub type CameraMotion = ZkCameraMotion;
pub type CameraTrajectory = ZkCameraTrajectory;
pub type CameraLerpType = ZkCameraLerpType;
pub type CameraLoopType = ZkCameraLoopType;
pub type LightType = ZkLightType;
pub type LightQuality = ZkLightQuality;
pub type VobType = ZkVobType;
pub type SpriteAlignment = ZkSpriteAlignment;
pub type ShadowType = ZkShadowType;
pub type AnimationType = ZkAnimationType;
pub type VisualType = ZkVisualType;
pub type AiType = ZkAiType;
pub type MessageFilterAction = ZkMessageFilterAction;
pub type MoverMessageType = ZkMoverMessageType;
pub type TouchCollisionType = ZkTouchCollisionType;
pub type NpcNewsId = ZkNpcNewsId;
pub type NpcNewsSpread = ZkNpcNewsSpread;
pub type SoundMaterialType = ZkSoundMaterialType;
pub type SoundMode = ZkSoundMode;
pub type SoundTriggerVolumeType = ZkSoundTriggerVolumeType;
pub type VfsOverwriteBehavior = ZkVfsOverwriteBehavior;
pub type MoverBehavior = ZkMoverBehavior;
pub type MoverLerpType = ZkMoverLerpType;
pub type MoverSpeedType = ZkMoverSpeedType;
pub type TriggerBatchMode = ZkTriggerBatchMode;
pub type BspTreeType = ZkBspTreeType;
pub type ArchiveFormat = ZkArchiveFormat;
pub type DaedalusOpcode = ZkDaedalusOpcode;
pub type DaedalusDataType = ZkDaedalusDataType;
pub type DaedalusInstanceType = ZkDaedalusInstanceType;
pub type LogLevel = ZkLogLevel;
pub type TextureFormat = ZkTextureFormat;
pub type AnimationFlag = ZkAnimationFlag;
pub type AnimationDirection = ZkAnimationDirection;
pub type EventType = ZkEventType;
pub type FightMode = ZkFightMode;
pub type SaveTopicSection = ZkSaveTopicSection;
pub type SaveTopicStatus = ZkSaveTopicStatus;

// #[repr(u32)]
// pub type meVersion = GameVersion;
//     GOTHIC1 = ZkGameVersion::ZkGameVersion_GOTHIC1 as u32,
//     GOTHIC2 = ZkGameVersion::ZkGameVersion_GOTHIC2 as u32,
// }

// impl GameVersion {
//     pub fn to_raw(&self) -> ZkGameVersion {
//         match *self {
//             GameVersion::GOTHIC1 => ZkGameVersion::ZkGameVersion_GOTHIC1,
//             GameVersion::GOTHIC2 => ZkGameVersion::ZkGameVersion_GOTHIC2,
//         }
//     }
// }
