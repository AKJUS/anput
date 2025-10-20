use crate::{
    archetype::{Archetype, ArchetypeColumnInfo, ArchetypeView},
    bundle::BundleColumns,
    component::Component,
    entity::Entity,
    query::{
        DynamicLookupAccess, DynamicLookupIter, DynamicQueryFilter, DynamicQueryIter,
        TypedLookupAccess, TypedLookupFetch, TypedLookupIter, TypedQueryFetch, TypedQueryIter,
    },
    world::World,
};
use std::{
    marker::PhantomData,
    ops::{Bound, Deref, RangeBounds},
};

pub struct TypedWorldView<B: BundleColumns> {
    view: WorldView,
    _phantom: PhantomData<B>,
}

impl<B: BundleColumns> TypedWorldView<B> {
    pub fn new(world: &World) -> Self {
        Self {
            view: WorldView::new::<B>(world),
            _phantom: PhantomData,
        }
    }

    pub fn new_raw(view: WorldView) -> Option<Self> {
        for column in B::columns_static() {
            if !view.views.iter().any(|v| v.has_column(&column)) {
                return None;
            }
        }
        Some(Self {
            view,
            _phantom: PhantomData,
        })
    }

    pub fn into_inner(self) -> WorldView {
        self.view
    }
}

impl<B: BundleColumns> Clone for TypedWorldView<B> {
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<B: BundleColumns> Deref for TypedWorldView<B> {
    type Target = WorldView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[derive(Default, Clone)]
pub struct WorldView {
    views: Vec<ArchetypeView>,
}

impl WorldView {
    pub fn new<B: BundleColumns>(world: &World) -> Self {
        Self::default().with::<B>(world)
    }

    pub fn with<B: BundleColumns>(mut self, world: &World) -> Self {
        self.include::<B>(world);
        self
    }

    pub fn with_raw(mut self, world: &World, columns: &[ArchetypeColumnInfo]) -> Self {
        self.include_raw(world, columns);
        self
    }

    pub fn include<B: BundleColumns>(&mut self, world: &World) {
        for archetype in world.archetypes() {
            if let Some(view) = archetype.view::<B>() {
                self.views.push(view);
            }
        }
    }

    pub fn include_raw(&mut self, world: &World, columns: &[ArchetypeColumnInfo]) {
        for archetype in world.archetypes() {
            if let Some(view) = archetype.view_raw(columns) {
                self.views.push(view);
            }
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.archetypes().map(|archetype| archetype.len()).sum()
    }

    #[inline]
    pub fn archetypes(&self) -> impl Iterator<Item = &Archetype> {
        self.views.iter().map(|view| view.archetype())
    }

    #[inline]
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.views.iter().flat_map(|view| view.entities().iter())
    }

    #[inline]
    pub fn entity_by_index(&self, mut index: usize) -> Option<Entity> {
        for archetype in self.archetypes() {
            if index >= archetype.len() {
                index -= archetype.len();
                continue;
            }
            return archetype.entities().get(index);
        }
        None
    }

    #[inline]
    pub fn entities_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = Entity> + '_ {
        WorldViewEntityRangeIter::new(self, range)
    }

    #[inline]
    pub fn entities_work_group(
        &self,
        group_index: usize,
        mut groups_count: usize,
        mut min_items_per_group: usize,
    ) -> impl Iterator<Item = Entity> + '_ {
        groups_count = groups_count.max(1);
        min_items_per_group = min_items_per_group.max(1);
        let group_size = (self.len() / groups_count).max(min_items_per_group);
        let start = group_index * group_size;
        let end = start + group_size;
        self.entities_range(start..end)
    }

    pub fn find_by<const LOCKING: bool, T: Component + PartialEq>(
        &self,
        data: &T,
    ) -> Option<Entity> {
        for (entity, component) in self.query::<LOCKING, (Entity, &T)>() {
            if component == data {
                return Some(entity);
            }
        }
        None
    }

    pub fn find_with<const LOCKING: bool, T: Component>(
        &self,
        f: impl Fn(&T) -> bool,
    ) -> Option<Entity> {
        for (entity, component) in self.query::<LOCKING, (Entity, &T)>() {
            if f(component) {
                return Some(entity);
            }
        }
        None
    }

    pub fn entity<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
        entity: Entity,
    ) -> Option<Fetch::Value> {
        // TODO: this might be fucked up here, i believe we could potentially extend
        // fetched references lifetimes, which can lead to memory corruption - INVESTIGATE!
        self.lookup_access::<LOCKING, Fetch>().access(entity)
    }

    pub fn query<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>>(
        &'a self,
    ) -> TypedQueryIter<'a, LOCKING, Fetch> {
        TypedQueryIter::new_view(self)
    }

    pub fn dynamic_query<'a, const LOCKING: bool>(
        &'a self,
        filter: &DynamicQueryFilter,
    ) -> DynamicQueryIter<'a, LOCKING> {
        DynamicQueryIter::new_view(filter, self)
    }

    pub fn lookup<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
        entities: impl IntoIterator<Item = Entity> + 'a,
    ) -> TypedLookupIter<'a, LOCKING, Fetch> {
        TypedLookupIter::new_view(self, entities)
    }

    pub fn lookup_access<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
    ) -> TypedLookupAccess<'a, LOCKING, Fetch> {
        TypedLookupAccess::new_view(self)
    }

    pub fn dynamic_lookup<'a, const LOCKING: bool>(
        &'a self,
        filter: &DynamicQueryFilter,
        entities: impl IntoIterator<Item = Entity> + 'a,
    ) -> DynamicLookupIter<'a, LOCKING> {
        DynamicLookupIter::new_view(filter, self, entities)
    }

    pub fn dynamic_lookup_access<'a, const LOCKING: bool>(
        &'a self,
        filter: &DynamicQueryFilter,
    ) -> DynamicLookupAccess<'a, LOCKING> {
        DynamicLookupAccess::new_view(filter, self)
    }
}

pub struct WorldViewEntityRangeIter<'a> {
    views: &'a [ArchetypeView],
    index: usize,
    offset: usize,
    left: usize,
}

impl<'a> WorldViewEntityRangeIter<'a> {
    pub fn new(view: &'a WorldView, range: impl RangeBounds<usize>) -> Self {
        let size = view.len();
        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => start.saturating_add(1),
            Bound::Unbounded => 0,
        }
        .min(size);
        let end = match range.end_bound() {
            Bound::Included(end) => end.saturating_add(1),
            Bound::Excluded(end) => *end,
            Bound::Unbounded => size,
        }
        .max(start)
        .min(size);
        let mut offset = start;
        let mut index = 0;
        for view in &view.views {
            if offset >= view.len() {
                offset -= view.len();
                index += 1;
            } else {
                break;
            }
        }
        let left = end - start;
        Self {
            views: &view.views,
            index,
            offset,
            left,
        }
    }
}

impl Iterator for WorldViewEntityRangeIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        while self.left > 0 {
            let view = self.views.get(self.index)?;
            if let Some(entity) = view.entities().get(self.offset) {
                self.offset += 1;
                self.left -= 1;
                return Some(entity);
            } else {
                self.offset = 0;
                self.index += 1;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moirai::Jobs;
    use std::{
        thread::{sleep, spawn},
        time::Duration,
    };

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_world_view_threads() {
        is_async::<WorldView>();

        let mut world = World::default();
        for index in 0..20usize {
            world.spawn((index, index % 2 == 0)).unwrap();
        }

        let view = WorldView::new::<(usize,)>(&world);
        let join = spawn(move || {
            view.query::<true, &usize>()
                .inspect(|value| {
                    println!("Value: {value}");
                    sleep(Duration::from_millis(10));
                })
                .copied()
                .sum::<usize>()
        });

        sleep(Duration::from_millis(50));
        println!("Try spawn SDIR locked columns");
        // View has SDIR locked `usize` column, so we can't spawn anything
        // with those columns.
        assert!(world.spawn((42usize, false)).is_err());

        sleep(Duration::from_millis(50));
        println!("Spawn SDIR unlocked columns");
        // Neighter `bool`, nor `i32` column has SDIR lock in view, so we
        // are safe to spawn those.
        world.spawn((true, 42i32)).unwrap();

        sleep(Duration::from_millis(50));
        println!("Wait for job result");
        let sum = join.join().unwrap();
        println!("Sum: {sum}");
        assert_eq!(sum, world.query::<true, &usize>().copied().sum::<usize>());

        // View no longer exists, so no more SDIR lock on columns.
        println!("Spawn previously SDIR locked columns");
        world.spawn((42usize, false)).unwrap();
    }

    #[test]
    fn test_world_view_parallel() {
        const N: usize = if cfg!(miri) { 10 } else { 1000 };
        let jobs = Jobs::default();

        let mut world = World::default();
        for index in 0..N {
            world.spawn((index, index % 2 == 0)).unwrap();
        }

        // World view over selected columns.
        let view = WorldView::new::<(usize,)>(&world);

        // Process view with parallelized execution of view work groups.
        let sum = jobs
            .broadcast(move |ctx| {
                let entities =
                    view.entities_work_group(ctx.work_group_index, ctx.work_groups_count, 10);
                view.lookup::<true, &usize>(entities)
                    .copied()
                    .sum::<usize>()
            })
            .unwrap()
            .wait()
            .unwrap()
            .into_iter()
            .sum::<usize>();

        assert_eq!(sum, world.query::<true, &usize>().copied().sum());
    }
}
