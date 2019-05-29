extern crate analysis_to_transgress as a2t;

#[test]
fn ensure_analysis() -> a2t::Result<()> {
    let _ = pretty_env_logger::try_init();

    a2t::tools::ensure_analysis("../test-crate".as_ref())?;

    Ok(())

    /*
    let mut host = rls_analysis::AnalysisHost::new(rls_analysis::Target::Debug);
    host.reload("../test-crate".as_ref(), "../test-crate".as_ref())
        .unwrap();

    println!("{:?}", host.def_roots());
    */
}
