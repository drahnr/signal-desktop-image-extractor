use chrono::{DateTime, Utc};
use clap::Parser;
use color_eyre::eyre::OptionExt;
use fs_err as fs;
use uuid::Uuid;

use rusqlite::Connection;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attachment {
    content: Option<String>,
    caption: Option<String>,
    content_type: String,
    flags: Option<u64>,
    file_name: Option<String>,
    size: u64,
    path: std::path::PathBuf,
    height: Option<u64>,
}

/// JSON child entry of a message in the messages table
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Json {
    attachments: Option<Vec<Attachment>>,
    body: Option<String>,
    // bodyRanges: Option<Vec<String>>,
    // contact: Vec<String>,
    conversation_id: Uuid,
    // errors: Option<Vec<String>>,
    flags: Option<u64>,
    id: Uuid,
}

/// A subset of a message in the messages table
#[derive(Debug)]
#[allow(dead_code)]
struct Message {
    rowid: i64,
    id: Uuid,
    json: Json,
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

fn main() -> color_eyre::Result<()> {
    let Args {
        verbose,
        base,
        dest: dest_dir,
    } = Args::parse();
    pretty_env_logger::formatted_timed_builder()
        .filter_level(verbose.log_level_filter())
        .init();

    fs::create_dir_all(&dest_dir)?;

    let base = base.unwrap_or(    dirs::config_dir().ok_or_eyre("Your system does not provide a XDG config dir, set XDG_CONFIG_HOME to provide in this case")?.join("Signal")
);
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
        let json: Json = serde_json::from_str(&json)?;
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
                        at.file_name.unwrap_or("unnamed".to_owned())
                    );
                    let src = base.join("attachments.noindex").join(&at.path);
                    let fallback = at
                        .content_type
                        .split_once('/')
                        .ok_or_eyre(
                            "JSON contained content type always contains exactly one slash",
                        )?
                        .1;

                    let ext = inf
                        .get_from_path(&src)?
                        .map(|x| x.extension())
                        .unwrap_or(&fallback);

                    let dst = dest_dir.join(fname).with_extension(ext);
                    log::info!("Copying from {} to {}", src.display(), dst.display());

                    fs::copy(src, dst)?;
                }
            }
        }
    }

    Ok(())
}
