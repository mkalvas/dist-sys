use std::io::{StdinLock, StdoutLock, Write};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    src: String,
    dest: String,
    body: Body,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Body {
    msg_id: Option<i32>,
    in_reply_to: Option<i32>,
    #[serde(flatten)]
    payload: Payload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Payload {
    Init {
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk,
    Echo {
        echo: String,
    },
    EchoOk {
        echo: String,
    },
    Generate,
    GenerateOk {
        id: String,
    },
    Topology {
        node_ids: Vec<String>,
    },
    TopologyOk,
    Broadcast {
        message: i32,
    },
    BroadcastOk,
    Read,
    ReadOk {
        messages: Vec<i32>,
    },
}

pub struct Node<'a> {
    messages: Vec<i32>,
    next_msg_id: i32,
    node_id: Option<String>,
    node_ids: Option<Vec<String>>,
    stdout: StdoutLock<'a>,
}

impl Node<'_> {
    pub fn new(stdout: StdoutLock) -> Node {
        Node {
            messages: vec![],
            next_msg_id: 0,
            node_id: None,
            node_ids: None,
            stdout,
        }
    }

    pub fn run(&mut self, stdin: StdinLock) -> Result<()> {
        let strin = Deserializer::from_reader(stdin).into_iter::<Message>();
        for msg in strin {
            let msg = msg.context("STDIN could not be deserialized")?;
            self.handle(msg).context("handler failed")?;
        }
        Ok(())
    }

    pub fn handle(&mut self, msg: Message) -> Result<()> {
        match msg.body.payload.clone() {
            Payload::Echo { echo } => self.reply(msg, Payload::EchoOk { echo }),
            Payload::Init { node_id, node_ids } => {
                self.init(node_id, node_ids).context("failed to init")?;
                self.reply(msg, Payload::InitOk)
            }
            Payload::Generate { .. } => self.reply(
                msg,
                Payload::GenerateOk {
                    id: self.generate_id(),
                },
            ),
            Payload::Topology { .. } => self.reply(msg, Payload::Topology { node_ids: vec![] }),
            Payload::Broadcast { .. } => self.reply(msg, Payload::BroadcastOk),
            Payload::Read { .. } => self.reply(
                msg,
                Payload::ReadOk {
                    messages: self.messages.clone(),
                },
            ),
            _ => Ok(()), // ignore "oks" from other nodes
        }
    }

    pub fn init(&mut self, node_id: String, node_ids: Vec<String>) -> Result<()> {
        self.node_id = Some(node_id);
        self.node_ids = Some(node_ids);
        Ok(())
    }

    pub fn generate_id(&self) -> String {
        let n = self.node_id.as_ref().expect("generating id before init");
        format!("{}-{}", n, self.next_msg_id)
    }

    fn reply(&mut self, msg: Message, payload: Payload) -> Result<()> {
        let reply = Message {
            src: msg.dest,
            dest: msg.src,
            body: Body {
                msg_id: Some(self.next_msg_id),
                in_reply_to: msg.body.msg_id,
                payload,
            },
        };

        serde_json::to_writer(&mut self.stdout, &reply).context("serialize reply")?;
        self.stdout
            .write_all(b"\n")
            .context("add trailing newline to replies")?;

        self.next_msg_id += 1;
        Ok(())
    }
}
