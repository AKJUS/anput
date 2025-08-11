use crate::{components::Visible, game::Inputs};
use anput::{
    commands::CommandBuffer,
    systems::SystemContext,
    third_party::intuicio_data::managed::ManagedLazy,
    universe::{Local, Res},
};
use anput_physics::{
    collisions::{CollisionMask, CollisionProfile, ContactDetection},
    components::{
        BodyDensityFieldRelation, BodyParentRelation, BodyParticleRelation, ExternalForces,
        LinearVelocity, Mass, ParticleMaterial, PhysicsBody, PhysicsParticle, Position,
    },
    density_fields::{DensityFieldBox, sphere::SphereDensityField},
    third_party::vek::{Rgba, Vec2},
};
use send_wrapper::SendWrapper;
use spitfire_draw::utils::Vertex;
use spitfire_glow::graphics::Graphics;
use std::error::Error;

pub fn control_bodies(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (mut commands, graphics, inputs, mut spawn_bodies) = context.fetch::<(
        Res<true, &mut CommandBuffer>,
        Res<true, &SendWrapper<ManagedLazy<Graphics<Vertex>>>>,
        Res<true, &SendWrapper<Inputs>>,
        Local<true, &mut SpawnBodies>,
    )>()?;

    let trigger = inputs.mouse_trigger.get();
    if trigger.is_idle() {
        return Ok(());
    }

    let graphics = graphics.read().unwrap();
    let xy = inputs.mouse_xy.get();
    let screen_coords = graphics
        .state
        .main_camera
        .screen_matrix()
        .mul_point(xy.into());
    let world_coords = graphics
        .state
        .main_camera
        .world_matrix()
        .inverted()
        .mul_point(screen_coords);

    if trigger.is_pressed() {
        spawn_bodies.drag_position = Some(world_coords);
    }
    if trigger.is_released() {
        if let Some(drag_position) = spawn_bodies.drag_position.take() {
            let direction = world_coords - drag_position;

            commands.schedule(move |world| {
                let ball = world
                    .spawn((
                        PhysicsBody,
                        PhysicsParticle,
                        DensityFieldBox::new(SphereDensityField::<true>::new_hard(1.0, 50.0)),
                        CollisionProfile::default().with_block(CollisionMask::flag(0)),
                        ContactDetection::default(),
                        Mass::new(1.0),
                        Position::new(world_coords),
                        LinearVelocity::new(direction),
                        ExternalForces::default(),
                        ParticleMaterial::default(),
                        Rgba::<f32>::red(),
                        Visible,
                    ))
                    .unwrap();
                world
                    .relate::<true, _>(BodyParentRelation, ball, ball)
                    .unwrap();
                world
                    .relate::<true, _>(BodyDensityFieldRelation, ball, ball)
                    .unwrap();
                world
                    .relate::<true, _>(BodyParticleRelation, ball, ball)
                    .unwrap();
            });
        }
    }

    Ok(())
}

#[derive(Default)]
pub struct SpawnBodies {
    drag_position: Option<Vec2<f32>>,
}
