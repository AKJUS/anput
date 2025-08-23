use intuicio_core::{IntuicioStruct, registry::Registry};
use intuicio_derive::*;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap};

/// Represents an entity with a unique `id` and a `generation` to track lifecycle and version.
#[derive(IntuicioStruct, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[intuicio(module_name = "ecs_entity")]
pub struct Entity {
    #[intuicio(ignore)]
    pub(crate) id: u32,
    #[intuicio(ignore)]
    pub(crate) generation: u32,
}

impl Default for Entity {
    fn default() -> Self {
        Self::INVALID
    }
}

#[intuicio_methods(module_name = "ecs_entity")]
impl Entity {
    /// A constant representing an invalid `Entity`, which is the result of an invalid `id`.
    pub const INVALID: Self = unsafe { Self::new_unchecked(u32::MAX, 0) };

    /// Creates a new `Entity` with the specified `id` and `generation` if the `id` is valid.
    /// Returns `None` if the `id` is invalid (e.g., equals `u32::MAX`).
    pub const fn new(id: u32, generation: u32) -> Option<Self> {
        if id < u32::MAX {
            Some(Self { id, generation })
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// This method is unsafe because it bypasses the validity check for `id` and `generation`.
    /// It's the caller's responsibility to ensure the `id` and `generation` are appropriate,
    /// as an invalid `id` (such as `u32::MAX`) could cause undefined behavior in the ECS system.
    pub const unsafe fn new_unchecked(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Returns `true` if the `Entity` is valid, meaning the `id` is not equal to `u32::MAX`.
    #[intuicio_method()]
    pub const fn is_valid(self) -> bool {
        self.id < u32::MAX
    }

    /// Returns the `id` of the `Entity`.
    #[intuicio_method()]
    pub const fn id(self) -> u32 {
        self.id
    }

    /// Returns the `generation` of the `Entity`.
    #[intuicio_method()]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Converts the `Entity` into a `u64` representation combining the `generation` and `id`.
    #[intuicio_method()]
    pub const fn to_u64(self) -> u64 {
        ((self.generation as u64) << 32) | self.id as u64
    }

    /// Converts a `u64` value back into an `Entity`, decoding the `generation` and `id`.
    #[intuicio_method()]
    pub const fn from_u64(value: u64) -> Self {
        Self {
            generation: (value >> 32) as u32,
            id: value as u32,
        }
    }

    /// Increments the `generation` of the `Entity`. This method is typically used when an entity
    /// is reused or updated to prevent conflicts.
    pub(crate) const fn bump_generation(mut self) -> Self {
        self.generation = self.generation.wrapping_add(1);
        self
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(f, "@{}:#{}", self.id, self.generation)
        } else {
            write!(f, "@none:#{}", self.generation)
        }
    }
}

impl PartialOrd for Entity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.generation
            .cmp(&other.generation)
            .then(self.id.cmp(&other.id))
    }
}

impl Entity {
    /// Registers the `Entity` type and its associated methods in the given registry.
    pub fn install(registry: &mut Registry) {
        registry.add_type(Self::define_struct(registry));
        registry.add_function(Self::is_valid__define_function(registry));
        registry.add_function(Self::id__define_function(registry));
        registry.add_function(Self::generation__define_function(registry));
        registry.add_function(Self::to_u64__define_function(registry));
        registry.add_function(Self::from_u64__define_function(registry));
    }
}

/// A structure to store entities in a dense array.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityDenseMap {
    entity_to_index: BTreeMap<Entity, usize>,
    index_to_entity: BTreeMap<usize, Entity>,
    indices_to_reuse: Vec<usize>,
}

impl EntityDenseMap {
    /// Tells if there are no eentities stored.
    pub fn is_empty(&self) -> bool {
        self.entity_to_index.is_empty()
    }

    /// Returns number of entities stored.
    pub fn len(&self) -> usize {
        self.entity_to_index.len()
    }

    /// Clears the map, removing all entities from it.
    pub fn clear(&mut self) {
        self.entity_to_index.clear();
        self.index_to_entity.clear();
        self.indices_to_reuse.clear();
    }

    /// Inserts a new entity into the map.
    /// Returns `Err(index)` if the entity already exists, otherwise `Ok(index)` with the insertion index.
    pub fn insert(&mut self, entity: Entity) -> Result<usize, usize> {
        if let Some(index) = self.index_of(entity) {
            Err(index)
        } else {
            let index = if let Some(reused_index) = self.indices_to_reuse.pop() {
                reused_index
            } else {
                self.entity_to_index.len()
            };
            self.entity_to_index.insert(entity, index);
            self.index_to_entity.insert(index, entity);
            Ok(index)
        }
    }

    /// Removes an entity from the map and returns its index if it was found.
    pub fn remove(&mut self, entity: Entity) -> Option<usize> {
        let index = self.entity_to_index.remove(&entity)?;
        self.index_to_entity.remove(&index);
        self.indices_to_reuse.push(index);
        Some(index)
    }

    /// Checks whether the specified entity is present in the map.
    pub fn contains(&self, entity: Entity) -> bool {
        self.entity_to_index.contains_key(&entity)
    }

    /// Finds the index of the specified entity in the map.
    pub fn index_of(&self, entity: Entity) -> Option<usize> {
        self.entity_to_index.get(&entity).copied()
    }

    /// Retrieves the entity at the given index if available.
    pub fn get(&self, index: usize) -> Option<Entity> {
        self.index_to_entity.get(&index).copied()
    }

    /// Returns an iterator over the entities in the map.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entity_to_index.keys().copied()
    }
}
