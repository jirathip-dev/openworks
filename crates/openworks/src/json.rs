use std::collections::{BTreeMap, BTreeSet, HashMap};

use openworks_core::{Extension, Handler, Id, Record, Type, Value};

use super::Route;

pub(crate) type Extended = (&'static Extension, Vec<(Id, Vec<Value>)>);

pub(crate) fn page(records: &[Record], extended: &[Extended]) -> String {
    let found: Vec<HashMap<&Id, &Vec<Value>>> = extended
        .iter()
        .map(|(_, rows)| rows.iter().map(|(id, values)| (id, values)).collect())
        .collect();
    let items: Vec<String> = records
        .iter()
        .map(|record| {
            let ext = extended.iter().zip(&found).map(|((extension, _), found)| {
                let value = found.get(&record.id).map_or_else(
                    || "null".to_owned(),
                    |values| {
                        object(
                            extension
                                .fields
                                .iter()
                                .map(|(name, _)| *name)
                                .zip(values.iter().map(value)),
                        )
                    },
                );
                (extension.key, value)
            });
            let fields = [("id", text(&record.id.0))]
                .into_iter()
                .chain(record.fields.iter().map(|(name, field)| (*name, value(field))))
                .chain([("ext", object(ext))]);
            object(fields)
        })
        .collect();
    format!("[{}]", items.join(","))
}

pub(crate) fn openapi(routes: &HashMap<String, Route>, extensions: &[&'static Extension]) -> String {
    let mut records = BTreeSet::new();
    let paths: BTreeMap<&str, String> = routes
        .iter()
        .map(|(path, route)| {
            let needs = text(route.declaration.needs().0);
            let ok = match route.declaration.handler() {
                Handler::List(record, _) => {
                    records.insert(record);
                    format!(
                        r#"{{"description":"ok","content":{{"application/json":{{"schema":{{"type":"array","items":{{"$ref":{}}}}}}}}}}}"#,
                        text(&format!("#/components/schemas/{record}"))
                    )
                }
                Handler::Store(_) | Handler::Closed(_) => r#"{"description":"ok"}"#.to_owned(),
            };
            (
                path.as_str(),
                format!(r#"{{"post":{{"x-needs":{needs},"responses":{{"200":{ok}}}}}}}"#),
            )
        })
        .collect();
    let schemas = records.into_iter().map(|record| {
        let ext = extensions
            .iter()
            .filter(|extension| extension.record == record)
            .map(|extension| {
                let fields = extension.fields.iter().map(|(name, kind)| (*name, typed(*kind)));
                let schema = format!(r#"{{"type":["object","null"],"properties":{}}}"#, object(fields));
                (extension.key, schema)
            });
        let properties = object(
            [
                ("id", typed(Type::Text)),
                ("ext", format!(r#"{{"type":"object","properties":{}}}"#, object(ext))),
            ]
            .into_iter(),
        );
        (record, format!(r#"{{"type":"object","properties":{properties}}}"#))
    });
    format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"openworks","version":{}}},"paths":{},"components":{{"schemas":{}}}}}"#,
        text(env!("CARGO_PKG_VERSION")),
        object(paths.into_iter()),
        object(schemas)
    )
}

fn typed(kind: Type) -> String {
    match kind {
        Type::Int => r#"{"type":"integer"}"#,
        Type::Text => r#"{"type":"string"}"#,
    }
    .to_owned()
}

fn object<'a>(members: impl Iterator<Item = (&'a str, String)>) -> String {
    let members: Vec<String> = members.map(|(name, json)| format!("{}:{json}", text(name))).collect();
    format!("{{{}}}", members.join(","))
}

fn value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Int(int) => int.to_string(),
        Value::Text(string) => text(string),
    }
}

fn text(string: &str) -> String {
    let mut quoted = String::with_capacity(string.len() + 2);
    quoted.push('"');
    for c in string.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            c if c.is_control() => {
                let code = u32::from(c);
                quoted.push_str("\\u00");
                quoted.extend([code >> 4, code & 0xf].map(|digit| char::from_digit(digit, 16).unwrap_or('0')));
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}
