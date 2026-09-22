use super::*;

pub(super) fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<ObservedMatch> {
    let parse = |index| -> rusqlite::Result<Option<Value>> {
        let raw: Option<String> = r.get(index)?;
        raw.map(|s| {
            serde_json::from_str(&s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .transpose()
    };
    Ok(ObservedMatch {
        id: r.get(0)?,
        platform: r.get(1)?,
        game_id: r.get(2)?,
        account_id: r.get(3)?,
        segment_id: r.get(4)?,
        first_seen: r.get::<_, i64>(5)? as f64,
        last_seen: r.get::<_, i64>(6)? as f64,
        champion_id: r.get(7)?,
        queue_name: r.get(8)?,
        state: r.get(9)?,
        game: parse(10)?
            .map(|detail| crate::matches::project(&detail))
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        data_error: r.get(11)?,
        observation_id: r.get(12)?,
        automatic: r.get(13)?,
        manual: r.get(14)?,
    })
}
