use std::iter::Peekable;
use std::str::Chars;

use crate::router::json::text;

#[derive(Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(members) => members.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }

    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn id(&self) -> Option<String> {
        match self {
            Self::Number(number) => Some(number.clone()),
            Self::Text(string) => Some(text(string)),
            _ => None,
        }
    }
}

type Input<'a> = Peekable<Chars<'a>>;

pub(crate) fn parse(line: &str) -> Option<Json> {
    let mut input = line.chars().peekable();
    let value = value(&mut input)?;
    blank(&mut input);
    input.next().is_none().then_some(value)
}

fn value(input: &mut Input) -> Option<Json> {
    blank(input);
    match input.next()? {
        '{' => object(input),
        '[' => list(input),
        '"' => string(input).map(Json::Text),
        't' => word(input, "rue", Json::Bool(true)),
        'f' => word(input, "alse", Json::Bool(false)),
        'n' => word(input, "ull", Json::Null),
        first @ ('-' | '0'..='9') => {
            let mut number = String::from(first);
            while let Some(digit) = input.next_if(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')) {
                number.push(digit);
            }
            Some(Json::Number(number))
        }
        _ => None,
    }
}

fn object(input: &mut Input) -> Option<Json> {
    let mut members = Vec::new();
    blank(input);
    if input.next_if_eq(&'}').is_some() {
        return Some(Json::Object(members));
    }
    loop {
        blank(input);
        input.next_if_eq(&'"')?;
        let name = string(input)?;
        blank(input);
        input.next_if_eq(&':')?;
        members.push((name, value(input)?));
        blank(input);
        match input.next()? {
            ',' => {}
            '}' => return Some(Json::Object(members)),
            _ => return None,
        }
    }
}

fn list(input: &mut Input) -> Option<Json> {
    let mut items = Vec::new();
    blank(input);
    if input.next_if_eq(&']').is_some() {
        return Some(Json::List(items));
    }
    loop {
        items.push(value(input)?);
        blank(input);
        match input.next()? {
            ',' => {}
            ']' => return Some(Json::List(items)),
            _ => return None,
        }
    }
}

fn string(input: &mut Input) -> Option<String> {
    let mut string = String::new();
    loop {
        match input.next()? {
            '"' => return Some(string),
            '\\' => string.push(match input.next()? {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                'b' => '\u{8}',
                'f' => '\u{c}',
                'u' => unicode(input)?,
                escaped @ ('"' | '\\' | '/') => escaped,
                _ => return None,
            }),
            c => string.push(c),
        }
    }
}

fn unicode(input: &mut Input) -> Option<char> {
    let first = unit(input)?;
    let mut units = vec![first];
    if (0xD800..0xDC00).contains(&first) {
        input.next_if_eq(&'\\')?;
        input.next_if_eq(&'u')?;
        units.push(unit(input)?);
    }
    char::decode_utf16(units).next()?.ok()
}

fn unit(input: &mut Input) -> Option<u16> {
    let hex: String = input.by_ref().take(4).collect();
    u16::from_str_radix(&hex, 16).ok()
}

fn word(input: &mut Input, rest: &str, value: Json) -> Option<Json> {
    rest.chars().all(|c| input.next() == Some(c)).then_some(value)
}

fn blank(input: &mut Input) {
    while input.next_if(char::is_ascii_whitespace).is_some() {}
}
