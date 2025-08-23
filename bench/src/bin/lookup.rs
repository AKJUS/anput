use anput::{query::DynamicQueryFilter, world::World};
use bench::FooDefault;
use rand::seq::SliceRandom;

const ITERATIONS: usize = 1000000;

fn main() {
    ittapi::pause();

    let lookup_linear_event = ittapi::Event::new("Lookup linear entities");
    let lookup_random_event = ittapi::Event::new("Lookup random entities");
    let fetch_linear_event = ittapi::Event::new("Lookup linear fetch components");
    let fetch_random_event = ittapi::Event::new("Lookup random fetch components");

    let dynamic_lookup_linear_event = ittapi::Event::new("Dynamic lookup linear entities");
    let dynamic_lookup_random_event = ittapi::Event::new("Dynamic lookup random entities");
    let dynamic_fetch_linear_event = ittapi::Event::new("Dynamic lookup linear fetch components");
    let dynamic_fetch_random_event = ittapi::Event::new("Dynamic lookup random fetch components");

    let access_lookup_linear_event = ittapi::Event::new("Lookup access linear entities");
    let access_lookup_random_event = ittapi::Event::new("Lookup access random entities");
    let access_fetch_linear_event = ittapi::Event::new("Lookup access linear fetch components");
    let access_fetch_random_event = ittapi::Event::new("Lookup access random fetch components");

    let dynamic_access_lookup_linear_event =
        ittapi::Event::new("Dynamic lookup access linear entities");
    let dynamic_access_lookup_random_event =
        ittapi::Event::new("Dynamic lookup access random entities");
    let dynamic_access_fetch_linear_event =
        ittapi::Event::new("Dynamic lookup access linear fetch components");
    let dynamic_access_fetch_random_event =
        ittapi::Event::new("Dynamic lookup access random fetch components");

    let mut world = World::default();

    for _ in 0..ITERATIONS {
        world.spawn((FooDefault::default(),)).unwrap();
    }

    let mut entities = world.entities().collect::<Vec<_>>();

    {
        ittapi::resume();
        let event = lookup_linear_event.start();
        let mut iter = world.lookup::<true, &mut FooDefault>(entities.iter().copied());
        loop {
            let event = fetch_linear_event.start();
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
        let event = dynamic_lookup_linear_event.start();
        let filter = DynamicQueryFilter::default().write::<FooDefault>();
        let mut iter = world.dynamic_lookup::<true>(&filter, entities.iter().copied());
        loop {
            let event = dynamic_fetch_linear_event.start();
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

    {
        ittapi::resume();
        let event = access_lookup_linear_event.start();
        let mut access = world.lookup_access::<true, &mut FooDefault>();
        for entity in entities.iter().copied() {
            let event = access_fetch_linear_event.start();
            let Some(item) = access.access(entity) else {
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
        let event = dynamic_access_lookup_linear_event.start();
        let filter = DynamicQueryFilter::default().write::<FooDefault>();
        let access = world.dynamic_lookup_access::<true>(&filter);
        for entity in entities.iter().copied() {
            let event = dynamic_access_fetch_linear_event.start();
            let Some(mut item) = access.access(entity) else {
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

    entities.shuffle(&mut rand::rng());

    {
        ittapi::resume();
        let event = lookup_random_event.start();
        let mut iter = world.lookup::<true, &mut FooDefault>(entities.iter().copied());
        loop {
            let event = fetch_random_event.start();
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
        let event = dynamic_lookup_random_event.start();
        let filter = DynamicQueryFilter::default().write::<FooDefault>();
        let mut iter = world.dynamic_lookup::<true>(&filter, entities.iter().copied());
        loop {
            let event = dynamic_fetch_random_event.start();
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

    {
        ittapi::resume();
        let event = access_lookup_random_event.start();
        let mut access = world.lookup_access::<true, &mut FooDefault>();
        for entity in entities.iter().copied() {
            let event = access_fetch_random_event.start();
            let Some(item) = access.access(entity) else {
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
        let event = dynamic_access_lookup_random_event.start();
        let filter = DynamicQueryFilter::default().write::<FooDefault>();
        let access = world.dynamic_lookup_access::<true>(&filter);
        for entity in entities.iter().copied() {
            let event = dynamic_access_fetch_random_event.start();
            let Some(mut item) = access.access(entity) else {
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
