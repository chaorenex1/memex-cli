use memex_core::AppContext;

pub fn handle(_ctx: &AppContext) -> Result<(), String> {
    println!("memex policies: ok");
    Ok(())
}
