use anput::world::World;
use bench::FooDefault;

const ITERATIONS: usize = 1000000;

fn main() {
    ittapi::pause();

    let spawn_event = ittapi::Event::new("Spawn entities");
    let spawn_single_event = ittapi::Event::new("Spawn entity");

    let despawn_event = ittapi::Event::new("Despawn entities");
    let despawn_single_event = ittapi::Event::new("Despawn entity");

    let mut world = World::default();

    {
        ittapi::resume();
        let event = spawn_event.start();
        for _ in 0..ITERATIONS {
            let event = spawn_single_event.start();
            world.spawn((FooDefault::default(),)).unwrap();
            drop(event);
        }
        drop(event);
        ittapi::pause();
    }

    let entities = world.entities().collect::<Vec<_>>();

    {
        ittapi::resume();
        let event = despawn_event.start();
        for &entity in &entities {
            let event = despawn_single_event.start();
            world.despawn(entity).unwrap();
            drop(event);
        }
        drop(event);
        ittapi::pause();
    }
}
