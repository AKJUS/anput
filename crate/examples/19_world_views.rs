use anput::{prelude::*, view::WorldView};
use rand::{thread_rng, Rng};
use std::{error::Error, thread::spawn};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Position(f32, f32);

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Velocity(f32, f32);

fn main() -> Result<(), Box<dyn Error>> {
    let mut world = World::default();

    let mut rng = thread_rng();
    for _ in 0..1000 {
        let position = Position(rng.gen_range(-100.0..100.0), rng.gen_range(-100.0..100.0));
        let velocity = Velocity(rng.gen_range(-10.0..10.0), rng.gen_range(-10.0..10.0));
        world.spawn((position, velocity)).unwrap();
    }

    let view = WorldView::new::<(Position, Velocity)>(&world);
    spawn(move || {
        for (pos, vel) in view.query::<true, (&mut Position, &Velocity)>() {
            pos.0 += vel.0;
            pos.1 += vel.1;
        }
    })
    .join()
    .unwrap();

    Ok(())
}
