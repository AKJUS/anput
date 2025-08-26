use anput::{query::DynamicQueryFilter, world::World};
use bench::FooDefault;

const ITERATIONS: usize = 1000000;

fn main() {
    ittapi::pause();

    let query_event = ittapi::Event::new("Query entities");
    let fetch_event = ittapi::Event::new("Query fetch components");

    let dynamic_query_event = ittapi::Event::new("Dynamic query entities");
    let dynamic_fetch_event = ittapi::Event::new("Dynamic query fetch components");

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

    {
        ittapi::resume();
        let event = dynamic_query_event.start();
        let filter = DynamicQueryFilter::default().write::<FooDefault>();
        let mut iter = world.dynamic_query::<true>(&filter);
        loop {
            let event = dynamic_fetch_event.start();
            let Some(mut item) = iter.next() else {
                break;
            };
            drop(event);
            let item = item.write::<FooDefault>().unwrap();
            let item = item.write::<FooDefault>().unwrap();
            item.update();
        }
        drop(event);
        ittapi::pause();
    }
}
