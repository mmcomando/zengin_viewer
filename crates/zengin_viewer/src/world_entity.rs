use bevy::prelude::Component;

/// Entity spawned from the loaded world data or its scripts.
///
/// GUI, camera and other application-lifetime entities are not tagged, so
/// despawning this component's entities keeps them alive across world restarts.
#[derive(Component, Default)]
pub(crate) struct WorldEntityMarker;
