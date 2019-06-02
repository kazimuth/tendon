extern crate analysis_to_transgress as a2t;
use rls_analysis::{AnalysisHost, Def, Id, Target};
use std::io::Write;

#[test]
fn basic() -> a2t::Result<()> {
    let _ = pretty_env_logger::try_init();

    a2t::tools::ensure_analysis("../test-crate".as_ref())?;

    let host = AnalysisHost::new(Target::Debug);
    host.reload("../test-crate".as_ref(), "../test-crate".as_ref())?;

    let roots = host.def_roots()?;
    let krate: Id = roots
        .into_iter()
        .filter_map(|(id, name)| if name == "test_crate" { Some(id) } else { None })
        .next()
        .unwrap();

    /*
    let mut out = std::fs::File::create("deps.gv")?;
    writeln!(out, "digraph deps {{")?;

    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    queue.push_back(krate);
    while queue.len() != 0 {
        print!(".");
        let par = queue.pop_front().unwrap();
        if visited.contains(&par) {
            continue;
        }
        visited.insert(par);
        let pardef = host.get_def(par)?;
        writeln!(out, "{} [label={:?}];", short(par), pardef.qualname)?;
        host.for_each_child_def(krate, |child: Id, _: &Def| {
            queue.push_back(child);
            writeln!(out, "{} -> {};", short(par), short(child)).unwrap();
        })?;
        println!("`{}` {}", pardef.qualname, pardef.value);
        dbg!(pardef);
    }
    writeln!(out, "}}")?;

    Ok(())
    */
}

fn short(id: Id) -> String {
    let result = format!("{:?}", id);
    result[3..result.len() - 1].into()
}
