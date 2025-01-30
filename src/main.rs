use chrono::{Date, DateTime, Datelike, LocalResult, TimeZone, Timelike, Utc};
use clap::Parser;
use color_eyre::eyre::OptionExt;
use fs_err as fs;
use serde_json::Value;
use std::collections::HashMap;
use std::env::{self, home_dir};
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use uuid::{Timestamp, Uuid};

use rusqlite::{Connection, Map, Result, Row};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Attachment {
    content: Option<String>,
    caption: Option<String>,
    contentType: String,
    flags: Option<u64>,
    fileName: Option<String>,
    size: u64,
    path: std::path::PathBuf,
    height: Option<u64>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct J {
    attachments: Option<Vec<Attachment>>,
    body: Option<String>,
    // bodyRanges: Option<Vec<String>>,
    // contact: Vec<String>,
    conversationId: Uuid,
    // errors: Option<Vec<String>>,
    flags: Option<u64>,
    id: Uuid,
}

#[derive(Debug)]
struct Message {
    rowid: i64,
    id: Uuid,
    json: J,
    sent_at: DateTime<Utc>,
    conversation_id: Uuid,
    received_at: DateTime<Utc>,
    has_attachments: i32,
    has_file_attachments: i32,
    has_visual_media_attachments: i32,
    body: String,
}

#[derive(clap::Parser, Clone, Debug)]
struct Args {
    #[command(flatten)]
    verbose: clap_verbosity::Verbosity,
    #[arg(long, default_value = None)]
    base: Option<std::path::PathBuf>,
    #[arg(long)]
    dest: std::path::PathBuf,
}

fn default_base() -> std::path::PathBuf {
    dirs::config_dir().unwrap().join("Signal")
}

fn main() -> color_eyre::Result<()> {
    let Args {
        verbose,
        base,
        dest,
    } = Args::parse();
    pretty_env_logger::formatted_timed_builder()
        .filter_level(verbose.log_level_filter())
        .init();

    let base_dest = dest;
    let base = base.unwrap_or(default_base());
    let config = base.join("config.json");
    let db = base.join("sql").join("db.sqlite");

    let conn = Connection::open(db)?;

    let config = fs_err::File::open(config)?;
    let val: serde_json::Value = serde_json::from_reader(config)?;
    let val = val
        .as_object()
        .ok_or_eyre("Failed to parse config.json file")?;
    let val = val
        .get("key")
        .ok_or_eyre(r##"Missing "key" key in config.json"##)?
        .to_string();

    conn.pragma_update(None, "KEY", format!(r###"x'{}'"###, val.trim_matches('"')))?;

    let mut stmt = conn.prepare("SELECT rowid, id, json, sent_at, conversationId, received_at, hasAttachments, hasFileAttachments, hasVisualMediaAttachments, body, sourceUuid, serverGuid, expiresAt  FROM messages")?;
    let ts = |secs| {
        DateTime::<Utc>::from_timestamp(secs, 0)
            .ok_or_eyre("Couldn't convert unix timestamp to date time type")
    };
    let uu = |s: String| Uuid::try_parse(&s);
    let iter = stmt.query([])?;
    let mapper = |row: &rusqlite::Row| -> color_eyre::Result<Message> {
        let json = row.get::<_, String>(2)?;
        let json: J = serde_json::from_str(&json)?;
        let row = Message {
            rowid: row.get(0)?,
            id: uu(row.get(1)?)?,
            json: json.into(),
            sent_at: ts(row.get(3)?)?,
            conversation_id: uu(row.get(4)?)?,
            received_at: ts(row.get(5)?)?,
            has_attachments: row.get(6).unwrap_or_default(),
            has_file_attachments: row.get(7).unwrap_or_default(),
            has_visual_media_attachments: row.get(8).unwrap_or_default(),
            body: row.get(9).unwrap_or_default(),
        };
        color_eyre::Result::Ok(row)
    };

    let iter = iter.and_then(mapper);
    let inf = infer::Infer::new();
    for res in iter {
        match res {
            Err(e) => log::warn!("Failed to decode message: {e:?}"),
            Ok(m) => {
                for at in m.json.attachments.into_iter().flatten() {
                    let fname = format!(
                        "signal_{}__{}",
                        m.sent_at.format("20%y-%m-%dT%H:%M:%S"),
                        at.fileName.unwrap_or("unnamed".to_owned())
                    );
                    let src = base.join("attachments.noindex").join(&at.path);
                    let fallback = at
                        .contentType
                        .split_once('/')
                        .ok_or_eyre(
                            "JSON contained content type always contains exactly one slash",
                        )?
                        .1;

                    let ext = inf
                        .get_from_path(&src)?
                        .map(|x| x.extension())
                        .unwrap_or(&fallback);

                    let dst = dbg!(&base_dest).join(dbg!(fname)).with_extension(ext);
                    log::info!("Copying from {} to {}", src.display(), dst.display());

                    fs::copy(src, dst)?;
                }
            }
        }
    }

    Ok(())
}
