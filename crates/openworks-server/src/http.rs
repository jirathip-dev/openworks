use std::io::{self, BufRead, BufReader, Read, Write};

const HEAD_LIMIT: u64 = 64 * 1024;
const BODY_LIMIT: usize = 32 * 1024 * 1024;

/// One HTTP request: its method, path without the query, headers and body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// One HTTP response: its status, content type and body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub kind: &'static str,
    pub body: Vec<u8>,
}

impl Asked {
    /// The value of a header, by its name in any case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(named, _)| named.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The bearer token the request carries, else an empty one.
    pub fn bearer(&self) -> &str {
        self.header("authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default()
            .trim()
    }
}

impl Answer {
    /// A plain-text answer.
    pub fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            kind: "text/plain; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }

    /// A JSON answer.
    pub fn json(status: u16, body: String) -> Self {
        Self {
            status,
            kind: "application/json",
            body: body.into_bytes(),
        }
    }
}

/// Reads one request; refuses it with the answer to send when it is not one this server reads.
pub fn read(stream: impl Read) -> Result<Asked, Answer> {
    let mut reader = BufReader::new(stream);
    let mut head = (&mut reader).take(HEAD_LIMIT);
    let mut line = String::new();
    head.read_line(&mut line).map_err(|_| refused(400))?;
    let mut words = line.split_whitespace();
    let (Some(method), Some(target)) = (words.next(), words.next()) else {
        return Err(refused(400));
    };
    let mut asked = Asked {
        method: method.to_owned(),
        path: target.split('?').next().unwrap_or_default().to_owned(),
        ..Asked::default()
    };
    loop {
        line.clear();
        if head.read_line(&mut line).map_err(|_| refused(400))? == 0 {
            return Err(refused(431));
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| refused(400))?;
        asked.headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    if asked.header("transfer-encoding").is_some() {
        return Err(refused(411));
    }
    let length = asked.header("content-length").map_or(Ok(0), str::parse::<usize>);
    let length = length.map_err(|_| refused(400))?;
    if length > BODY_LIMIT {
        return Err(refused(413));
    }
    asked.body = vec![0; length];
    reader.read_exact(&mut asked.body).map_err(|_| refused(400))?;
    Ok(asked)
}

/// Writes an answer and closes the exchange.
pub fn write(mut stream: impl Write, answer: &Answer) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        answer.status,
        reason(answer.status),
        answer.kind,
        answer.body.len()
    )?;
    stream.write_all(&answer.body)?;
    stream.flush()
}

fn refused(status: u16) -> Answer {
    Answer::text(status, reason(status))
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    }
}
