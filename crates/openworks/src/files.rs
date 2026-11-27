use openworks_core::{Call, Declaration, Fail, Tx, Value};
use openworks_db::attachments::{ADD, LIST as LIST_SQL};

/// Stores the body's file through the host's files under its tag; the body is the tag, the name, then the content, a line each.
pub const ATTACH: Declaration = openworks_core::create!("attach", needs: Write, queries: [ADD], attach);

/// Lists the files under the body's tag, one `id<TAB>name<TAB>size` line each; never their content.
pub const LIST: Declaration = openworks_core::read!("list", needs: Read, queries: [LIST_SQL], list);

fn attach(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let mut parts = call.body.splitn(3, '\n');
    let (Some(tag), Some(name), Some(content)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(Fail::Invalid("a file needs a tag, a name and its content".to_owned()));
    };
    let (tag, name) = (tag.trim(), name.trim());
    if tag.is_empty() || name.is_empty() {
        return Err(Fail::Invalid("a file needs a tag and a name".to_owned()));
    }
    let size = i64::try_from(content.len()).map_err(|_| Fail::Invalid("the file is too large".to_owned()))?;
    let org = &call.who.org.0;
    call.files.put(&format!("{org}/{}", call.id.0), content.as_bytes())?;
    tx.run(
        ADD,
        &[
            Value::Text(org.clone()),
            Value::Text(call.id.0.clone()),
            Value::Text(tag.to_owned()),
            Value::Text(name.to_owned()),
            Value::Int(size),
            Value::Int(call.at),
        ],
    )?;
    Ok(call.id.0.clone())
}

fn list(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let rows = tx.run(
        LIST_SQL,
        &[
            Value::Text(call.who.org.0.clone()),
            Value::Text(call.body.trim().to_owned()),
        ],
    )?;
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| match value {
                    Value::Null => String::new(),
                    Value::Int(n) => n.to_string(),
                    Value::Text(text) => text.clone(),
                })
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect();
    Ok(lines.join("\n"))
}
