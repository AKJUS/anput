use crate::archetype::ArchetypeEntityColumnAccess;
use std::ops::{Deref, DerefMut};

pub trait Component: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Component for T {}

pub struct ComponentRef<'a, const LOCKING: bool, T: Component> {
    pub(crate) inner: ArchetypeEntityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Component> Deref for ComponentRef<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.read().unwrap()
    }
}

pub struct ComponentRefMut<'a, const LOCKING: bool, T: Component> {
    pub(crate) inner: ArchetypeEntityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Component> Deref for ComponentRefMut<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.read().unwrap()
    }
}

impl<const LOCKING: bool, T: Component> DerefMut for ComponentRefMut<'_, LOCKING, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.write().unwrap()
    }
}
