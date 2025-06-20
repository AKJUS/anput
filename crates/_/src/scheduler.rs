use crate::{
    bundle::{Bundle, BundleChain},
    component::Component,
    entity::Entity,
    jobs::{Jobs, ScopedJobs},
    systems::{System, SystemContext, SystemObject},
    universe::{QuickPlugin, Universe},
    world::Relation,
};
use anput_jobs::JobLocation;
use std::{
    borrow::Cow,
    collections::{HashSet, VecDeque},
    error::Error,
    sync::RwLock,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemPriority(pub usize);

impl SystemPriority {
    pub fn top() -> Self {
        Self(usize::MAX)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]

pub struct SystemOrder(pub usize);
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SystemGroupChild;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SystemDependsOn;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum SystemParallelize {
    #[default]
    AnyWorker,
    NamedWorker(Cow<'static, str>),
}

#[derive(Default)]
pub struct GraphScheduler<const LOCKING: bool> {
    jobs: Jobs,
}

impl<const LOCKING: bool> GraphScheduler<LOCKING> {
    pub fn new(jobs: Jobs) -> Self {
        Self { jobs }
    }

    pub fn run(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        let mut visited = HashSet::with_capacity(universe.systems.len());
        let roots = Self::find_roots(universe);
        Self::validate_no_cycles(universe, roots.iter().copied(), &mut visited)?;
        visited.clear();
        let queue = VecDeque::default();
        let visited = RwLock::new(visited);
        let queue = RwLock::new(queue);
        self.run_group(universe, roots.into_iter(), &visited, &queue)?;
        while let Some(entity) = queue.write().unwrap().pop_front() {
            let mut scoped_jobs = ScopedJobs::new(&self.jobs);
            self.run_node(universe, entity, &visited, &queue, &mut scoped_jobs)?;
        }
        universe.clear_changes();
        universe.execute_commands::<LOCKING>();
        universe.maintain_plugins();
        Ok(())
    }

    fn find_roots(universe: &Universe) -> HashSet<Entity> {
        let mut entities = universe.systems.entities().collect::<HashSet<_>>();
        for relations in universe
            .systems
            .query::<LOCKING, &Relation<SystemGroupChild>>()
        {
            for entity in relations.entities() {
                if entities.contains(&entity) {
                    entities.remove(&entity);
                }
            }
        }
        entities
    }

    fn validate_no_cycles(
        universe: &Universe,
        entities: impl Iterator<Item = Entity>,
        visited: &mut HashSet<Entity>,
    ) -> Result<(), Box<dyn Error>> {
        for entity in entities {
            if visited.contains(&entity) {
                return Err(
                    format!("Found systems graph cycle for system entity: {}", entity).into(),
                );
            }
            visited.insert(entity);
            Self::validate_no_cycles(
                universe,
                universe
                    .systems
                    .relations_outgoing::<LOCKING, SystemGroupChild>(entity)
                    .map(|(_, _, entity)| entity)
                    .collect::<Vec<_>>()
                    .into_iter(),
                visited,
            )?;
        }
        Ok(())
    }

    fn run_node<'env>(
        &'env self,
        universe: &'env Universe,
        entity: Entity,
        visited: &'env RwLock<HashSet<Entity>>,
        queue: &'env RwLock<VecDeque<Entity>>,
        scoped_jobs: &mut ScopedJobs<'env, Result<(), String>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut visited_lock = visited.write().unwrap();
        let mut queue_lock = queue.write().unwrap();
        if visited_lock.contains(&entity) {
            return Ok(());
        }
        if universe
            .systems
            .relations_outgoing::<LOCKING, SystemDependsOn>(entity)
            .any(|(_, _, other)| !visited_lock.contains(&other))
        {
            queue_lock.push_back(entity);
            return Ok(());
        }
        visited_lock.insert(entity);
        drop(visited_lock);
        drop(queue_lock);
        let job = move || -> Result<(), String> {
            if let Ok(system) = universe.systems.component::<LOCKING, SystemObject>(entity) {
                if system.should_run(SystemContext::new(universe, entity)) {
                    system
                        .run(SystemContext::new(universe, entity))
                        .map_err(|error| format!("{}", error))?;
                }
            }
            self.run_group(
                universe,
                universe
                    .systems
                    .relations_outgoing::<LOCKING, SystemGroupChild>(entity)
                    .map(|(_, _, entity)| entity),
                visited,
                queue,
            )
            .map_err(|error| format!("{}", error))?;
            Ok(())
        };
        if let Ok(parallelize) = universe
            .systems
            .component::<LOCKING, SystemParallelize>(entity)
        {
            match &*parallelize {
                SystemParallelize::AnyWorker => {
                    scoped_jobs.queue_on(JobLocation::UnnamedWorker, move |_| job())?
                }
                SystemParallelize::NamedWorker(cow) => {
                    scoped_jobs.queue_on(JobLocation::named_worker(cow.as_ref()), move |_| job())?
                }
            }
        } else {
            job()?;
        }
        Ok(())
    }

    fn run_group(
        &self,
        universe: &Universe,
        entities: impl Iterator<Item = Entity>,
        visited: &RwLock<HashSet<Entity>>,
        queue: &RwLock<VecDeque<Entity>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut ordered = entities
            .map(|entity| {
                let priority = universe
                    .systems
                    .component::<LOCKING, SystemPriority>(entity)
                    .ok()
                    .map(|priority| *priority)
                    .unwrap_or_default();
                let order = universe
                    .systems
                    .component::<LOCKING, SystemOrder>(entity)
                    .ok()
                    .map(|order| *order)
                    .unwrap_or_default();
                (entity, priority, order)
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|(_, priority_a, order_a), (_, priority_b, order_b)| {
            priority_a
                .cmp(priority_b)
                .reverse()
                .then(order_a.cmp(order_b))
        });
        let mut scoped_jobs = ScopedJobs::new(&self.jobs);
        for (entity, _, _) in ordered {
            self.run_node(universe, entity, visited, queue, &mut scoped_jobs)?;
        }
        for result in scoped_jobs.execute() {
            result?;
        }
        Ok(())
    }
}

pub struct GraphSchedulerQuickPlugin<const LOCKING: bool, Tag: Send + Sync> {
    plugin: QuickPlugin<Tag>,
    order: usize,
}

impl<const LOCKING: bool, Tag: Send + Sync> Default for GraphSchedulerQuickPlugin<LOCKING, Tag> {
    fn default() -> Self {
        Self {
            plugin: Default::default(),
            order: 0,
        }
    }
}

impl<const LOCKING: bool, Tag: Send + Sync> GraphSchedulerQuickPlugin<LOCKING, Tag> {
    pub fn new(plugin: QuickPlugin<Tag>) -> Self {
        Self { plugin, order: 0 }
    }

    pub fn commit(self) -> QuickPlugin<Tag> {
        self.plugin
    }

    pub fn quick(mut self, f: impl FnOnce(QuickPlugin<Tag>) -> QuickPlugin<Tag>) -> Self {
        self.plugin = f(self.plugin);
        self
    }

    pub fn group<ID: Component + Clone + PartialEq, L: Bundle + Send + Sync + 'static>(
        mut self,
        id: ID,
        locals: L,
        f: impl FnOnce(GraphSchedulerGroup<LOCKING, ID, Tag>) -> GraphSchedulerGroup<LOCKING, ID, Tag>,
    ) -> Self {
        self.plugin = self
            .plugin
            .system_meta(BundleChain((id.clone(), SystemOrder(self.order)), locals));
        self.plugin = f(GraphSchedulerGroup {
            id,
            plugin: self.plugin,
            order: 0,
        })
        .plugin;
        self.order += 1;
        self
    }

    pub fn system<ID: Component>(
        mut self,
        system: impl System,
        id: ID,
        locals: impl Bundle + Send + Sync + 'static,
    ) -> Self {
        self.plugin = self
            .plugin
            .system(system, BundleChain((id, SystemOrder(self.order)), locals));
        self.order += 1;
        self
    }

    pub fn resource<T: Component>(mut self, resource: T) -> Self {
        self.plugin = self.plugin.resource(resource);
        self
    }

    pub fn with_resource<T: Component + Default>(
        mut self,
        f: impl Fn(&mut T) + Send + Sync + 'static,
    ) -> Self {
        self.plugin = self.plugin.with_resource(f);
        self
    }
}

pub struct GraphSchedulerGroup<
    const LOCKING: bool,
    ID: Component + Clone + PartialEq,
    Tag: Send + Sync,
> {
    id: ID,
    plugin: QuickPlugin<Tag>,
    order: usize,
}

impl<const LOCKING: bool, ID: Component + Clone + PartialEq, Tag: Send + Sync>
    GraphSchedulerGroup<LOCKING, ID, Tag>
{
    pub fn quick(mut self, f: impl FnOnce(QuickPlugin<Tag>) -> QuickPlugin<Tag>) -> Self {
        self.plugin = f(self.plugin);
        self
    }

    pub fn group<L: Bundle + Send + Sync + 'static>(
        mut self,
        id: ID,
        locals: L,
        f: impl FnOnce(Self) -> Self,
    ) -> Self {
        self.plugin = self
            .plugin
            .system_meta(BundleChain((id.clone(), SystemOrder(self.order)), locals));
        self.plugin = f(GraphSchedulerGroup {
            id: id.clone(),
            plugin: self.plugin,
            order: 0,
        })
        .plugin;
        self.plugin =
            self.plugin
                .system_relation::<LOCKING, _, _>(self.id.clone(), SystemGroupChild, id);
        self.order += 1;
        self
    }

    pub fn system(
        mut self,
        system: impl System,
        id: ID,
        locals: impl Bundle + Send + Sync + 'static,
    ) -> Self {
        self.plugin = self.plugin.system(
            system,
            BundleChain((id.clone(), SystemOrder(self.order)), locals),
        );
        self.plugin =
            self.plugin
                .system_relation::<LOCKING, _, _>(self.id.clone(), SystemGroupChild, id);
        self.order += 1;
        self
    }
}
