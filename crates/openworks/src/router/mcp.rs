use std::io::{self, BufRead, Lines, Write};

use openworks_core::{Offer, Via};

use crate::router::json::text;
use crate::{Host, Openworks, Request, Tool};

use parse::Json;

mod parse;

const PROTOCOL: &str = "2025-06-18";

type Reply = Result<String, (i64, String)>;

struct Session<'a, H, R, W> {
    openworks: &'a Openworks<H>,
    token: &'a str,
    lines: Lines<R>,
    out: W,
    confirms: bool,
    asked: u64,
}

impl<H: Host> Openworks<H> {
    /// Serves the tools over MCP, a JSON-RPC message a line, as the person the token signs in; a write runs once they confirm it.
    pub fn serve_mcp(&self, token: &str, input: impl BufRead, output: impl Write) -> io::Result<()> {
        let mut session = Session {
            openworks: self,
            token,
            lines: input.lines(),
            out: output,
            confirms: false,
            asked: 0,
        };
        while let Some(line) = session.lines.next().transpose()? {
            session.message(&line)?;
        }
        Ok(())
    }
}

impl<H: Host, R: BufRead, W: Write> Session<'_, H, R, W> {
    fn message(&mut self, line: &str) -> io::Result<()> {
        if line.trim().is_empty() {
            return Ok(());
        }
        let Some(message) = parse::parse(line) else {
            return self.send(&failed("null", -32700, "not JSON"));
        };
        let (Some(id), Some(method)) = (
            message.get("id").and_then(Json::id),
            message.get("method").and_then(Json::text),
        ) else {
            return Ok(());
        };
        let params = message.get("params");
        let reply = match method {
            "initialize" => {
                self.confirms = params
                    .and_then(|params| params.get("capabilities"))
                    .and_then(|capabilities| capabilities.get("elicitation"))
                    .is_some();
                Ok(initialized())
            }
            "ping" => Ok("{}".to_owned()),
            "tools/list" => Ok(self.listed()),
            "tools/call" => self.called(params)?,
            _ => Err((-32601, format!("no method {method}"))),
        };
        self.send(&match reply {
            Ok(result) => format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result}}}"#),
            Err((code, why)) => failed(&id, code, &why),
        })
    }

    fn listed(&self) -> String {
        let tools: Vec<String> = self.openworks.tools().iter().map(described).collect();
        format!(r#"{{"tools":[{}]}}"#, tools.join(","))
    }

    fn called(&mut self, params: Option<&Json>) -> io::Result<Reply> {
        let name = params.and_then(|params| params.get("name")).and_then(Json::text);
        let Some(tool) = self
            .openworks
            .tools()
            .into_iter()
            .find(|tool| Some(tool.name.as_str()) == name)
        else {
            return Ok(Err((-32602, format!("no tool {}", name.unwrap_or_default()))));
        };
        let body = params
            .and_then(|params| params.get("arguments"))
            .and_then(|arguments| arguments.get("body"))
            .and_then(Json::text)
            .unwrap_or_default();
        let via = match tool.offer {
            Offer::Proposal if !self.confirms => {
                return Ok(Ok(answered(
                    true,
                    "this client cannot ask the person to confirm: nothing was written",
                )));
            }
            Offer::Proposal if !self.confirmed(&tool, body)? => {
                return Ok(Ok(answered(true, "the person declined: nothing was written")));
            }
            Offer::Proposal => Via::Person,
            Offer::Read | Offer::ByHand => Via::Assistant,
        };
        let request = Request {
            path: tool.path,
            credential: self.token.to_owned(),
            idempotency_key: (via == Via::Person).then(|| self.openworks.host.id().0),
            body: body.to_owned(),
        };
        let response = self.openworks.respond(&request, Some(via));
        Ok(Ok(answered(response.status != 200, &response.body)))
    }

    fn confirmed(&mut self, tool: &Tool, body: &str) -> io::Result<bool> {
        self.asked += 1;
        let id = text(&format!("confirm-{}", self.asked));
        let ask = text(&format!("Confirm {} ({}) with:\n{body}", tool.path, tool.needs.name()));
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"elicitation/create","params":{{"message":{ask},"requestedSchema":{{"type":"object","properties":{{}}}}}}}}"#
        ))?;
        while let Some(line) = self.lines.next().transpose()? {
            match parse::parse(&line) {
                Some(answer) if answer.get("id").and_then(Json::id).as_ref() == Some(&id) => {
                    let action = answer.get("result").and_then(|result| result.get("action"));
                    return Ok(action.and_then(Json::text) == Some("accept"));
                }
                _ => self.message(&line)?,
            }
        }
        Ok(false)
    }

    fn send(&mut self, line: &str) -> io::Result<()> {
        writeln!(self.out, "{line}")?;
        self.out.flush()
    }
}

fn initialized() -> String {
    format!(
        r#"{{"protocolVersion":{},"capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"openworks","version":{}}}}}"#,
        text(PROTOCOL),
        text(env!("CARGO_PKG_VERSION"))
    )
}

fn described(tool: &Tool) -> String {
    let (what, read_only) = match tool.offer {
        Offer::Read => ("Reads", true),
        Offer::Proposal | Offer::ByHand => ("Proposes, for the person to confirm,", false),
    };
    let description = format!("{what} {}; needs {}", tool.path, tool.needs.name());
    format!(
        r#"{{"name":{},"description":{},"inputSchema":{{"type":"object","properties":{{"body":{{"type":"string"}}}}}},"annotations":{{"readOnlyHint":{read_only}}}}}"#,
        text(&tool.name),
        text(&description)
    )
}

fn answered(error: bool, body: &str) -> String {
    format!(
        r#"{{"content":[{{"type":"text","text":{}}}],"isError":{error}}}"#,
        text(body)
    )
}

fn failed(id: &str, code: i64, why: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":{code},"message":{}}}}}"#,
        text(why)
    )
}
