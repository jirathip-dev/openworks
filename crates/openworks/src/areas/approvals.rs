use openworks_core::{Approval, Call, Declaration, Fail, Mount, Reach, Tx, Value, Via, Who, Word};
use openworks_db::approvals::{
    DECIDE as DECIDE_SQL, KIND, OPEN, PENDING as PENDING_SQL, PROPOSE, RECENT as RECENT_SQL, REQUEST as REQUEST_SQL,
    RESET as RESET_SQL, SAVE_KIND as SAVE_KIND_SQL,
};
use openworks_db::attachments::FIND as FILE;

use crate::files::lines;

/// Adds a company's own kind of approval from a body of its key, then `signature` when a yes needs a signature file.
pub const SAVE_KIND: Declaration =
    openworks_core::write!("save-kind", needs: Settings, queries: [SAVE_KIND_SQL], save_kind);

/// Asks for an approval from a body of `kind subject`, the subject any record by reference, then a note line.
pub const REQUEST: Declaration =
    openworks_core::create!("request", needs: Write, queries: [KIND, REQUEST_SQL], request);

/// Decides a pending approval from a body of `id yes|no [signature file]`, then a note line; an assistant only proposes.
pub const DECIDE: Declaration = openworks_core::write!(
    "decide",
    needs: Approve,
    queries: [OPEN, KIND, FILE, PROPOSE, DECIDE_SQL],
    decide
);

/// Resets the approved approval with the body's id to none, so its subject may be asked for again.
pub const RESET: Declaration = openworks_core::write!("reset", needs: Settings, queries: [RESET_SQL], reset);

/// Lists the pending approvals, a `id kind subject asked_by note proposed proposed_note` line each, tab-separated.
pub const PENDING: Declaration = openworks_core::read!("pending", needs: Read, queries: [PENDING_SQL], pending);

/// Lists the latest decisions, a `id kind subject state decided_by note signature` line each, tab-separated.
pub const RECENT: Declaration = openworks_core::read!("recent", needs: Read, queries: [RECENT_SQL], recent);

pub(crate) const JOB_ORDER: Approval = Approval {
    key: "job_order",
    needs: Word::Approve,
    signature: false,
};

struct Rule {
    needs: Word,
    signature: bool,
}

fn save_kind(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let (key, signature) = match call.body.split_whitespace().collect::<Vec<_>>()[..] {
        [key] => (key, false),
        [key, "signature"] => (key, true),
        _ => {
            return Err(Fail::Invalid(
                "a kind is a key, then `signature` when a yes needs one".to_owned(),
            ));
        }
    };
    if held(call).any(|kind| kind.key == key) {
        return Err(Fail::Invalid(format!("a pack or the core holds the kind {key}")));
    }
    tx.run(SAVE_KIND_SQL, &[org(call.who), text(key), text(yes_no(signature))])?;
    Ok(key.to_owned())
}

fn request(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let (head, note) = split(call.body)?;
    let [kind, subject] = head[..] else {
        return Err(Fail::Invalid("a request needs a kind and a subject".to_owned()));
    };
    rule(call, tx, kind)?;
    let id = (call.fresh)().0;
    let asked = tx.run(
        REQUEST_SQL,
        &[
            org(call.who),
            text(&id),
            text(kind),
            text(subject),
            text(&call.who.person.0),
            Value::Int(call.at),
            text(note),
        ],
    )?;
    if asked.is_empty() {
        return Err(Fail::Invalid(format!(
            "{kind} on {subject} is pending or approved already"
        )));
    }
    Ok(id)
}

fn decide(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let (head, note) = split(call.body)?;
    let (id, yes, signature) = match head[..] {
        [id, "yes"] => (id, true, None),
        [id, "yes", file] => (id, true, Some(file)),
        [id, "no"] => (id, false, None),
        _ => return Err(Fail::Invalid("a decision is an id, then yes or no".to_owned())),
    };
    if !yes && note.is_empty() {
        return Err(Fail::Invalid("a no needs a note".to_owned()));
    }
    let kind = match tx.run(OPEN, &[org(who), text(id)])?.first().map(Vec::as_slice) {
        Some([Value::Text(kind)]) => kind.clone(),
        None => return Err(Fail::NotFound),
        Some(_) => return Err(Fail::Store("a pending approval row is its kind".to_owned())),
    };
    let rule = rule(call, tx, &kind)?;
    if who.role.grants(rule.needs).is_none() {
        return Err(Fail::Needs(rule.needs));
    }
    let answer = yes_no(yes);
    if who.via == Via::Assistant {
        tx.run(
            PROPOSE,
            &[org(who), text(id), text(answer), text(&who.person.0), text(note)],
        )?;
        return Ok(format!("{id} proposed {answer}"));
    }
    match (rule.signature && yes, signature) {
        (true, None) => return Err(Fail::Invalid(format!("{kind} needs a signature file"))),
        (false, Some(_)) => return Err(Fail::Invalid("this decision takes no signature file".to_owned())),
        (_, Some(file)) if tx.run(FILE, &[org(who), text(file)])?.is_empty() => {
            return Err(Fail::Invalid(format!("no file {file}")));
        }
        _ => {}
    }
    let state = if yes { "approved" } else { "rejected" };
    tx.run(
        DECIDE_SQL,
        &[
            org(who),
            text(id),
            text(state),
            text(&who.person.0),
            Value::Int(call.at),
            text(note),
            signature.map_or(Value::Null, text),
        ],
    )?;
    Ok(format!("{id} {state}"))
}

fn reset(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = call.body.trim();
    if tx.run(RESET_SQL, &[org(call.who), text(id)])?.is_empty() {
        return Err(Fail::Invalid(format!("{id} is not approved")));
    }
    Ok(id.to_owned())
}

fn pending(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    Ok(lines(&tx.run(PENDING_SQL, &[org(call.who)])?))
}

fn recent(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    Ok(lines(&tx.run(RECENT_SQL, &[org(call.who)])?))
}

fn rule(call: &Call, tx: &mut dyn Tx, key: &str) -> Result<Rule, Fail> {
    if let Some(kind) = held(call).find(|kind| kind.key == key) {
        return Ok(Rule {
            needs: kind.needs,
            signature: kind.signature,
        });
    }
    match tx.run(KIND, &[org(call.who), text(key)])?.first().map(Vec::as_slice) {
        Some([Value::Text(signature)]) => Ok(Rule {
            needs: Word::Approve,
            signature: signature == "yes",
        }),
        None => Err(Fail::Invalid(format!("no approval kind {key}"))),
        Some(_) => Err(Fail::Store("an approval kind row is its signature".to_owned())),
    }
}

fn held<'a>(call: &'a Call) -> impl Iterator<Item = &'a Approval> {
    call.approvals.iter().chain([&JOB_ORDER])
}

fn split(body: &str) -> Result<(Vec<&str>, &str), Fail> {
    let (head, note) = body.split_once('\n').unwrap_or((body, ""));
    let note = note.trim();
    if note.contains(char::is_control) {
        return Err(Fail::Invalid("a note is one line".to_owned()));
    }
    Ok((head.split_whitespace().collect(), note))
}

const fn yes_no(yes: bool) -> &'static str {
    if yes { "yes" } else { "no" }
}

fn text(text: &str) -> Value {
    Value::Text(text.to_owned())
}

fn org(who: &Who) -> Value {
    Value::Text(who.org.0.clone())
}

pub(crate) fn mount() -> Mount {
    Mount::new("/approvals", "approvals")
        .operation(&SAVE_KIND, Reach::Org)
        .operation(&REQUEST, Reach::Org)
        .operation(&DECIDE, Reach::Org)
        .operation(&RESET, Reach::Org)
        .operation(&PENDING, Reach::Org)
        .operation(&RECENT, Reach::Org)
}
