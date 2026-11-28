const KEYWORDS: [&str; 5] = ["org_id", "primary", "foreign", "unique", "check"];

fn migrated() -> Vec<(String, Vec<String>)> {
    let mut tables: Vec<(String, Vec<String>)> = Vec::new();
    for (_, sql) in openworks_db::MIGRATIONS {
        for statement in sql.split(';').map(str::trim) {
            if let Some(rest) = statement.strip_prefix("create table if not exists works.") {
                let (name, body) = rest.split_once(" (").unwrap();
                let columns = body
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                    .filter(|word| !KEYWORDS.contains(word) && *word != ")")
                    .map(str::to_owned)
                    .collect();
                tables.push((name.to_owned(), columns));
            } else if let Some(rest) = statement.strip_prefix("alter table works.") {
                let (name, column) = rest.split_once(" add column if not exists ").unwrap();
                let column = column.split_whitespace().next().unwrap().to_owned();
                tables
                    .iter_mut()
                    .find(|(table, _)| table == name)
                    .unwrap()
                    .1
                    .push(column);
            }
        }
    }
    tables
}

#[test]
fn the_export_reads_every_migrated_table_with_every_column_but_org_id() {
    let exported: Vec<(String, Vec<String>)> = openworks_db::TABLES
        .iter()
        .map(|table| {
            let columns = table.columns.iter().map(|column| (*column).to_owned());
            (table.name.to_owned(), columns.collect())
        })
        .collect();

    assert_eq!(exported, migrated());
}
