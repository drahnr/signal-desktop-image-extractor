use chrono::{Date, DateTime, Datelike, LocalResult, TimeZone, Utc};
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

fn main() -> color_eyre::Result<()> {
    pretty_env_logger::init();
    let mut args = env::args();
    let _ = args.next();

    let base_dest = std::path::PathBuf::from(args.next().unwrap());
    let base = dirs::config_dir().unwrap().join("Signal");
    let config = base.join("config.json");
    let db = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("sql").join("db.sqlite"));
    let conn = Connection::open(db)?;

    let config = fs_err::File::open(config)?;
    let val: serde_json::Value = serde_json::from_reader(config)?;
    let val = val.as_object().unwrap();
    let val = val.get("key").unwrap().to_string();

    conn.pragma_update(None, "KEY", format!(r###"x'{}'"###, val.trim_matches('"')))?;

    let mut stmt = conn.prepare("SELECT rowid, id, json, sent_at, conversationId, received_at, hasAttachments, hasFileAttachments, hasVisualMediaAttachments, body, sourceUuid, serverGuid, expiresAt  FROM messages")?;
    let ts = |secs| DateTime::<Utc>::from_timestamp(secs, 0).unwrap();
    let uu = |s: String| Uuid::try_parse(&s).unwrap();
    let iter = stmt.query_map([], |row| {
        let json = row.get::<_, String>(2)?;
        let json: J = serde_json::from_str(&json).unwrap();
        let row = Message {
            rowid: row.get(0)?,
            id: uu(row.get(1)?),
            json: json.into(),
            sent_at: ts(row.get(3)?),
            conversation_id: uu(row.get(4)?),
            received_at: ts(row.get(5)?),
            has_attachments: row.get(6).unwrap_or_default(),
            has_file_attachments: row.get(7).unwrap_or_default(),
            has_visual_media_attachments: row.get(8).unwrap_or_default(),
            body: row.get(9).unwrap_or_default(),
        };
        Ok(row)
    })?;

    let inf = infer::Infer::new();
    for res in iter {
        match res {
            Err(e) => log::warn!("Failed to decode message: {e:?}"),
            Ok(m) => {
                for at in m.json.attachments.into_iter().flatten() {
                    let fname = format!(
                        "{}_{}",
                        m.sent_at.to_rfc3339(),
                        at.fileName.unwrap_or("unnamed".to_owned())
                    );
                    let src = base.join("attachments.noindex").join(&at.path);
                    log::info!("Copying from {} to {}", src.display(), base_dest.display());
                    let fallback = at.contentType.split_once('/').unwrap().1;

                    let ext = inf
                        .get_from_path(&src)?
                        .map(|x| x.extension())
                        .unwrap_or(&fallback);

                    let dst = base_dest.join(fname).with_extension(ext);
                    fs::copy(src, dst)?;
                }
            }
        }
    }

    Ok(())
}
