#[test]
fn use_analysis() {
    let _ = pretty_env_logger::try_init();

    let mut host = rls_analysis::AnalysisHost::new(rls_analysis::Target::Debug);
    host.reload("../test-crate".as_ref(), "../test-crate".as_ref())
        .unwrap();

    println!("{:?}", host.def_roots());
}
