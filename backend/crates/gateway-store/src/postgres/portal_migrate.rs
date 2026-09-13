//! Portal 扩展迁移：独立表 `portal_schema_migrations`，不占用官方 sqlx 版本号。

use sha2::{Digest as _, Sha256};
use sqlx::{PgPool, Postgres, Transaction};

use crate::{StoreBackend, StoreError, StoreResult, postgres_unavailable};

const PORTAL_MIGRATIONS: &[(&str, i64, &str)] = &[(
    "0001_portal",
    1,
    include_str!("../../migrations/portal/0001_portal.sql"),
)];

/// 官方迁移完成后再应用 Portal 扩展；用咨询锁串行化启动。
pub async fn apply_portal_migrations(pool: &PgPool) -> StoreResult<()> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| postgres_unavailable("begin portal migration lock"))?;
    sqlx::query("select pg_advisory_xact_lock(87204601)")
        .execute(&mut *tx)
        .await
        .map_err(|_| postgres_unavailable("lock portal migrations"))?;
    sqlx::query(
        "create table if not exists portal_schema_migrations (
            version bigint primary key,
            description text not null,
            checksum text not null,
            applied_at timestamptz not null
        )",
    )
    .execute(&mut *tx)
    .await
    .map_err(|_| postgres_unavailable("create portal_schema_migrations"))?;
    for (description, version, sql) in PORTAL_MIGRATIONS {
        apply_one(&mut tx, *version, description, sql).await?;
    }
    tx.commit()
        .await
        .map_err(|_| postgres_unavailable("commit portal migrations"))?;
    Ok(())
}

async fn apply_one(
    tx: &mut Transaction<'_, Postgres>,
    version: i64,
    description: &str,
    sql: &str,
) -> StoreResult<()> {
    let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
    let existing = sqlx::query_as::<_, (String,)>(
        "select checksum from portal_schema_migrations where version = $1",
    )
    .bind(version)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| postgres_unavailable("load portal migration checksum"))?;
    if let Some((stored,)) = existing {
        if stored != checksum {
            return Err(StoreError::Unavailable {
                backend: StoreBackend::PostgreSql,
                message: format!("portal migration {description} checksum mismatch"),
            });
        }
        return Ok(());
    }
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(&mut **tx)
        .await
        .map_err(|error| StoreError::Unavailable {
            backend: StoreBackend::PostgreSql,
            message: format!("apply portal migration {description}: {error}"),
        })?;
    sqlx::query(
        "insert into portal_schema_migrations (version, description, checksum, applied_at)
         values ($1, $2, $3, now())",
    )
    .bind(version)
    .bind(description)
    .bind(checksum)
    .execute(&mut **tx)
    .await
    .map_err(|_| postgres_unavailable("record portal migration"))?;
    Ok(())
}
