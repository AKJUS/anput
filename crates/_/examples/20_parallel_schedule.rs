use anput::{
    commands::{CommandBuffer, SpawnCommand},
    entity::Entity,
    query::Query,
    scheduler::{GraphScheduler, GraphSchedulerPlugin, SystemParallelize},
    systems::SystemContext,
    universe::{Res, Universe},
    world::{Relation, World},
};
use moirai::Jobs;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Energy(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Water(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Age(pub usize);

struct Parent;

fn main() -> Result<(), Box<dyn Error>> {
    let mut universe = Universe::default().with_basics(10240, 10240)?.with_plugin(
        GraphSchedulerPlugin::<true>::default()
            // We define sequenced group for tree progression.
            .plugin_setup(|plugin| {
                // In that group we run all its systems in parallel, because we know
                // they don't interact with same components mutably.
                // Group will wait for all its parallelized systems to complete and
                // only then this group completes.
                plugin
                    .system_setup(consume_energy, |system| {
                        system
                            .name("consume_energy")
                            .local(SystemParallelize::AnyWorker)
                    })
                    .system_setup(consume_water, |system| {
                        system
                            .name("consume_water")
                            .local(SystemParallelize::AnyWorker)
                    })
                    .system_setup(age, |system| {
                        system.name("age").local(SystemParallelize::AnyWorker)
                    })
            })
            // After entire progression group completes, we then run reproduction
            // system sequenced, because reproduction needs to mutate all components
            // to be able to spawn new trees.
            .system_setup(reproduce, |system| {
                system.name("reproduce").local(SystemParallelize::AnyWorker)
            }),
    );
    let jobs = Jobs::default();
    let scheduler = GraphScheduler::<true>;

    // Spawn first tree that will start chain of new generations.
    universe.simulation.spawn((
        Energy::default(),
        Water::default(),
        Age::default(),
        Relation::<Parent>::default(),
    ))?;

    // Run few frames to get few generations.
    for _ in 0..5 {
        scheduler.run(&jobs, &mut universe)?;
    }

    // Report forest population.
    for (entity, energy, water, age, parent) in
        universe
            .simulation
            .query::<true, (Entity, &Energy, &Water, &Age, &Relation<Parent>)>()
    {
        println!(
            "- Tree: {} | Energy: {} | Water: {} | Age: {} | Parent: {}",
            entity,
            energy.0,
            water.0,
            age.0,
            parent.entities().next().unwrap_or_default()
        );
    }

    Ok(())
}

fn consume_energy(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, query) = context.fetch::<(&World, Query<true, &mut Energy>)>()?;

    for energy in query.query(world) {
        energy.0 += 2;
    }

    Ok(())
}

fn consume_water(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, query) = context.fetch::<(&World, Query<true, &mut Water>)>()?;

    for water in query.query(world) {
        water.0 += 1;
    }

    Ok(())
}

fn age(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, query) = context.fetch::<(&World, Query<true, &mut Age>)>()?;

    for age in query.query(world) {
        age.0 += 1;
    }

    Ok(())
}

fn reproduce(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut commands, query) = context.fetch::<(
        &World,
        Res<true, &mut CommandBuffer>,
        Query<true, (Entity, &mut Energy, &mut Water)>,
    )>()?;

    for (entity, energy, water) in query.query(world) {
        while energy.0 >= 4 && water.0 >= 2 {
            energy.0 -= 4;
            water.0 -= 2;

            commands.command(SpawnCommand::new((
                Energy(2),
                Water(1),
                Age::default(),
                Relation::new(Parent, entity),
            )));
        }
    }

    Ok(())
}
