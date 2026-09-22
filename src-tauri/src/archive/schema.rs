use super::*;

const VERSION: i32 = 2;
const APPLICATION_ID: i32 = 0x4c52504c; // LRPL

pub(super) fn open(connection: &mut Connection) -> Result<()> {
    let version: i32 = connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(sql)?;
    let application: i32 = connection
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(sql)?;
    let fresh = version == 0 && application == 0;
    if fresh {
        let populated: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%')",
                [],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if populated {
            return Err("archive.unsupportedSchema".into());
        }
    } else if version != VERSION || application != APPLICATION_ID {
        return Err("archive.unsupportedSchema".into());
    } else {
        let initialized: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM user_state WHERE id=1) AND EXISTS(SELECT 1 FROM workspace_clock WHERE id=1)",
            [], |r| r.get(0),
        ).map_err(|_| "archive.invalidSchema")?;
        if !initialized {
            return Err("archive.invalidSchema".into());
        }
    }
    // Reject unsupported files before changing any persistent database settings.
    connection
        .busy_timeout(std::time::Duration::from_secs(3))
        .map_err(sql)?;
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(sql)?;
    if fresh {
        create(connection, include_str!("schema.sql"))?;
    }
    Ok(())
}

fn create(connection: &mut Connection, ddl: &str) -> Result<()> {
    let tx = connection.transaction().map_err(sql)?;
    tx.execute_batch(ddl).map_err(sql)?;
    sync::install(&tx)?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)
        .map_err(sql)?;
    tx.pragma_update(None, "user_version", VERSION)
        .map_err(sql)?;
    tx.commit().map_err(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_schema_is_versioned_and_reopening_does_not_recreate_deleted_data() {
        let mut db = Connection::open_in_memory().unwrap();
        open(&mut db).unwrap();
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                .unwrap(),
            VERSION
        );
        assert_eq!(
            db.pragma_query_value(None, "application_id", |r| r.get::<_, i32>(0))
                .unwrap(),
            APPLICATION_ID
        );
        db.execute("UPDATE user_state SET revision=7 WHERE id=1", [])
            .unwrap();
        open(&mut db).unwrap();
        assert_eq!(
            db.query_row("SELECT revision FROM user_state", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            7
        );
        // Opening an existing version must not silently manufacture missing records.
        db.execute("DELETE FROM user_state", []).unwrap();
        assert_eq!(open(&mut db).unwrap_err(), "archive.invalidSchema");
        assert_eq!(
            db.query_row("SELECT count(*) FROM user_state", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn unsupported_versions_and_unversioned_nonempty_files_are_untouched() {
        for (application, version) in [
            (APPLICATION_ID, 1),
            (APPLICATION_ID, VERSION + 1),
            (42, VERSION),
            (0, 0),
        ] {
            let mut db = Connection::open_in_memory().unwrap();
            db.execute_batch(
                "CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES ('keep');",
            )
            .unwrap();
            db.pragma_update(None, "application_id", application)
                .unwrap();
            db.pragma_update(None, "user_version", version).unwrap();
            assert_eq!(open(&mut db).unwrap_err(), "archive.unsupportedSchema");
            assert_eq!(
                db.query_row("SELECT value FROM sentinel", [], |r| r.get::<_, String>(0))
                    .unwrap(),
                "keep"
            );
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                    .unwrap(),
                version
            );
            assert_eq!(
                db.pragma_query_value(None, "application_id", |r| r.get::<_, i32>(0))
                    .unwrap(),
                application
            );
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='table'",
                    [],
                    |r| r.get::<_, u32>(0)
                )
                .unwrap(),
                1
            );
        }
    }

    #[test]
    fn failed_initialization_rolls_back_schema_and_version() {
        let mut db = Connection::open_in_memory().unwrap();
        let broken = format!("{}\nINVALID STATEMENT;", include_str!("schema.sql"));
        assert!(create(&mut db, &broken).is_err());
        assert_eq!(
            db.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                .unwrap(),
            0
        );
        open(&mut db).unwrap();
    }
}
