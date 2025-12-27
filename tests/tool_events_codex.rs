use memex_core::AppContext;

#[test]
fn integration_smoke() {
    let ctx = AppContext::new(memex_core::config::load::load_default());
    assert!(ctx.run().is_ok());
}
