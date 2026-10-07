#include <stdint.h>

// daedalus
#include "zenkit-capi/daedalus/CameraInstance.h"
#include "zenkit-capi/daedalus/EffectBaseInstance.hh"
#include "zenkit-capi/daedalus/FightAiInstance.hh"
#include "zenkit-capi/daedalus/FocusInstance.h"
#include "zenkit-capi/daedalus/GuildValuesInstance.h"
#include "zenkit-capi/daedalus/InfoInstance.h"
#include "zenkit-capi/daedalus/ItemInstance.h"
#include "zenkit-capi/daedalus/ItemReactInstance.h"
#include "zenkit-capi/daedalus/MenuInstance.h"
#include "zenkit-capi/daedalus/MenuItemInstance.h"
#include "zenkit-capi/daedalus/MissionInstance.h"
#include "zenkit-capi/daedalus/MusicJingleInstance.hh"
#include "zenkit-capi/daedalus/MusicSystemInstance.hh"
#include "zenkit-capi/daedalus/MusicThemeInstance.hh"
#include "zenkit-capi/daedalus/NpcInstance.h"
#include "zenkit-capi/daedalus/ParticleEffectEmitKeyInstance.hh"
#include "zenkit-capi/daedalus/ParticleEffectInstance.hh"
#include "zenkit-capi/daedalus/SoundEffectInstance.hh"
#include "zenkit-capi/daedalus/SoundSystemInstance.hh"
#include "zenkit-capi/daedalus/SpellInstance.h"
#include "zenkit-capi/daedalus/SvmInstance.hh"

// vobs
#include "zenkit-capi/vobs/Camera.h"
#include "zenkit-capi/vobs/Light.h"
#include "zenkit-capi/vobs/Misc.h"
#include "zenkit-capi/vobs/MovableObject.h"
#include "zenkit-capi/vobs/Sound.h"

// C bindings seem broken, definition of few types is missing
typedef struct ZkTriggerChangeLevel ZkTriggerChangeLevel;
typedef struct ZkTriggerListTarget ZkTriggerListTarget;
typedef struct ZkTriggerScript ZkTriggerScript;
typedef struct ZkTriggerWorldStart ZkTriggerWorldStart;
#include "zenkit-capi/vobs/Trigger.h"

#include "zenkit-capi/vobs/VirtualObject.h"
#include "zenkit-capi/vobs/Zone.h"

// world
#include "zenkit-capi/world/BspTree.h"
#include "zenkit-capi/world/WayNet.h"

// Other
#include "zenkit-capi/Archive.h"
#include "zenkit-capi/Boxes.h"
#include "zenkit-capi/CutsceneLibrary.h"
#include "zenkit-capi/DaedalusScript.h"
#include "zenkit-capi/DaedalusVm.h"
#include "zenkit-capi/Date.h"
#include "zenkit-capi/Font.h"
#include "zenkit-capi/Library.h"
#include "zenkit-capi/Logger.h"
#include "zenkit-capi/Material.h"
#include "zenkit-capi/Matrix.h"
#include "zenkit-capi/Mesh.h"
#include "zenkit-capi/Model.h"
#include "zenkit-capi/ModelAnimation.h"
#include "zenkit-capi/ModelHierarchy.h"
#include "zenkit-capi/ModelMesh.h"
#include "zenkit-capi/ModelScript.h"
#include "zenkit-capi/MorphMesh.h"
#include "zenkit-capi/MultiResolutionMesh.h"
#include "zenkit-capi/Object.h"
#include "zenkit-capi/SaveGame.h"
#include "zenkit-capi/SoftSkinMesh.h"
#include "zenkit-capi/Stream.h"
#include "zenkit-capi/Texture.h"
#include "zenkit-capi/Vector.h"
#include "zenkit-capi/Vfs.h"
#include "zenkit-capi/World.h"
