use anput_physics::third_party::vek::{Aabb, Vec2};
use spitfire_draw::utils::Vertex;
use spitfire_glow::graphics::Graphics;

pub fn screen_aabb(graphics: &Graphics<Vertex>) -> Aabb<f32> {
    graphics
        .state
        .main_camera
        .world_polygon()
        .into_iter()
        .map(|point| Aabb {
            min: point.with_z(0.0),
            max: point.with_z(0.0),
        })
        .reduce(|accum, value| accum.union(value))
        .unwrap()
}

pub fn aabb_vertices(aabb: &Aabb<f32>) -> [Vec2<f32>; 4] {
    [
        Vec2::<f32>::new(aabb.min.x, aabb.min.y),
        Vec2::<f32>::new(aabb.max.x, aabb.min.y),
        Vec2::<f32>::new(aabb.max.x, aabb.max.y),
        Vec2::<f32>::new(aabb.min.x, aabb.max.y),
    ]
}
