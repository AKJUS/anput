use anput::{
    scheduler::{GraphScheduler, GraphSchedulerPlugin},
    systems::SystemContext,
    universe::{Res, Universe},
};
use moirai::jobs::Jobs;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Gold(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Food(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Heat(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    let mut universe = Universe::default().with_plugin(
        GraphSchedulerPlugin::<true>::default()
            .resource(Gold(1000))
            .resource(Food(500))
            .resource(Heat(20))
            .plugin_setup(|plugin| {
                plugin
                    .system_setup(generate_income, |system| system.name("generate_income"))
                    .system_setup(harvest_food, |system| system.name("harvest_food"))
            })
            .plugin_setup(|plugin| {
                plugin
                    .system_setup(consume_food, |system| system.name("consume_food"))
                    .system_setup(increase_heat, |system| system.name("increase_heat"))
            }),
    );
    // Create jobs runner.
    let jobs = Jobs::default();
    // Create a scheduler instance that will run universe systems.
    let scheduler = GraphScheduler::<true>;

    // Perform single frame universe systems run.
    scheduler.run(&jobs, &mut universe)?;

    Ok(())
}

fn generate_income(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut gold = context.fetch::<Res<true, &mut Gold>>()?;

    gold.0 += 200;
    println!("Income generated during summer. Gold now: {}", gold.0);

    Ok(())
}

fn harvest_food(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut food = context.fetch::<Res<true, &mut Food>>()?;

    food.0 += 100;
    println!("Food harvested. Food now: {}", food.0);

    Ok(())
}

fn consume_food(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut food = context.fetch::<Res<true, &mut Food>>()?;

    if food.0 >= 50 {
        food.0 -= 50;
        println!("Food consumed. Winter survived!");
    } else {
        println!("Not enough food to survive the winter!")
    }

    Ok(())
}

fn increase_heat(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut heat = context.fetch::<Res<true, &mut Heat>>()?;

    heat.0 += 10;
    println!("Heat increased. Heat now: {}", heat.0);

    Ok(())
}
