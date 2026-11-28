mod memory;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Files, Role, Row, Tx, Value};
use openworks_db::{AUDIT, EVENT, TABLES, members};

#[derive(Clone, Default)]
struct Store {
    reads: Vec<(&'static str, Vec<Value>)>,
    trail: Vec<Value>,
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn rows(table: &str) -> Vec<Row> {
    match table {
        "orgs" => vec![vec![text("First org"), Value::Int(7)]],
        "clients" => vec![
            vec![text("c1"), text("Acme, \"North\""), Value::Null, Value::Null],
            vec![text("c2"), text("Bolt"), text("ref-2"), Value::Int(9)],
        ],
        "attachments" => vec![
            vec![
                text("f1"),
                text("visit"),
                text("check.txt"),
                Value::Int(9),
                Value::Int(1),
            ],
            vec![
                text("f2"),
                text("visit"),
                text("lost.txt"),
                Value::Int(4),
                Value::Int(2),
            ],
        ],
        "job_order_lines" => vec![vec![text("r1"), Value::Int(1), text("Set out\nthe yard")]],
        _ => Vec::new(),
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(match sql {
            members::FIND => Role::ALL
                .iter()
                .filter(|role| p[1] == text(role.name()))
                .map(|role| vec![text(role.name())])
                .collect(),
            AUDIT => {
                self.trail.push(p[3].clone());
                Vec::new()
            }
            EVENT => {
                self.trail.push(p[2].clone());
                Vec::new()
            }
            _ => match TABLES.iter().find(|table| table.select == sql) {
                Some(table) => {
                    self.reads.push((sql, p.to_vec()));
                    rows(table.name)
                }
                None => Vec::new(),
            },
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn u16_at(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn entries(zip: &[u8]) -> Vec<(String, u32, Vec<u8>)> {
    let mut entries = Vec::new();
    let mut at = 0;
    while u32_at(zip, at) == 0x0403_4b50 {
        assert_eq!(u16_at(zip, at + 8), 0, "stored");
        let crc = u32_at(zip, at + 14);
        let size = usize::try_from(u32_at(zip, at + 18)).unwrap();
        let name_length = u16_at(zip, at + 26);
        let name = &zip[at + 30..at + 30 + name_length];
        let data = &zip[at + 30 + name_length..at + 30 + name_length + size];
        entries.push((String::from_utf8(name.to_vec()).unwrap(), crc, data.to_vec()));
        at += 30 + name_length + size;
    }
    let end = zip.len() - 22;
    assert_eq!(u32_at(zip, end), 0x0605_4b50);
    assert_eq!(u16_at(zip, end + 10), entries.len());
    assert_eq!(usize::try_from(u32_at(zip, end + 16)).unwrap(), at);
    entries
}

#[test]
fn an_owner_exports_the_company_as_a_zip_of_one_csv_per_table_and_the_files() {
    let host = TestHost::default();
    host.files.put("o1/f1", b"123456789").unwrap();
    let openworks = Openworks::new(&host).build().unwrap();

    let key = ok(send(&openworks, "/exports/create", "owner", ""));

    assert_eq!(key, "o1/exports/id1.zip");
    let zip = host.files.get(&key).unwrap().unwrap();
    let entries = entries(&zip);
    let names: Vec<&str> = entries.iter().map(|(name, _, _)| name.as_str()).collect();
    let mut expected: Vec<String> = TABLES.iter().map(|table| format!("{}.csv", table.name)).collect();
    let attachments = expected.iter().position(|name| name == "attachments.csv").unwrap();
    expected.insert(attachments, "files/f1".to_owned());
    assert_eq!(names, expected);

    let entry = |name: &str| {
        let (_, crc, data) = entries.iter().find(|(entry, _, _)| entry == name).unwrap();
        (*crc, String::from_utf8(data.clone()).unwrap())
    };
    assert_eq!(entry("files/f1"), (0xCBF4_3926, "123456789".to_owned()));
    assert_eq!(entry("orgs.csv").1, "name,created_at\r\nFirst org,7\r\n");
    assert_eq!(
        entry("clients.csv").1,
        "client_id,name,host_ref,deactivated_at\r\nc1,\"Acme, \"\"North\"\"\",,\r\nc2,Bolt,ref-2,9\r\n"
    );
    assert_eq!(
        entry("job_order_lines.csv").1,
        "job_order_id,line,body\r\nr1,1,\"Set out\nthe yard\"\r\n"
    );
    assert_eq!(entry("stages.csv").1, "stage_key,name,kind,sort\r\n");

    let store = host.store();
    assert_eq!(store.reads.len(), TABLES.len());
    assert!(store.reads.iter().all(|(_, params)| *params == [text("o1")]));
    assert_eq!(store.trail, [text("exports.create"), text("exports.create")]);
}

#[test]
fn only_an_owner_exports() {
    for role in [Role::Manager, Role::Member, Role::Viewer] {
        let host = TestHost::default();
        let openworks = Openworks::new(&host).build().unwrap();

        let answered = send(&openworks, "/exports/create", role.name(), "");

        assert_eq!((answered.status, answered.body.as_str()), (403, "needs settings"));
        assert!(host.files.0.lock().unwrap().is_empty());
    }
}
