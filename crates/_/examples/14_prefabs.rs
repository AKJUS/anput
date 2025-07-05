use anput::{prefab::Prefab, prelude::*, processor::WorldProcessor};
use intuicio_core::prelude::*;
use intuicio_derive::IntuicioStruct;
use intuicio_framework_serde::SerializationRegistry;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(IntuicioStruct, Debug, Default, Clone, Copy, Serialize, Deserialize)]
struct Health {
    value: usize,
}

#[derive(IntuicioStruct, Debug, Default, Clone, Copy, Serialize, Deserialize)]
struct Strength {
    value: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut universe = Universe::default().with_basics(10240, 10240);

    // Register components to type registry (holds reflection info of each type).
    let registry = &mut *universe.resources.get_mut::<true, Registry>()?;
    registry.add_type(Health::define_struct(registry));
    registry.add_type(Strength::define_struct(registry));

    // Register components to serialization registry (tells how to (de)serialize each type).
    let serialization = &mut *universe
        .resources
        .get_mut::<true, SerializationRegistry>()?;
    serialization.register_serde::<Health>();
    serialization.register_serde::<Strength>();

    // Grab access to world processor (here does nothing, but knows how to remap entities in components).
    let processor = &*universe.resources.get::<true, WorldProcessor>()?;

    // Setup heroes entities.
    universe
        .simulation
        .spawn((Health { value: 100 }, Strength { value: 20 }))?;
    universe
        .simulation
        .spawn((Health { value: 120 }, Strength { value: 15 }))?;

    // Make prefab from world and serialize it to JSON.
    let prefab = Prefab::from_world::<true>(&universe.simulation, serialization, registry)?;
    let serialized = serde_json::to_string_pretty(&prefab)?;

    println!("{serialized}");

    // Deserialize JSON to prefab and build world from it.
    let deserialized = serde_json::from_str::<Prefab>(&serialized)?;
    let world = deserialized
        .to_world::<true>(processor, serialization, registry, ())?
        .0;

    for (entity, health, strength) in world.query::<true, (Entity, &Health, &Strength)>() {
        println!("Entity: {entity} | Health: {health:?} | Strength: {strength:?}");
    }

    Ok(())
}
