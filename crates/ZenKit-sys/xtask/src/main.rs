use std::path::PathBuf;

fn main() {
    let header = "../wrapper.h";

    let bindings = bindgen::Builder::default()
        .header(header)
        // Don't bind libc types
        .blocklist_item("_.*")
        .blocklist_item("FILE")
        .blocklist_item("time_t")
        // Don't bint functions which use libc types
        .blocklist_item("ZkRead_newFile")
        .blocklist_item("ZkWrite_newFile")
        .allowlist_item(".*Zk.*")
        // Fix exceptions
        .raw_line("#![allow(deprecated)]")
        .raw_line("use std::os::unix::raw::time_t;")
        // Improve enums
        .rustified_enum(".*")
        // Add Includes
        .clang_arg("-I../vendor/ZenKitCAPI/include")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from("../src/bindings.rs");
    bindings
        .write_to_file(&out_path)
        .expect("Couldn't write bindings!");

    // Make enum names shorter
    let mut bindings_text =
        std::fs::read_to_string(&out_path).expect("Should be able to read generated bindings");
    let remove_enum_prefixes = &[
        "ZkGameVersion_",
        "ZkFightAiMove_",
        "ZkDamageType_",
        "ZkNpcAttribute_",
        "ZkNpcTalent_",
        "ZkNpcType_",
        "ZkNpcFlag_",
        "ZkNpcName",
        "ZkNpcMission",
        "ZkItemInstanceFlag_",
        "ZkItemInstanceConditionSlot_",
        "ZkItemInstanceState_",
        "ZkItemInstanceText_",
        "ZkMenuFlag_",
        "ZkMenuItemType_",
        "ZkMenuItemFlag_",
        "ZkMusicTransitionEffect_",
        "ZkMusicTransitionType_",
        "ZkWhence_",
        "ZkMaterialGroup_",
        "ZkAnimationMapping_",
        "ZkWaveMode_",
        "ZkWaveSpeed_",
        "ZkAlphaFunction_",
        "ZkCameraMotion_",
        "ZkCameraTrajectory_",
        "ZkCameraLerpType_",
        "ZkCameraLoop_",
        "ZkLightType_",
        "ZkLightQuality_",
        "ZkVobType_",
        "ZkSpriteAlignment_",
        "ZkShadowType_",
        "ZkAnimationType_",
        "ZkVisualType_",
        "ZkAiType_",
        "ZkMessageFilterAction_",
        "ZkMoverMessageType_",
        "ZkTouchCollisionType_",
        "ZkNpcNewsId_",
        "ZkNpcNewsSpread_",
        "ZkSoundMaterialType_",
        "ZkSoundMode_",
        "SoundTriggerVolumeType_",
        "ZkVfsOverwriteBehavior_",
        "ZkMoverBehavior_",
        "ZkMoverLerpType_",
        "ZkMoverSpeedType_",
        "ZkTriggerBatchMode_",
        "ZkBspTreeType_",
        "ZkArchiveFormat_",
        "ZkDaedalusOpcode_",
        "ZkDaedalusDataType_",
        "ZkDaedalusInstanceType_",
        "ZkLogLevel_",
        "ZkTextureFormat_",
        "ZkAnimationFlag_",
        "ZkAnimationDirection_",
        "ZkEventType_",
        "ZkFightMode_",
        "ZkSaveTopicSection_",
        "ZkSaveTopicStatus_",
    ];
    for prefix in remove_enum_prefixes {
        bindings_text = bindings_text.replace(prefix, "");
    }
    std::fs::write(&out_path, &bindings_text)
        .expect("Couldn't write bindings with additional changes");

    println!("Bindings successfully generated at src/bindings.rs");
}
