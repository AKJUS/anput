use anput::world::World;
use bench::FooDefault;

const ITERATIONS: usize = 1000000;

fn main() {
    ittapi::pause();

    let query_event = ittapi::Event::new("Query entities");
    let fetch_event = ittapi::Event::new("Query fetch components");

    let mut world = World::default();

    for _ in 0..ITERATIONS {
        world.spawn((FooDefault::default(),)).unwrap();
    }

    {
        ittapi::resume();
        let event = query_event.start();
        let mut iter = world.query::<true, &mut FooDefault>();
        loop {
            let event = fetch_event.start();
            let Some(item) = iter.next() else {
                break;
            };
            drop(event);
            item.update();
        }
        drop(event);
        ittapi::pause();
    }
}
