use anput::world::World;
use bench::FooDefault;
use rand::seq::SliceRandom;

const ITERATIONS: usize = 1000000;

fn main() {
    ittapi::pause();

    let lookup_linear_event = ittapi::Event::new("Lookup access linear entities");
    let lookup_random_event = ittapi::Event::new("Lookup access random entities");
    let fetch_linear_event = ittapi::Event::new("Lookup access linear fetch components");
    let fetch_random_event = ittapi::Event::new("Lookup access random fetch components");

    let mut world = World::default();

    for _ in 0..ITERATIONS {
        world.spawn((FooDefault::default(),)).unwrap();
    }

    let mut entities = world.entities().collect::<Vec<_>>();

    {
        ittapi::resume();
        let event = lookup_linear_event.start();
        let mut access = world.lookup_access::<true, &mut FooDefault>();
        for entity in entities.iter().copied() {
            let event = fetch_linear_event.start();
            let Some(item) = access.access(entity) else {
                break;
            };
            drop(event);
            item.update();
        }
        drop(event);
        ittapi::pause();
    }

    entities.shuffle(&mut rand::rng());

    {
        ittapi::resume();
        let event = lookup_random_event.start();
        let mut access = world.lookup_access::<true, &mut FooDefault>();
        for entity in entities.iter().copied() {
            let event = fetch_random_event.start();
            let Some(item) = access.access(entity) else {
                break;
            };
            drop(event);
            item.update();
        }
        drop(event);
        ittapi::pause();
    }
}
