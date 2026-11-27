const MONEY: &[&str] = &[
    "money", "price", "cost", "amount", "currency", "wage", "salary", "payroll", "invoice", "budget", "fee", "rate",
    "tax", "pay", "paid", "cents", "total", "balance",
];

fn money_names(sql: &str) -> Vec<String> {
    sql.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|name| name.split('_').any(|part| MONEY.contains(&part)))
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_migration_names_money() {
    for (name, sql) in openworks_db::MIGRATIONS {
        assert_eq!(money_names(sql), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn a_money_column_is_caught() {
    let sql = "create table works.jobs (org_id text, unit_price bigint, Rate int)";

    assert_eq!(money_names(sql), ["unit_price", "rate"]);
}
