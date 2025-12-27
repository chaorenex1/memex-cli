use memex_core::AppContext;

pub fn handle(_ctx: &AppContext) -> Result<(), String> {
    println!("memex memory: ok");
    Ok(())
}
