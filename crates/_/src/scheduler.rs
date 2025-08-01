use crate::{
    bundle::DynamicBundle,
    component::Component,
    entity::Entity,
    resources::Resources,
    systems::{System, SystemContext, SystemObject, Systems},
    universe::{Plugin, Universe},
    world::{Relation, World},
};
use anput_jobs::{JobLocation, JobPriority, Jobs, ScopedJobs};
use intuicio_data::managed::DynamicManaged;
use std::{
    borrow::Cow,
    collections::HashSet,
    error::Error,
    ops::{Deref, Range},
    sync::RwLock,
    time::{Duration, Instant},
};

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemName(Cow<'static, str>);

impl SystemName {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for SystemName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemInjectInto(Cow<'static, str>);

impl SystemInjectInto {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.0
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }
}

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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum SystemParallelize {
    #[default]
    AnyWorker,
    NamedWorker(Cow<'static, str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemSubsteps {
    Fixed(usize),
    TimeDuration(Duration),
}

impl Default for SystemSubsteps {
    fn default() -> Self {
        Self::Fixed(1)
    }
}

impl SystemSubsteps {
    pub fn iter(&self) -> SystemSubstepsIter {
        match self {
            SystemSubsteps::Fixed(count) => SystemSubstepsIter::Fixed(0..((*count).max(1))),
            SystemSubsteps::TimeDuration(duration) => SystemSubstepsIter::TimeDuration {
                duration: *duration,
                timer: Instant::now(),
                substep: 0,
            },
        }
    }
}

pub enum SystemSubstepsIter {
    Fixed(Range<usize>),
    TimeDuration {
        duration: Duration,
        timer: Instant,
        substep: usize,
    },
}

impl Iterator for SystemSubstepsIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SystemSubstepsIter::Fixed(range) => range.next(),
            SystemSubstepsIter::TimeDuration {
                duration,
                timer,
                substep,
            } => {
                let result = *substep;
                *substep += 1;
                if timer.elapsed() >= *duration {
                    None
                } else {
                    Some(result)
                }
            }
        }
    }
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
        let roots = Self::find_roots(&universe.systems);
        Self::validate_no_cycles(universe, roots.iter().copied(), &mut visited)?;
        visited.clear();
        let visited = RwLock::new(visited);
        self.run_group(
            universe,
            roots.into_iter(),
            &visited,
            SystemSubsteps::default(),
        )?;
        self.jobs.run_local();
        universe.clear_changes();
        universe.execute_commands::<LOCKING>();
        Ok(())
    }

    fn find_roots(systems: &Systems) -> HashSet<Entity> {
        let mut entities = systems.entities().collect::<HashSet<_>>();
        for relations in systems.query::<LOCKING, &Relation<SystemGroupChild>>() {
            for entity in relations.entities() {
                entities.remove(&entity);
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
                    format!("Found systems graph cycle for system entity: {entity}").into(),
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
        scoped_jobs: &mut ScopedJobs<'env, Result<(), String>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut visited_lock = visited.write().unwrap();
        if visited_lock.contains(&entity) {
            return Ok(());
        }
        visited_lock.insert(entity);
        drop(visited_lock);
        let job = move || -> Result<(), String> {
            if let Ok(system) = universe.systems.component::<LOCKING, SystemObject>(entity) {
                if system.should_run(SystemContext::new(universe, entity)) {
                    system
                        .run(SystemContext::new(universe, entity))
                        .map_err(|error| format!("{error}"))?;
                }
            }
            let substeps = universe
                .systems
                .component::<LOCKING, SystemSubsteps>(entity)
                .map(|substeps| *substeps)
                .unwrap_or_default();
            let entities = universe
                .systems
                .relations_outgoing::<LOCKING, SystemGroupChild>(entity)
                .map(|(_, _, entity)| entity);
            self.run_group(universe, entities, visited, substeps)
                .map_err(|error| format!("{error}"))?;
            Ok(())
        };
        if let Ok(parallelize) = universe
            .systems
            .component::<LOCKING, SystemParallelize>(entity)
        {
            match &*parallelize {
                SystemParallelize::AnyWorker => {
                    scoped_jobs
                        .queue_on(JobLocation::NonLocal, JobPriority::Normal, move |_| job())?
                }
                SystemParallelize::NamedWorker(cow) => scoped_jobs.queue_on(
                    JobLocation::named_worker(cow.as_ref()),
                    JobPriority::Normal,
                    move |_| job(),
                )?,
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
        substeps: SystemSubsteps,
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

        for _ in substeps.iter() {
            let mut scoped_jobs = ScopedJobs::new(&self.jobs);
            for (entity, _, _) in ordered.iter().copied() {
                self.run_node(universe, entity, visited, &mut scoped_jobs)?;
            }
            for result in scoped_jobs.execute() {
                result?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct GraphSchedulerPlugin<const LOCKING: bool> {
    locals: DynamicBundle,
    #[allow(clippy::type_complexity)]
    simulation: Vec<Box<dyn FnOnce(&mut World) + Send + Sync>>,
    resources: DynamicBundle,
    systems: Vec<DynamicBundle>,
    plugins: Vec<Self>,
    order: usize,
}

impl<const LOCKING: bool> GraphSchedulerPlugin<LOCKING> {
    pub fn make(self, f: impl FnOnce(Self) -> Self) -> Self {
        f(Self::default())
    }

    pub fn setup(self, f: impl FnOnce(Self) -> Self) -> Self {
        f(self)
    }

    pub fn maybe_setup(mut self, f: impl FnOnce(Self) -> Option<Self>) -> Self {
        let plugin = Self {
            order: self.order,
            ..Default::default()
        };
        if let Some(plugin) = f(plugin) {
            let Self {
                locals,
                simulation,
                resources,
                systems,
                plugins,
                order,
            } = plugin;
            self.locals.append(locals);
            self.simulation.extend(simulation);
            self.resources.append(resources);
            self.systems.extend(systems);
            self.plugins.extend(plugins);
            self.order = order;
        }
        self
    }

    pub fn name(self, name: impl Into<Cow<'static, str>>) -> Self {
        self.local(SystemName::new(name))
    }

    pub fn inject_into(self, name: impl Into<Cow<'static, str>>) -> Self {
        self.local(SystemInjectInto::new(name))
    }

    pub fn local<T: Component>(mut self, component: T) -> Self {
        self.locals.add_component(component).ok().unwrap();
        self
    }

    pub fn local_raw(mut self, component: DynamicManaged) -> Self {
        self.locals.add_component_raw(component);
        self
    }

    pub fn resource<T: Component>(mut self, resource: T) -> Self {
        self.resources.add_component(resource).ok().unwrap();
        self
    }

    pub fn resource_raw(mut self, resource: DynamicManaged) -> Self {
        self.resources.add_component_raw(resource);
        self
    }

    pub fn system(self, system: impl System) -> GraphSchedulerPluginSystem<LOCKING> {
        GraphSchedulerPluginSystem {
            bundle: DynamicBundle::new(SystemObject::new(system))
                .ok()
                .unwrap()
                .with_component(SystemOrder(self.order))
                .unwrap(),
            plugin: self,
        }
    }

    pub fn system_setup(
        self,
        system: impl System,
        f: impl FnOnce(GraphSchedulerPluginSystem<LOCKING>) -> GraphSchedulerPluginSystem<LOCKING>,
    ) -> Self {
        f(self.system(system)).commit()
    }

    pub fn is_empty(&self) -> bool {
        self.locals.is_empty()
            && self.simulation.is_empty()
            && self.resources.is_empty()
            && self.systems.is_empty()
            && self.plugins.is_empty()
    }

    pub fn plugin(mut self, plugin: Self) -> Self {
        self.plugins.push(plugin);
        self
    }

    pub fn plugin_setup(self, f: impl FnOnce(Self) -> Self) -> Self {
        self.plugin(f(Self::default()))
    }

    pub fn maybe_plugin_setup(self, f: impl FnOnce(Self) -> Option<Self>) -> Self {
        if let Some(plugin) = f(Self::default()) {
            self.plugin(plugin)
        } else {
            self
        }
    }

    fn apply(
        mut self,
        parent: Option<Entity>,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    ) {
        let parent = self
            .locals
            .remove_component::<SystemInjectInto>()
            .and_then(|v| Self::find_system_by_path(systems, v.as_str()))
            .or(parent);
        if self.locals.is_empty() {
            self.locals.add_component(()).unwrap();
        }
        let group = systems.spawn(self.locals).unwrap();
        if let Some(parent) = parent {
            systems
                .relate::<LOCKING, _>(SystemGroupChild, parent, group)
                .unwrap();
        }
        for plugin in self.plugins {
            if !plugin.is_empty() {
                plugin.apply(Some(group), simulation, systems, resources);
            }
        }
        for execute in self.simulation {
            execute(simulation);
        }
        resources.add(self.resources).unwrap();
        for mut bundle in self.systems {
            let parent = bundle
                .remove_component::<SystemInjectInto>()
                .and_then(|v| Self::find_system_by_path(systems, v.as_str()))
                .unwrap_or(group);
            let entity = systems.spawn(bundle).unwrap();
            systems
                .relate::<LOCKING, _>(SystemGroupChild, parent, entity)
                .unwrap();
        }
    }

    pub fn find_system_by_path(systems: &Systems, path: &str) -> Option<Entity> {
        let parts = path
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        for entity in GraphScheduler::<LOCKING>::find_roots(systems) {
            if let Some(found) = Self::find_system_inner(systems, entity, &parts) {
                return Some(found);
            }
        }
        None
    }

    fn find_system_inner(systems: &Systems, entity: Entity, parts: &[&str]) -> Option<Entity> {
        if parts.is_empty() {
            return None;
        }
        let search = parts[0];
        let parts = &parts[1..];
        if search != "*" {
            systems
                .component::<LOCKING, SystemName>(entity)
                .ok()
                .filter(|v| v.as_str() == search)?;
        }
        if parts.is_empty() {
            return Some(entity);
        }
        for (_, _, entity) in systems.relations_outgoing::<LOCKING, SystemGroupChild>(entity) {
            if let Some(found) = Self::find_system_inner(systems, entity, parts) {
                return Some(found);
            }
        }
        None
    }
}

impl<const LOCKING: bool> Plugin for GraphSchedulerPlugin<LOCKING> {
    fn install(self, simulation: &mut World, systems: &mut Systems, resources: &mut Resources) {
        self.apply(None, simulation, systems, resources);
    }
}

pub struct GraphSchedulerPluginSystem<const LOCKING: bool> {
    plugin: GraphSchedulerPlugin<LOCKING>,
    bundle: DynamicBundle,
}

impl<const LOCKING: bool> GraphSchedulerPluginSystem<LOCKING> {
    pub fn name(self, name: impl Into<Cow<'static, str>>) -> Self {
        self.local(SystemName::new(name))
    }

    pub fn inject_into(self, name: impl Into<Cow<'static, str>>) -> Self {
        self.local(SystemInjectInto::new(name))
    }

    pub fn local<T: Component>(mut self, component: T) -> Self {
        self.bundle.add_component(component).ok().unwrap();
        self
    }

    pub fn local_raw(mut self, component: DynamicManaged) -> Self {
        self.bundle.add_component_raw(component);
        self
    }

    pub fn commit(mut self) -> GraphSchedulerPlugin<LOCKING> {
        self.plugin.systems.push(self.bundle);
        self.plugin.order += 1;
        self.plugin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_scheduler_plugin() {
        let mut world = World::default();
        let mut systems = Systems::default();
        let mut resources = Resources::default();

        fn noop(_: SystemContext) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        let plugin = GraphSchedulerPlugin::<false>::default()
            .name("a")
            .plugin_setup(|plugin| {
                plugin
                    .name("b")
                    .system_setup(noop, |system| system.name("c"))
                    .system_setup(noop, |system| system.name("d").inject_into("a/b/c"))
            })
            .system_setup(noop, |system| system.name("e").inject_into("a/b/c/d"));
        plugin.install(&mut world, &mut systems, &mut resources);

        let a = systems
            .find_with::<true, SystemName>(|name| name.as_str() == "a")
            .unwrap();
        let b = systems
            .find_with::<true, SystemName>(|name| name.as_str() == "b")
            .unwrap();
        let c = systems
            .find_with::<true, SystemName>(|name| name.as_str() == "c")
            .unwrap();
        let d = systems
            .find_with::<true, SystemName>(|name| name.as_str() == "d")
            .unwrap();
        let e = systems
            .find_with::<true, SystemName>(|name| name.as_str() == "e")
            .unwrap();

        assert!(systems.has_relation::<true, SystemGroupChild>(a, b));
        assert!(systems.has_relation::<true, SystemGroupChild>(b, c));
        assert!(systems.has_relation::<true, SystemGroupChild>(c, d));
        assert!(systems.has_relation::<true, SystemGroupChild>(d, e));
    }
}
