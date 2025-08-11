use crate::{
    archetype::ArchetypeColumnInfo,
    bundle::{Bundle, BundleColumns},
    component::{Component, ComponentRef, ComponentRefMut},
    entity::Entity,
    query::{TypedLookupAccess, TypedLookupFetch, TypedQueryFetch, TypedQueryIter},
    world::{World, WorldChanges, WorldError},
};
use intuicio_data::type_hash::TypeHash;
use std::{error::Error, sync::RwLockReadGuard};

pub struct Resources {
    world: World,
    entity: Entity,
}

impl Default for Resources {
    fn default() -> Self {
        let mut world = World::default();
        let entity = world.spawn(((),)).unwrap();
        Self { world, entity }
    }
}

impl Resources {
    pub fn add(&mut self, bundle: impl Bundle) -> Result<(), Box<dyn Error>> {
        WorldError::allow(
            self.world.insert(self.entity, bundle),
            [WorldError::EmptyColumnSet],
            (),
        )?;
        Ok(())
    }

    pub fn remove<T: BundleColumns>(&mut self) -> Result<(), Box<dyn Error>> {
        self.world.remove::<T>(self.entity)?;
        Ok(())
    }

    pub fn remove_raw(&mut self, columns: Vec<ArchetypeColumnInfo>) -> Result<(), Box<dyn Error>> {
        self.world.remove_raw(self.entity, columns)?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.world.clear();
        self.entity = self.world.spawn(((),)).unwrap();
    }

    pub fn clear_changes(&mut self) {
        self.world.clear_changes();
    }

    pub fn added(&self) -> &WorldChanges {
        self.world.added()
    }

    pub fn removed(&self) -> &WorldChanges {
        self.world.removed()
    }

    pub fn updated(&self) -> Option<RwLockReadGuard<'_, WorldChanges>> {
        self.world.updated()
    }

    pub fn did_changed<T: Component>(&self) -> bool {
        self.world.component_did_changed::<T>()
    }

    pub fn did_changed_raw(&self, type_hash: TypeHash) -> bool {
        self.world.component_did_changed_raw(type_hash)
    }

    pub fn has<T: Component>(&self) -> bool {
        self.world.has_entity_component::<T>(self.entity)
    }

    pub fn ensure<const LOCKING: bool, T: Component + Default>(
        &'_ mut self,
    ) -> Result<ComponentRefMut<'_, LOCKING, T>, Box<dyn Error>> {
        if !self.world.has_entity_component::<T>(self.entity) {
            self.world.insert(self.entity, (T::default(),))?;
        }
        Ok(self.world.component_mut(self.entity)?)
    }

    pub fn get<const LOCKING: bool, T: Component>(
        &'_ self,
    ) -> Result<ComponentRef<'_, LOCKING, T>, Box<dyn Error>> {
        Ok(self.world.component(self.entity)?)
    }

    pub fn get_mut<const LOCKING: bool, T: Component>(
        &'_ self,
    ) -> Result<ComponentRefMut<'_, LOCKING, T>, Box<dyn Error>> {
        Ok(self.world.component_mut(self.entity)?)
    }

    pub fn query<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>>(
        &'a self,
    ) -> TypedQueryIter<'a, LOCKING, Fetch> {
        self.world.query::<LOCKING, Fetch>()
    }

    pub fn lookup<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
    ) -> TypedLookupAccess<'a, LOCKING, Fetch> {
        self.world.lookup_access::<LOCKING, Fetch>()
    }

    pub fn lookup_one<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
    ) -> Option<Fetch::ValueOne> {
        self.world.lookup_one::<LOCKING, Fetch>(self.entity)
    }
}
