pub mod doctor;
pub mod memory;
pub mod policies;
pub mod run;

use memex_core::AppContext;

pub fn dispatch(ctx: &AppContext, args: &[String]) -> Result<(), String> {
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("run");
    match cmd {
        "run" => run::handle(ctx),
        "doctor" => doctor::handle(ctx),
        "memory" => memory::handle(ctx),
        "policies" => policies::handle(ctx),
        _ => Err(format!("unknown command: {}", cmd)),
    }
}
