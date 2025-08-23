use anput::world::World;
use bench::FooDefault;

const ITERATIONS: usize = 1000000;

fn main() {
    ittapi::pause();

    let spawn_event = ittapi::Event::new("Spawn entities");

    let mut world = World::default();

    {
        ittapi::resume();
        let event = spawn_event.start();
        for _ in 0..ITERATIONS {
            world.spawn((FooDefault::default(),)).unwrap();
        }
        drop(event);
        ittapi::pause();
    }
}
