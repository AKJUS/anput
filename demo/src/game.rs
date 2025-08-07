use crate::{
    components::{PlayerControlled, Visible},
    control_player::control_player,
    diagnostics::ChromeTracing,
    renderers::{
        contacts_renderer::render_contacts, density_field_renderer::render_density_fields,
    },
};
use anput::{
    scheduler::{GraphScheduler, GraphSchedulerPlugin, SystemName, SystemSubsteps},
    third_party::{anput_jobs::Jobs, intuicio_data::managed::ManagedLazy},
    universe::Universe,
};
use anput_physics::{
    PhysicsPlugin,
    collisions::{CollisionMask, CollisionProfile, ContactDetection},
    components::{
        BodyDensityFieldRelation, BodyParentRelation, BodyParticleRelation, ExternalForces,
        LinearVelocity, Mass, PhysicsBody, PhysicsParticle, Position,
    },
    density_fields::{DensityFieldBox, aabb::AabbDensityField, sphere::SphereDensityField},
    queries::shape::ShapeOverlapQuery,
    third_party::vek::{Aabb, Rgba, Vec3},
};
use glutin::{
    event::{Event, MouseButton, VirtualKeyCode},
    window::Window,
};
use send_wrapper::SendWrapper;
use spitfire_draw::{
    context::DrawContext,
    draw_buffer::DrawBuffer,
    pixels::Pixels,
    sprite::Sprite,
    utils::{Drawable, ShaderRef, Vertex},
};
use spitfire_glow::{
    app::{AppControl, AppState},
    graphics::{Graphics, Shader},
    renderer::{GlowBlending, GlowTextureFiltering},
};
use spitfire_gui::{context::GuiContext, interactions::GuiInteractionsInputs};
use spitfire_input::{
    ArrayInputCombinator, CardinalInputCombinator, InputActionRef, InputAxisRef, InputConsume,
    InputContext, InputMapping, VirtualAction, VirtualAxis,
};
use std::{
    fs::File,
    sync::{Arc, mpsc::channel},
    time::{Duration, Instant},
};

pub const PIXEL_SIZE: u32 = 10;

pub struct Game {
    universe: Universe,
    jobs: Jobs,
    scheduler: GraphScheduler<true>,
    fixed_step_timer: Instant,
    variable_step_timer: Instant,
    exit_game: InputActionRef,
    #[cfg(debug_assertions)]
    tracing: Option<ChromeTracing>,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            universe: Default::default(),
            jobs: Jobs::new(0),
            scheduler: Default::default(),
            fixed_step_timer: Instant::now(),
            variable_step_timer: Instant::now(),
            exit_game: Default::default(),
            #[cfg(debug_assertions)]
            tracing: None,
        }
    }
}

impl AppState<Vertex> for Game {
    fn on_init(&mut self, graphics: &mut Graphics<Vertex>, _: &mut AppControl) {
        #[cfg(debug_assertions)]
        let (jobs_sender, scheduler_sender) = {
            let file = File::create("./trace.json").unwrap();
            let (jobs_sender, jobs_receiver) = channel();
            let (scheduler_sender, scheduler_receiver) = channel();
            self.tracing = Some(ChromeTracing::new(file, jobs_receiver, scheduler_receiver));
            (jobs_sender, scheduler_sender)
        };

        graphics.state.color = [0.15, 0.15, 0.15, 1.0];
        graphics.state.main_camera.screen_alignment = 0.5.into();

        let mut draw = DrawContext::default();
        let mut gui = GuiContext::default();
        let mut input_context = InputContext::default();
        let mut inputs = Inputs::default();

        draw.shaders.insert(
            "color".into(),
            graphics
                .shader(Shader::COLORED_VERTEX_2D, Shader::PASS_FRAGMENT)
                .unwrap(),
        );
        draw.shaders.insert(
            "image".into(),
            graphics
                .shader(Shader::TEXTURED_VERTEX_2D, Shader::TEXTURED_FRAGMENT)
                .unwrap(),
        );
        draw.shaders.insert(
            "text".into(),
            graphics
                .shader(Shader::TEXT_VERTEX, Shader::TEXT_FRAGMENT)
                .unwrap(),
        );

        gui.interactions.engine.deselect_when_no_button_found = true;
        gui.texture_filtering = GlowTextureFiltering::Linear;

        self.exit_game = InputActionRef::default();
        let pointer_x = InputAxisRef::default();
        let pointer_y = InputAxisRef::default();
        let pointer_trigger = InputActionRef::default();
        let movement_left = InputActionRef::default();
        let movement_right = InputActionRef::default();
        let movement_up = InputActionRef::default();
        let movement_down = InputActionRef::default();

        inputs.movement = CardinalInputCombinator::new(
            movement_left.clone(),
            movement_right.clone(),
            movement_up.clone(),
            movement_down.clone(),
        );
        gui.interactions.inputs = GuiInteractionsInputs {
            pointer_position: ArrayInputCombinator::new([pointer_x.clone(), pointer_y.clone()]),
            pointer_trigger: pointer_trigger.clone(),
            ..Default::default()
        };

        input_context.push_mapping(
            InputMapping::default()
                .consume(InputConsume::Hit)
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::Escape),
                    self.exit_game.clone(),
                )
                .axis(VirtualAxis::MousePositionX, pointer_x)
                .axis(VirtualAxis::MousePositionY, pointer_y)
                .action(
                    VirtualAction::MouseButton(MouseButton::Left),
                    pointer_trigger,
                )
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::A),
                    movement_left.clone(),
                )
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::D),
                    movement_right.clone(),
                )
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::W),
                    movement_up.clone(),
                )
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::S),
                    movement_down.clone(),
                )
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::Left),
                    movement_left,
                )
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::Right),
                    movement_right,
                )
                .action(VirtualAction::KeyButton(VirtualKeyCode::Up), movement_up)
                .action(
                    VirtualAction::KeyButton(VirtualKeyCode::Down),
                    movement_down,
                ),
        );

        #[cfg(debug_assertions)]
        {
            self.jobs.diagnostics = Some(Arc::new(jobs_sender));
            self.scheduler.diagnostics = Some(Arc::new(scheduler_sender));
        }

        self.universe = Universe::default()
            .with_basics(10240, 10240)
            .unwrap()
            .with_resource(Clock::default())
            .unwrap()
            .with_resource(SendWrapper::new(draw))
            .unwrap()
            .with_resource(SendWrapper::new(gui))
            .unwrap()
            .with_resource(input_context)
            .unwrap()
            .with_resource(SendWrapper::new(
                Pixels::simple(
                    graphics.state.main_camera.screen_size.x as u32 / PIXEL_SIZE,
                    graphics.state.main_camera.screen_size.y as u32 / PIXEL_SIZE,
                    graphics,
                )
                .unwrap(),
            ))
            .unwrap()
            .with_resource(SendWrapper::new(inputs))
            .unwrap()
            .with_plugin(
                GraphSchedulerPlugin::<true>::default()
                    .name("root")
                    .plugin(GraphSchedulerPlugin::<true>::default().name("update"))
                    .plugin(
                        GraphSchedulerPlugin::<true>::default()
                            .name("fixed-step-update")
                            .plugin(
                                PhysicsPlugin::<true>::default()
                                    .shape_overlap_query(ShapeOverlapQuery {
                                        voxelization_size_limit: (PIXEL_SIZE * 3) as f32,
                                        ..Default::default()
                                    })
                                    .make(),
                            )
                            .system_setup(control_player, |system| system.name("control_player")),
                    )
                    .plugin(
                        GraphSchedulerPlugin::<true>::default()
                            .name("draw-pixels")
                            .system_setup(render_density_fields, |system| {
                                system.name("render_density_fields")
                            }),
                    )
                    .plugin(
                        GraphSchedulerPlugin::<true>::default()
                            .name("draw-world")
                            .system_setup(render_contacts, |system| system.name("render_contacts")),
                    )
                    .plugin(GraphSchedulerPlugin::<true>::default().name("draw-gui")),
            );

        let ground = self
            .universe
            .simulation
            .spawn((
                PhysicsBody,
                DensityFieldBox::new(AabbDensityField {
                    aabb: Aabb {
                        min: Vec3::new(-1000.0, 200.0, 0.0),
                        max: Vec3::new(1000.0, 400.0, 0.0),
                    },
                    density: 1.0,
                }),
                CollisionProfile::default().with_block(CollisionMask::flag(0)),
                ContactDetection::default(),
                Rgba::<f32>::new(0.0, 0.5, 0.0, 1.0),
                Visible,
            ))
            .unwrap();
        self.universe
            .simulation
            .relate::<true, _>(BodyParentRelation, ground, ground)
            .unwrap();
        self.universe
            .simulation
            .relate::<true, _>(BodyDensityFieldRelation, ground, ground)
            .unwrap();

        let ball = self
            .universe
            .simulation
            .spawn((
                PhysicsBody,
                PhysicsParticle,
                DensityFieldBox::new(SphereDensityField::<true>::new_hard(1.0, 50.0)),
                CollisionProfile::default().with_block(CollisionMask::flag(0)),
                ContactDetection::default(),
                Mass::new(1.0),
                Position::new(Vec3::new(0.0, 0.0, 0.0)),
                LinearVelocity::default(),
                ExternalForces::default(),
                Rgba::<f32>::yellow(),
                Visible,
                PlayerControlled,
            ))
            .unwrap();
        self.universe
            .simulation
            .relate::<true, _>(BodyParentRelation, ball, ball)
            .unwrap();
        self.universe
            .simulation
            .relate::<true, _>(BodyDensityFieldRelation, ball, ball)
            .unwrap();
        self.universe
            .simulation
            .relate::<true, _>(BodyParticleRelation, ball, ball)
            .unwrap();

        self.fixed_step_timer = Instant::now();
        self.variable_step_timer = Instant::now();
    }

    fn on_redraw(&mut self, graphics: &mut Graphics<Vertex>, control: &mut AppControl) {
        #[cfg(debug_assertions)]
        {
            self.tracing.as_mut().unwrap().frame_begin();
        }

        if self.exit_game.get().is_pressed() {
            control.close_requested = true;
        }

        let draw_buffer = DrawBuffer::new(graphics);
        let (graphics, _graphics_lifetime) = ManagedLazy::make(graphics);
        self.universe
            .resources
            .add((
                SendWrapper::new(graphics.clone()),
                SendWrapper::new(draw_buffer),
            ))
            .unwrap();

        {
            let mut clock = self.universe.resources.get_mut::<true, Clock>().unwrap();
            clock.fixed_step_timer = self.fixed_step_timer;
            clock.variable_step_timer = self.variable_step_timer;
            self.variable_step_timer = Instant::now();
        }

        {
            let pixels = &mut **self
                .universe
                .resources
                .get_mut::<true, SendWrapper<Pixels>>()
                .unwrap();
            let graphics = graphics.read().unwrap();
            let desired_width = graphics.state.main_camera.screen_size.x as u32 / PIXEL_SIZE;
            let desired_height = graphics.state.main_camera.screen_size.y as u32 / PIXEL_SIZE;
            if pixels.width() != desired_width as usize
                || pixels.height() != desired_height as usize
            {
                *pixels = Pixels::simple(desired_width, desired_height, &graphics).unwrap();
            }
        }

        self.scheduler
            .run_system(
                &self.jobs,
                &self.universe,
                self.universe
                    .systems
                    .find_with::<true, SystemName>(|name| name.as_str() == "update")
                    .unwrap(),
                SystemSubsteps::default(),
            )
            .unwrap();

        if self.fixed_step_timer.elapsed().as_millis() > 1000 / 30 {
            self.fixed_step_timer = Instant::now();

            self.scheduler
                .run_system(
                    &self.jobs,
                    &self.universe,
                    self.universe
                        .systems
                        .find_with::<true, SystemName>(|name| name.as_str() == "fixed-step-update")
                        .unwrap(),
                    SystemSubsteps::default(),
                )
                .unwrap();
        }

        {
            let draw = &mut **self
                .universe
                .resources
                .get_mut::<true, SendWrapper<DrawContext>>()
                .unwrap();
            let mut graphics = graphics.write().unwrap();
            draw.begin_frame(&mut graphics);
            draw.push_shader(&ShaderRef::name("image"));
            draw.push_blending(GlowBlending::Alpha);
        }

        self.scheduler
            .run_system(
                &self.jobs,
                &self.universe,
                self.universe
                    .systems
                    .find_with::<true, SystemName>(|name| name.as_str() == "draw-pixels")
                    .unwrap(),
                SystemSubsteps::default(),
            )
            .unwrap();

        {
            let draw = &mut **self
                .universe
                .resources
                .get_mut::<true, SendWrapper<DrawContext>>()
                .unwrap();
            let pixels = &mut **self
                .universe
                .resources
                .get_mut::<true, SendWrapper<Pixels>>()
                .unwrap();
            let mut graphics = graphics.write().unwrap();
            pixels.commit();
            Sprite::single(pixels.sprite_texture("u_image".into(), GlowTextureFiltering::Nearest))
                .size(graphics.state.main_camera.screen_size)
                .screen_space(true)
                .draw(draw, &mut *graphics);
            pixels.access_channels().fill([0, 0, 0, 255]);
        }

        self.scheduler
            .run_system(
                &self.jobs,
                &self.universe,
                self.universe
                    .systems
                    .find_with::<true, SystemName>(|name| name.as_str() == "draw-world")
                    .unwrap(),
                SystemSubsteps::default(),
            )
            .unwrap();

        {
            let mut graphics = graphics.write().unwrap();
            let mut draw_buffer = self
                .universe
                .resources
                .get_mut::<true, SendWrapper<DrawBuffer>>()
                .unwrap();
            draw_buffer.submit(&mut *graphics);
        }

        self.universe
            .resources
            .get_mut::<true, SendWrapper<GuiContext>>()
            .unwrap()
            .begin_frame();

        self.scheduler
            .run_system(
                &self.jobs,
                &self.universe,
                self.universe
                    .systems
                    .find_with::<true, SystemName>(|name| name.as_str() == "draw-gui")
                    .unwrap(),
                SystemSubsteps::default(),
            )
            .unwrap();

        {
            let draw = &mut **self
                .universe
                .resources
                .get_mut::<true, SendWrapper<DrawContext>>()
                .unwrap();
            let gui = &mut **self
                .universe
                .resources
                .get_mut::<true, SendWrapper<GuiContext>>()
                .unwrap();
            let mut graphics = graphics.write().unwrap();
            gui.end_frame(
                draw,
                &mut graphics,
                &ShaderRef::name("color"),
                &ShaderRef::name("image"),
                &ShaderRef::name("text"),
            );
            draw.end_frame();
        }

        self.universe
            .resources
            .get_mut::<true, InputContext>()
            .unwrap()
            .maintain();

        self.universe
            .resources
            .remove::<(
                SendWrapper<ManagedLazy<Graphics<Vertex>>>,
                SendWrapper<DrawBuffer>,
            )>()
            .unwrap();

        GraphScheduler::<true>::maintenance(&self.jobs, &mut self.universe);

        #[cfg(debug_assertions)]
        {
            self.tracing.as_mut().unwrap().frame_end();
            self.tracing.as_mut().unwrap().maintain();
        }
    }

    fn on_event(&mut self, event: Event<()>, _: &mut Window) -> bool {
        if let Event::WindowEvent { event, .. } = event {
            self.universe
                .resources
                .get_mut::<true, InputContext>()
                .unwrap()
                .on_event(&event);
        }
        true
    }
}

pub struct Clock {
    fixed_step_timer: Instant,
    variable_step_timer: Instant,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            fixed_step_timer: Instant::now(),
            variable_step_timer: Instant::now(),
        }
    }
}

impl Clock {
    pub fn fixed_step_elapsed(&self) -> Duration {
        self.fixed_step_timer.elapsed()
    }

    pub fn variable_step_elapsed(&self) -> Duration {
        self.variable_step_timer.elapsed()
    }
}

#[derive(Default)]
pub struct Inputs {
    pub movement: CardinalInputCombinator,
}
