use anput::{
    entity::Entity,
    query::TypedLookupFetch,
    scheduler::GraphSchedulerQuickPlugin,
    systems::SystemContext,
    universe::{Plugin, Res},
    world::World,
};
use rstar::{primitives::GeomWithData, Envelope, PointDistance, RTree, RTreeObject};
use std::{error::Error, marker::PhantomData};

pub mod third_party {
    pub use rstar;
}

pub struct SpatialPartitioningPlugin<const LOCKING: bool, Extractor: SpatialExtractor>(
    PhantomData<fn() -> Extractor>,
);

impl<const LOCKING: bool, Extractor: SpatialExtractor>
    SpatialPartitioningPlugin<LOCKING, Extractor>
{
    pub fn make() -> impl Plugin {
        GraphSchedulerQuickPlugin::<LOCKING, Self>::default()
            .resource(SpatialPartitioning::<Extractor>::default())
            .system(
                spatial_partitioning::<LOCKING, Extractor>,
                "spatial_partitioning",
                (),
            )
            .commit()
    }
}

pub struct SpatialPartitioning<Extractor: SpatialExtractor> {
    tree: RTree<GeomWithData<Extractor::SpatialObject, Entity>>,
}

impl<Extractor: SpatialExtractor> Default for SpatialPartitioning<Extractor> {
    fn default() -> Self {
        Self {
            tree: RTree::default(),
        }
    }
}

impl<Extractor: SpatialExtractor> SpatialPartitioning<Extractor> {
    pub fn rebuild<const LOCKING: bool>(&mut self, world: &World) {
        self.tree = RTree::bulk_load(
            Extractor::extract::<LOCKING>(world)
                .map(|(entity, object)| GeomWithData::new(object, entity))
                .collect::<Vec<_>>(),
        );
    }

    pub fn tree(&self) -> &RTree<GeomWithData<Extractor::SpatialObject, Entity>> {
        &self.tree
    }

    pub fn nearest_entities(
        &self,
        point: &<<Extractor::SpatialObject as RTreeObject>::Envelope as Envelope>::Point,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.tree.nearest_neighbor_iter(point).map(|geom| geom.data)
    }

    pub fn locate_contained_entities(
        &self,
        envelope: &<Extractor::SpatialObject as RTreeObject>::Envelope,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.tree.locate_in_envelope(envelope).map(|geom| geom.data)
    }

    pub fn locate_intersecting_entities(
        &self,
        envelope: &<Extractor::SpatialObject as RTreeObject>::Envelope,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.tree
            .locate_in_envelope_intersecting(envelope)
            .map(|geom| geom.data)
    }

    pub fn nearest_query<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
        world: &'a World,
        point: &<<Extractor::SpatialObject as RTreeObject>::Envelope as Envelope>::Point,
    ) -> impl Iterator<Item = Fetch::Value> {
        world.lookup::<LOCKING, Fetch>(self.nearest_entities(point))
    }

    pub fn locate_contained_query<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>>(
        &'a self,
        world: &'a World,
        envelope: &<Extractor::SpatialObject as RTreeObject>::Envelope,
    ) -> impl Iterator<Item = Fetch::Value> {
        world.lookup::<LOCKING, Fetch>(self.locate_contained_entities(envelope))
    }

    pub fn locate_intersecting_query<
        'a,
        const LOCKING: bool,
        Fetch: TypedLookupFetch<'a, LOCKING>,
    >(
        &'a self,
        world: &'a World,
        envelope: &<Extractor::SpatialObject as RTreeObject>::Envelope,
    ) -> impl Iterator<Item = Fetch::Value> {
        world.lookup::<LOCKING, Fetch>(self.locate_intersecting_entities(envelope))
    }
}

pub fn spatial_partitioning<const LOCKING: bool, Extractor: SpatialExtractor>(
    context: SystemContext,
) -> Result<(), Box<dyn Error>> {
    let (world, mut partitioning) =
        context.fetch::<(&World, Res<LOCKING, &mut SpatialPartitioning<Extractor>>)>()?;

    partitioning.rebuild::<LOCKING>(world);

    Ok(())
}

pub trait SpatialExtractor: 'static
where
    <<Self as SpatialExtractor>::SpatialObject as RTreeObject>::Envelope: Send + Sync,
{
    type SpatialObject: RTreeObject + PointDistance + Send + Sync;

    fn extract<const LOCKING: bool>(
        world: &World,
    ) -> impl Iterator<Item = (Entity, Self::SpatialObject)>;
}
