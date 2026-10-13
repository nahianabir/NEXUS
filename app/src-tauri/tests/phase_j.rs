//! Phase J: document intelligence.

mod common;

use nexus_core::intelligence;

#[test]
fn end_to_end_pipe_table_to_csv() {
    let text = "intro\n| name | age |\n| ---- | --- |\n| Ada  | 36  |\n| Bob  | 24  |\nafter\n";
    let tables = intelligence::tables::extract(text);
    assert_eq!(tables.len(), 1);
    let t = &tables[0];
    assert_eq!(t.headers, vec!["name", "age"]);
    let csv = intelligence::csv::to_csv(t, &intelligence::csv::CsvOptions::default());
    assert!(csv.contains("name,age"));
    assert!(csv.contains("Ada,36"));
    let md = intelligence::tables::render_markdown(t);
    assert!(md.contains("| name | age |"));
}

#[test]
fn end_to_end_diff_summarises_changes() {
    let a = "alpha\nbeta\ngamma";
    let b = "alpha\nbeta\ndelta";
    let d = intelligence::diff::diff_lines(a, b);
    assert_eq!(d.same, 2);
    assert_eq!(d.added, 1);
    assert_eq!(d.removed, 1);
}
