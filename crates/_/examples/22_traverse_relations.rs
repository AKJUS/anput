use anput::prelude::*;
use std::error::Error;

// Relations.
struct Child;
struct Parent;

// Tag components.
struct Corpus;
struct Head;
struct Arm;
struct Hand;
struct Leg;
struct Foot;
struct CanHold;
struct CanStand;

enum Side {
    Left,
    Right,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut world = World::default();

    let corpus = world.spawn((Corpus, "corpus"))?;

    let head = world.spawn((Head, "head"))?;
    world.relate_pair::<true, _, _>(Parent, Child, corpus, head)?;

    let arm_left = world.spawn((Arm, Side::Left, "arm-left"))?;
    world.relate_pair::<true, _, _>(Parent, Child, corpus, arm_left)?;

    let hand_left = world.spawn((Hand, Side::Left, CanHold, "hand-left"))?;
    world.relate_pair::<true, _, _>(Parent, Child, arm_left, hand_left)?;

    let arm_right = world.spawn((Arm, Side::Right, "arm-right"))?;
    world.relate_pair::<true, _, _>(Parent, Child, corpus, arm_right)?;

    let hand_right = world.spawn((Hand, Side::Left, CanHold, "hand-right"))?;
    world.relate_pair::<true, _, _>(Parent, Child, arm_right, hand_right)?;

    let leg_left = world.spawn((Leg, Side::Left, "leg-left"))?;
    world.relate_pair::<true, _, _>(Parent, Child, corpus, leg_left)?;

    let foot_left = world.spawn((Foot, Side::Left, CanStand, "foot-left"))?;
    world.relate_pair::<true, _, _>(Parent, Child, leg_left, foot_left)?;

    let leg_right = world.spawn((Leg, Side::Right, "leg-right"))?;
    world.relate_pair::<true, _, _>(Parent, Child, corpus, leg_right)?;

    let foot_right = world.spawn((Foot, Side::Right, CanStand, "foot-right"))?;
    world.relate_pair::<true, _, _>(Parent, Child, leg_right, foot_right)?;

    for name in world.relation_lookup::<true, Traverse<true, Child, Lookup<true, &&str>>>(corpus) {
        println!("Body part: {}", *name);
    }

    for (name, _) in world
        .relation_lookup::<true, Traverse<true, Child, Lookup<true, (&&str, Include<CanHold>)>>>(
            corpus,
        )
    {
        println!("Can hold: {}", *name);
    }

    for (name, _) in world
        .relation_lookup::<true, Traverse<true, Child, Lookup<true, (&&str, Include<CanStand>)>>>(
            corpus,
        )
    {
        println!("Can stand: {}", *name);
    }

    Ok(())
}
