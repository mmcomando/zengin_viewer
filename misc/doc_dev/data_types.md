# Naming and conceptual thinking about game data

A game engine processes a lot of data. That data has various properties and
purposes, and two of them are easy to mix up in names and in code.

There are two main categories:

- Immutable data, ex: textures, model vertices, sounds
- State, ex: player health, animation frame time

## Immutable data

Most of the time there is no problem in naming and conceptual thinking about
immutable data. Data can be easily named, and what it contains is most of the
time obvious.

Immutable here means "not modified at runtime".

Examples:
- textures, sounds, fonts
- model vertices, compiled animation clips
- configuration tables: item definitions, spawn tables, combat tables
- level geometry, navigation data, spawn points
- localization strings

## State data

Data representing state is more problematic. It has several properties that are
easy to conflate, and conflating them creates technical debt. Two of those
properties are described here, each with its own values:

- save policy, whether a value must survive a reload: `Persistent` / `Ephemeral`
- provenance, where a value comes from: `Authoritative` / `Derived` / `Cached`

Each value is described on its own below. How they combine, and the symptoms of
conflating them, is covered in [Combining the properties](#combining-the-properties)
and [Mixing up the properties](#mixing-up-the-properties).

### Save policy

The two values below decide what a save file has to contain.

#### Persistent state

Data which can't be recomputed, so it has to be stored to recreate the whole
game state. It has to be saved for save/load functionality.

Examples:
- health, position, rotation, death flag
- inventory contents and item counts, equipped items
- quest progress, journal entries, faction standings
- in-game clock, play time, progress flags

#### Ephemeral state

Data is not needed during game load.

It is dropped at a save boundary and reset to a documented default. This data is not important for gameplay and can be dropped or recreated with approximation.

Examples:
- currently played sounds, spawned particles, emitters
- animation phase: current frame index and elapsed time
- animation queue: the clip waiting to play next, current facing direction
- look-at target, current patrol assignment, body state imposed by an interaction
- deadlines and cooldowns of any kind
- UI selection, hover highlight, scroll offset

### Provenance

The three values below describe where a value comes from and how far it may be
trusted as the source of truth.

#### Authoritative state

Data which defines the object. Sometimes the same conceptual thing might be
defined by multiple data structures, and authoritative data is the source of
truth among them.

Example usage: the game might change the player's armor in various places in the
code. From time to time the game can check whether the rendered visual matches
the authoritative state, to detect bugs or to fix temporary problems.

Examples:
- path to the player's armor model
- appearance identity: body and head model, texture variants, rig
  (skeleton hierarchy) name, scale and physical proportions
- equipped item, weapon mode
- statistics, talents, attribute values
- inventory items
- quest state: journal status, known dialog nodes, faction standings

#### Derived state

Data which can be derived from authoritative state or other data.

The test: with the same authoritative state and the same assets, is the result
identical?

Examples:
- vertices of a 3d model (derived from the model path)
- skeleton, bone matrices, inverse bind poses, colliders
- the animation node chosen for a desired action, weapon mode and current scheme
- decision data rebuilt by re-running an object's initialization
- event callbacks registered by re-running its initialization
- runtime entities and components spawned from authoritative data

#### Cached state

Data which is similar to derived, but might actually be a copy of any of the
above. It is introduced for convenience or performance reasons: a copy of data
that already exists, or the same data presented in a different form for faster
access.

Being a duplicate means it must be invalidated whenever its inputs change, which
normally means guarding it with a content or version fingerprint of those inputs.

Examples:
- nearby entity positions, spatial queries and broad-phase structures
- id to object and id to entity indexes, keeping logical ids and runtime objects
  in sync
- asset handles reused by path (materials, models, meshes, animations)
- resolved appearance strings stored in a save to skip an asset lookup per load

### Combining the properties

Every value has one save policy and one provenance:

| Property | Question it answers | Values |
|---|---|---|
| Save policy | must it survive a reload? | `Persistent` / `Ephemeral` |
| Provenance | where does the value come from? | `Authoritative` / `Derived` / `Cached` |

- `player health` — authoritative, persistent
- `armor mesh vertices` — derived, ephemeral
- resolved asset paths — cached, persistent
- nearby entity positions — cached, ephemeral

All combinations are allowed. A cache is data that was paid for once and kept,
which is why it may be written to a save and be persistent, or rebuilt every
load and be ephemeral. An authoritative value is usually persistent in practice,
but that is a choice per value, not a consequence of it being authoritative.

### Mixing up the properties

Being independent properties, the same value can legitimately be described with
more than one word. The confusion starts when the words are treated as
synonyms, or when a value is forced into one bucket to avoid saying two:

- **Immutable and derived are not alternatives.** Immutability is a third axis:
  model vertices are immutable *and* derived, a function of the model path. Do
  not invent a fourth category for data that is both.
- **Authoritative does not mean saved, and saved does not mean authoritative.**
  The first says the value is the source of truth, the second says it crosses a
  save boundary.
- **A cache is not automatically ephemeral, and an ephemeral value is not a
  cache.** Persisting a cache is a performance decision.
- **Derived and cached are easy to swap.** Both can come from the same
  computation; a cached value is only one that was kept past the moment it was
  computed, so the difference is the cost of recomputation, not the nature of
  the value.

Symptoms of conflating them into a single name or a single struct:

- the save file contains data that could be recomputed from what is already in it
- loading a save duplicates logic that already exists in spawn or init
- two systems write the same conceptual value and nobody can say which wins
- a value silently goes stale after reload because only one of its copies was saved
- a name does not tell whether losing the value is free or fatal

## Example state structures

The names below show the intended shape: the class is visible in the type name
inside the serialized state graph, and `Cached` / `Ephemeral` values stay out of
it.

### One struct per class

```rust
/// Defines the actor. Always persisted.
pub struct ActorAuthoritativeState {
    /// Appearance identity: ids only, everything else is derived from them.
    pub appearance: AppearanceIdentity,
    pub equipment: Equipment,
    pub inventory: Inventory,
    pub stats: Stats,
    /// Position and death flag.
    pub placement: Placement,
}

/// A function of `ActorAuthoritativeState` plus assets. Rebuilt, never saved.
#[derive(Default)]
pub struct ActorDerivedState {
    /// Resolved asset paths of `AppearanceIdentity`.
    pub appearance: AppearanceResolved,
    /// Behavior data rebuilt by re-running the actor initialization.
    pub routine: RoutineDerived,
    /// Event callbacks rebuilt by the current behavior state.
    pub perception: PerceptionDerived,
}

/// Reset to `Default` on load, never persisted.
#[derive(Default)]
pub struct ActorEphemeralState {
    /// Assignment the behavior last picked, only used to detect a change.
    pub assigned_waypoint: Option<WaypointId>,
    /// Actor currently looked at.
    pub look_at: Option<EntityId>,
    /// State imposed by the interaction currently in progress.
    pub interaction_state: Option<i32>,
    /// Time until which the actor refuses to talk.
    pub refuse_talk_until: Option<f64>,
}
```

## Naming rules

- Put the class in the type name if there are few structs which represent similar data. Ex. NpcVisualAuthoritativeState, NpcVisualDerivedState,
- Avoid words the codebase already overloads. `model` collides with the 3d model,
  `visual` with appearance identity. Do not call data "runtime" when it is the persisted core.
- Prefer naming the class of a *container* by how it is used, and keep the classes
  of its fields visible.
