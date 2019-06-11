#![feature(async_await)]

extern crate analysis_to_transgress as a2t;

use a2t::db::{Children, Defs};
use a2t::inspect::Inspected;
use rls_data::Id;

#[runtime::test]
async fn basic() -> a2t::Result<()> {
    let _ = pretty_env_logger::try_init();

    a2t::tools::ensure_analysis("../test-crate".as_ref()).await?;
    let path = a2t::tools::analysis_path("../test-crate".as_ref());
    let _analysis: Vec<rls_data::Analysis> = a2t::tools::fetch_analysis(&path).await?;

    /*
    for a in &analysis[analysis.len() - 1..] {
        let children = Children::new(a);
        let defs = Defs::new(a);
        println!(
            "=== {} {} ===",
            a.prelude.as_ref().expect("no prelude").crate_id.name,
            a.version.as_ref().expect("no version")
        );
        for def in a.defs.iter().filter(|def| def.parent.is_none()) {
            print_tree(def.id, &children, &defs, 0);
            println!();
        }
    }
    */
    /*

    let analysis: Vec<rls_data::Analysis> =
        a2t::tools::fetch_analysis("/Users/james/Dev/rust/slurm/build/x86_64-unknown-linux-gnu/test/run-make-fulldeps/save-analysis/save-analysis/save-analysis".as_ref()).await?;

    for a in &analysis {
        let children = Children::new(a);
        let defs = Defs::new(a);
        println!(
            "=== {} {} ===",
            a.prelude.as_ref().expect("no prelude").crate_id.name,
            a.version.as_ref().expect("no version")
        );
        for def in a.defs.iter().filter(|def| def.parent.is_none()) {
            print_tree(def.id, &children, &defs, 0);
        }
    }
    */

    Ok(())
}

fn print_tree(id: Id, c: &Children, defs: &Defs, indent: usize) {
    for _ in 0..indent {
        print!("    ");
    }
    if let Some(def) = defs.0.get(&id) {
        if def.sig.is_none() {
            return;
        }
        let children = c.children(id);

        println!("{}", Inspected(*def));
        for child in children {
            print_tree(*child, c, defs, indent + 1);
        }
    } else {
        println!("[non_def];")
    }
}
