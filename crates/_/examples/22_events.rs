use anput::{
    entity::Entity,
    event::{EventDispatcher, EventSink},
    world::World,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut world = World::default();

    let mut event = EventDispatcher::<Entity>::default();
    let (_, receiver) = event.bind_sender_make();
    let (_, sink) = event.bind_sink_make();
    let bob = world.spawn((event,))?;
    let alice = world.spawn((sink,))?;

    for (entity, event) in world.query::<true, (Entity, &EventDispatcher<Entity>)>() {
        event.dispatch(&entity);
    }

    assert_eq!(receiver.recv().unwrap(), bob);
    assert_eq!(
        world
            .component::<true, EventSink<Entity>>(alice)
            .unwrap()
            .recv()
            .unwrap(),
        bob
    );

    Ok(())
}
