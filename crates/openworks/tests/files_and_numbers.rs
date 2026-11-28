use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Barrier, Condvar, Mutex};
use std::thread;

use openworks::{Caller, Files, Host, Openworks, Period, Request, Response, Series, Via};
use openworks_core::{Change, Declaration, Fail, Hook, Id, Mount, Reach, Role, Row, Tx, Value};
use openworks_db::{attachments, members, series};

const T0: i64 = 1_700_000_000_000;
const JAN_2024: i64 = 1_704_067_200_000;

#[derive(Default)]
struct Committed {
    statements: Vec<(&'static str, Vec<Value>)>,
    counters: HashMap<String, i64>,
    attachments: Vec<Row>,
}

#[derive(Default)]
struct SharedHost {
    committed: Mutex<Committed>,
    locked: Mutex<HashSet<String>>,
    unlocked: Condvar,
    files: Mutex<HashMap<String, Vec<u8>>>,
    next_id: AtomicU32,
    clock: AtomicI64,
}

struct Pending<'a> {
    host: &'a SharedHost,
    statements: Vec<(&'static str, Vec<Value>)>,
    counters: HashMap<String, i64>,
    attachments: Vec<Row>,
    held: Vec<String>,
}

impl Pending<'_> {
    fn lock_row(&mut self, row: String) {
        if self.held.contains(&row) {
            return;
        }
        let mut locked = self.host.locked.lock().unwrap();
        while locked.contains(&row) {
            locked = self.host.unlocked.wait(locked).unwrap();
        }
        locked.insert(row.clone());
        self.held.push(row);
    }
}

impl Tx for Pending<'_> {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        self.statements.push((sql, params.to_vec()));
        Ok(match sql {
            series::NEXT => {
                let row = format!("{params:?}");
                self.lock_row(row.clone());
                let committed = self.host.committed.lock().unwrap().counters.get(&row).copied();
                let last = self.counters.get(&row).copied().or(committed).unwrap_or(0) + 1;
                self.counters.insert(row, last);
                vec![vec![Value::Int(last)]]
            }
            members::FIND => vec![vec![Value::Text(Role::Member.name().to_owned())]],
            attachments::ADD => {
                self.attachments.push(params.to_vec());
                Vec::new()
            }
            attachments::LIST => {
                let committed = &self.host.committed.lock().unwrap().attachments;
                committed
                    .iter()
                    .chain(&self.attachments)
                    .filter(|row| row[0] == params[0] && row[2] == params[1])
                    .map(|row| vec![row[1].clone(), row[3].clone(), row[4].clone()])
                    .collect()
            }
            _ => Vec::new(),
        })
    }
}

impl Files for SharedHost {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail> {
        self.files.lock().unwrap().insert(key.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail> {
        Ok(self.files.lock().unwrap().get(key).cloned())
    }
}

impl Host for &SharedHost {
    fn who(&self, credential: &str) -> Option<Caller> {
        let org = match credential {
            "member" => "o1",
            "other" => "o2",
            _ => return None,
        };
        Some(Caller {
            person: Id("p1".to_owned()),
            email: format!("{credential}@o.test"),
            org: Id(org.to_owned()),
            via: Via::Person,
        })
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        let mut pending = Pending {
            host: self,
            statements: Vec::new(),
            counters: HashMap::new(),
            attachments: Vec::new(),
            held: Vec::new(),
        };
        let done = work(&mut pending);
        if done.is_ok() {
            let mut committed = self.committed.lock().unwrap();
            committed.statements.append(&mut pending.statements);
            committed.counters.extend(pending.counters.drain());
            committed.attachments.append(&mut pending.attachments);
        }
        let mut locked = self.locked.lock().unwrap();
        for row in &pending.held {
            locked.remove(row);
        }
        self.unlocked.notify_all();
        done
    }

    fn id(&self) -> Id {
        Id(format!("id{}", self.next_id.fetch_add(1, Ordering::SeqCst) + 1))
    }

    fn now(&self) -> i64 {
        T0 + self.clock.load(Ordering::SeqCst)
    }

    fn files(&self) -> &dyn Files {
        *self
    }

    fn name(&self, _: &str) -> Option<String> {
        None
    }
}

impl SharedHost {
    fn count(&self, sql: &str) -> usize {
        let committed = self.committed.lock().unwrap();
        committed.statements.iter().filter(|(run, _)| *run == sql).count()
    }
}

const JOB: &str = "insert into works.jobs (org_id, number) values ($1, $2)";

const JOBS: Series = Series {
    key: "jobs",
    pattern: "J-{period}-{n}",
    period: Period::Year,
};

const OPEN_JOB: Declaration = openworks_core::write!("open", needs: Write, queries: [JOB, series::NEXT], |call, tx| {
    let number = JOBS.next(call, tx)?;
    if call.body == "fail" {
        return Err(Fail::Invalid("refused after numbering".to_owned()));
    }
    tx.run(JOB, &[Value::Text(call.who.org.0.clone()), Value::Text(number.clone())])?;
    Ok(number)
});

struct Veto;

impl Hook for Veto {
    fn before_commit(&self, change: &Change) -> Result<(), Fail> {
        if change.body == "veto" {
            return Err(Fail::Invalid("vetoed".to_owned()));
        }
        Ok(())
    }
}

fn openworks(host: &SharedHost) -> Openworks<&SharedHost> {
    Openworks::new(host)
        .mount(Mount::new("/numbered", "numbered").operation(&OPEN_JOB, Reach::Org))
        .hook(Veto)
        .build()
        .unwrap()
}

fn send(openworks: &Openworks<&SharedHost>, path: &str, credential: &str, key: Option<&str>, body: &str) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: credential.to_owned(),
        idempotency_key: key.map(str::to_owned),
        body: body.to_owned(),
    })
}

fn open_job(openworks: &Openworks<&SharedHost>, credential: &str, body: &str) -> Response {
    send(openworks, "/numbered/open", credential, None, body)
}

#[test]
fn two_concurrent_writes_get_two_numbers() {
    let host = SharedHost::default();
    let start = Barrier::new(2);

    let mut numbers: Vec<String> = thread::scope(|scope| {
        let writers: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let openworks = openworks(&host);
                    start.wait();
                    open_job(&openworks, "member", "a job")
                })
            })
            .collect();
        writers
            .into_iter()
            .map(|writer| {
                let answer = writer.join().unwrap();
                assert_eq!(answer.status, 200, "{answer:?}");
                answer.body
            })
            .collect()
    });
    numbers.sort();

    assert_eq!(numbers, ["J-2023-1", "J-2023-2"]);
    assert_eq!(host.count(JOB), 2);
}

#[test]
fn a_rolled_back_write_burns_no_number() {
    let host = SharedHost::default();
    let openworks = openworks(&host);

    assert_eq!(open_job(&openworks, "member", "a job").body, "J-2023-1");
    assert_eq!(open_job(&openworks, "member", "fail").status, 400);
    assert_eq!(open_job(&openworks, "member", "veto").status, 400);
    assert_eq!(open_job(&openworks, "member", "a job").body, "J-2023-2");
    assert_eq!(host.count(series::NEXT), 2);
}

#[test]
fn a_series_counts_per_org_and_starts_again_each_period() {
    let host = SharedHost::default();
    let openworks = openworks(&host);

    assert_eq!(open_job(&openworks, "member", "a job").body, "J-2023-1");
    assert_eq!(open_job(&openworks, "other", "a job").body, "J-2023-1");
    host.clock.store(JAN_2024 - T0, Ordering::SeqCst);
    assert_eq!(open_job(&openworks, "member", "a job").body, "J-2024-1");
    assert_eq!(open_job(&openworks, "member", "a job").body, "J-2024-2");
}

#[test]
fn a_file_goes_to_the_hosts_files_and_never_into_an_answer() {
    let host = SharedHost::default();
    let openworks = openworks(&host);

    let attached = send(
        &openworks,
        "/files/attach",
        "member",
        Some("k1"),
        "job-7\nplan.txt\nsecret plan",
    );
    send(
        &openworks,
        "/files/attach",
        "member",
        Some("k2"),
        "job-8\nother.txt\nelsewhere",
    );
    let listed = send(&openworks, "/files/list", "member", None, "job-7");

    assert_eq!(attached.status, 200);
    let id = attached.body;
    assert_eq!(listed.body, format!("{id}\tplan.txt\t11"));
    assert!(!listed.body.contains("secret"));
    assert_eq!(host.get(&format!("o1/{id}")).unwrap(), Some(b"secret plan".to_vec()));
    assert_eq!(send(&openworks, "/files/list", "other", None, "job-7").body, "");
    assert_eq!(host.count(openworks_db::AUDIT), 2);
    assert_eq!(host.count(openworks_db::EVENT), 2);
}

#[test]
fn a_file_without_a_tag_or_name_is_refused() {
    let host = SharedHost::default();
    let openworks = openworks(&host);

    for body in ["\nplan.txt\nx", "job-7\n\nx", "job-7"] {
        assert_eq!(
            send(&openworks, "/files/attach", "member", Some(body), body).status,
            400,
            "{body:?}"
        );
    }
    assert!(host.files.lock().unwrap().is_empty());
    assert_eq!(host.count(attachments::ADD), 0);
}
