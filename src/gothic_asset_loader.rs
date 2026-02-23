use bevy::asset::io::AssetSourceBuilder;

pub fn create_gothic_asset_loader() -> AssetSourceBuilder {
    AssetSourceBuilder::platform_default("assets", None)
}
