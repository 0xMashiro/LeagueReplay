use super::*;
use std::io::{BufRead, BufReader, Read};

pub const STREAM_LIMIT: u64 = 20 * 1024 * 1024 * 1024;
const ROW_LIMIT: u64 = 64 * 1024 * 1024;
const ROW_COUNT_LIMIT: u64 = 1_000_000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Header {
    format: String,
    version: u32,
    created_at: i64,
    tables: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "camelCase")]
enum Record {
    Row {
        table: String,
        values: BTreeMap<String, Value>,
    },
    End {
        rows: u64,
    },
}

/// Visits one row at a time. The table names come only from our schema allowlist.
pub(super) fn rows(
    db: &Connection,
    table: &str,
    mut visit: impl FnMut(Vec<Value>) -> Result<()>,
) -> Result<()> {
    let mut statement = db
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .map_err(sql)?;
    let columns = statement.column_count();
    let mut cursor = statement.query([]).map_err(sql)?;
    while let Some(row) = cursor.next().map_err(sql)? {
        let values = (0..columns)
            .map(|index| match row.get_ref(index).map_err(sql)? {
                rusqlite::types::ValueRef::Null => Ok(Value::Null),
                rusqlite::types::ValueRef::Integer(n) => Ok(n.into()),
                rusqlite::types::ValueRef::Text(text) => {
                    Ok(std::str::from_utf8(text).map_err(invalid)?.into())
                }
                _ => Err("backup.invalid".into()),
            })
            .collect::<Result<Vec<_>>>()?;
        visit(values)?;
    }
    Ok(())
}

pub(super) fn copy(source: &Connection, destination: &Connection) -> Result<()> {
    for table in TABLES.iter().rev() {
        destination
            .execute(&format!("DELETE FROM {table}"), [])
            .map_err(sql)?;
    }
    for table in TABLES {
        rows(source, table, |row| insert(destination, table, &row))?;
    }
    Ok(())
}

pub(super) fn insert(db: &Connection, table: &str, row: &[Value]) -> Result<()> {
    let values = row
        .iter()
        .map(|value| match value {
            Value::Null => Ok(rusqlite::types::Value::Null),
            Value::String(text) => Ok(rusqlite::types::Value::Text(text.clone())),
            Value::Number(n) => n
                .as_i64()
                .map(rusqlite::types::Value::Integer)
                .ok_or("backup.invalid"),
            _ => Err("backup.invalid"),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    db.prepare_cached(&format!(
        "INSERT INTO {table} VALUES({})",
        vec!["?"; values.len()].join(",")
    ))
    .map_err(invalid)?
    .execute(rusqlite::params_from_iter(values))
    .map_err(invalid)?;
    Ok(())
}

fn columns(db: &Connection, table: &str) -> Result<Vec<(String, String, bool)>> {
    db.prepare(&format!("PRAGMA table_info({table})"))
        .map_err(sql)?
        .query_map([], |row| {
            Ok((
                row.get(1)?,
                row.get(2)?,
                row.get::<_, bool>(3)? || row.get::<_, i64>(5)? != 0,
            ))
        })
        .map_err(sql)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(sql)
}

impl Store {
    pub fn save_backup(&self, path: &Path) -> Result<()> {
        let temporary = path.with_extension(format!("{}.tmp", id()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| "backup.writeFailed")?;
            self.write_backup(&mut file)?;
            file.sync_all().map_err(|_| "backup.writeFailed")?;
            drop(file);
            std::fs::rename(&temporary, path).map_err(|_| "backup.writeFailed".into())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    /// SQLite owns and removes this disk-backed temporary database on close.
    pub fn backup_snapshot(&self) -> Result<Store> {
        let mut target = Store::new(Connection::open("").map_err(sql)?)?;
        let tx = target.0.transaction().map_err(sql)?;
        copy(&self.0, &tx)?;
        tx.commit().map_err(sql)?;
        Ok(target)
    }

    pub fn write_backup(&self, writer: &mut impl Write) -> Result<()> {
        let mut bytes = 0u64;
        let mut write = |record: Vec<u8>| -> Result<()> {
            bytes += record.len() as u64 + 1;
            if bytes > STREAM_LIMIT || record.len() as u64 > ROW_LIMIT {
                return Err("backup.tooLarge".into());
            }
            writer
                .write_all(&record)
                .and_then(|_| writer.write_all(b"\n"))
                .map_err(|_| "backup.writeFailed".into())
        };
        write(
            serde_json::to_vec(&Header {
                format: "LeagueReplay archive".into(),
                version: 1,
                created_at: now(),
                tables: TABLES.iter().map(|s| (*s).into()).collect(),
            })
            .map_err(invalid)?,
        )?;
        let mut count = 0;
        for table in TABLES {
            let names = columns(&self.0, table)?;
            rows(&self.0, table, |row| {
                count += 1;
                if count > ROW_COUNT_LIMIT {
                    return Err("backup.tooLarge".into());
                }
                write(
                    serde_json::to_vec(&Record::Row {
                        table: (*table).into(),
                        values: names.iter().map(|c| c.0.clone()).zip(row).collect(),
                    })
                    .map_err(invalid)?,
                )
            })?;
        }
        write(serde_json::to_vec(&Record::End { rows: count }).map_err(invalid)?)
    }

    pub fn read_backup(reader: impl Read) -> Result<(Store, i64)> {
        let mut reader = BufReader::new(reader.take(STREAM_LIMIT + 1));
        let mut bytes = 0;
        let mut line = Vec::new();
        let mut next = || -> Result<Vec<u8>> {
            line.clear();
            (&mut reader)
                .take(ROW_LIMIT + 2)
                .read_until(b'\n', &mut line)
                .map_err(invalid)?;
            bytes += line.len() as u64;
            if bytes > STREAM_LIMIT
                || line.strip_suffix(b"\n").unwrap_or(&line).len() as u64 > ROW_LIMIT
            {
                return Err("backup.tooLarge".into());
            }
            Ok(std::mem::take(&mut line))
        };
        let header: Header = serde_json::from_slice(&next()?).map_err(invalid)?;
        if header.format != "LeagueReplay archive" || header.version != 1 {
            return Err("backup.version".into());
        }
        if header.tables != TABLES {
            return Err("backup.invalid".into());
        }
        let mut store = Store::new(Connection::open("").map_err(sql)?)?;
        let tx = store.0.transaction().map_err(sql)?;
        tx.execute("DELETE FROM user_state", []).map_err(sql)?;
        let schemas = TABLES
            .iter()
            .map(|t| columns(&tx, t))
            .collect::<Result<Vec<_>>>()?;
        let mut count = 0;
        let mut last_table = 0;
        loop {
            match serde_json::from_slice::<Record>(&next()?).map_err(invalid)? {
                Record::Row { table, values } => {
                    let index = TABLES
                        .iter()
                        .position(|t| *t == table)
                        .ok_or("backup.invalid")?;
                    if index < last_table || values.len() != schemas[index].len() {
                        return Err("backup.invalid".into());
                    }
                    last_table = index;
                    count += 1;
                    if count > ROW_COUNT_LIMIT {
                        return Err("backup.tooLarge".into());
                    }
                    let row = schemas[index]
                        .iter()
                        .map(|(name, kind, required)| {
                            let value = values.get(name).ok_or("backup.invalid")?;
                            match value {
                                Value::Null if !required => Ok(value.clone()),
                                Value::String(_) if kind == "TEXT" => Ok(value.clone()),
                                Value::Number(n) if kind == "INTEGER" && n.as_i64().is_some() => {
                                    Ok(value.clone())
                                }
                                _ => Err("backup.invalid"),
                            }
                        })
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    insert(&tx, &table, &row)?;
                }
                Record::End { rows } => {
                    if rows != count || !next()?.is_empty() {
                        return Err("backup.invalid".into());
                    }
                    break;
                }
            }
        }
        tx.commit().map_err(sql)?;
        validate_store(&mut store)?;
        Ok((store, header.created_at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_archive_exports_merges_and_leaves_a_recoverable_safety_copy() {
        let directory = std::env::temp_dir().join(format!("league-replay-stream-{}", id()));
        std::fs::create_dir(&directory).unwrap();
        let mut large = Store::new(Connection::open("").unwrap()).unwrap();
        let timeline =
            serde_json::json!({"frames": [], "padding": "x".repeat(1024 * 1024)}).to_string();
        let tx = large.0.transaction().unwrap();
        for n in 1..=65 {
            tx.execute(
                "INSERT INTO games(id,platform,game_number,timeline) VALUES (?1,'HN1',?2,?3)",
                params![format!("HN1_{n}"), n.to_string(), timeline],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let before = large.backup_fingerprint().unwrap();
        let path = directory.join("large.jsonl");
        large.save_backup(&path).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 64 * 1024 * 1024);
        let (imported, _) = Store::read_backup(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(imported.backup_fingerprint().unwrap(), before);
        let small = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let merged = small.merge_snapshot(&imported).unwrap();
        assert_eq!(
            merged
                .0
                .query_row("SELECT count(*) FROM games", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            65
        );
        let copy = large.restore_snapshot(&small, &before, &directory).unwrap();
        assert_eq!(
            large
                .0
                .query_row("SELECT count(*) FROM games", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
        let (recovery, _) = Store::read_backup(std::fs::File::open(&copy).unwrap()).unwrap();
        assert_eq!(recovery.backup_fingerprint().unwrap(), before);
        let fingerprint = large.backup_fingerprint().unwrap();
        let second_copy = large
            .restore_snapshot(&recovery, &fingerprint, &directory)
            .unwrap();
        assert_eq!(
            large
                .0
                .query_row("SELECT count(*) FROM games", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            65
        );
        for file in [path, copy.into(), second_copy.into()] {
            std::fs::remove_file(file).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn rejects_truncation_unknown_columns_wrong_types_and_trailing_content() {
        let store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let mut data = Vec::new();
        store.write_backup(&mut data).unwrap();
        let text = String::from_utf8(data).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(Store::read_backup(lines[..2].join("\n").as_bytes()).is_err());
        assert!(Store::read_backup(format!("{text}{{}}\n").as_bytes()).is_err());
        for (key, value) in [
            ("revision", Value::String("0".into())),
            ("unknown", Value::Null),
        ] {
            let mut row: Value = serde_json::from_str(lines[1]).unwrap();
            row["values"][key] = value;
            let bad = format!("{}\n{}\n{}\n", lines[0], row, lines[2]);
            assert!(Store::read_backup(bad.as_bytes()).is_err());
        }
        let mut header: Value = serde_json::from_str(lines[0]).unwrap();
        header["version"] = 99.into();
        assert!(
            matches!(Store::read_backup(format!("{header}\n").as_bytes()), Err(error) if error == "backup.version")
        );
    }
}
