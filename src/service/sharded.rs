use crate::cloud::CloudOptions;
use crate::config::HazeConfig;
use crate::database::DatabaseFamily;
use crate::service::ServiceTrait;
use crate::Result;
use bollard::Docker;
use futures_util::future::try_join_all;
use maplit::hashmap;
use miette::Report;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::convert::identity;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Sharding;

const SHARDS: &[&str] = &["-1", "-2", "-3", "-4"];

#[async_trait::async_trait]
impl ServiceTrait for Sharding {
    fn name(&self) -> &str {
        "sharding"
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
        options: &CloudOptions,
    ) -> Result<Vec<String>> {
        if options.db.family() == DatabaseFamily::Sqlite {
            return Err(Report::msg("Sharding is not supported with sqlite"));
        }

        let containers = try_join_all(
            SHARDS
                .iter()
                .copied()
                .map(|postfix| options.db.spawn(docker, cloud_id, network, postfix)),
        )
        .await?;

        Ok(containers.into_iter().flatten().collect())
    }

    async fn is_healthy(
        &self,
        docker: &Docker,
        cloud_id: &str,
        options: &CloudOptions,
    ) -> Result<bool> {
        let running = try_join_all(
            SHARDS
                .iter()
                .copied()
                .map(|postfix| options.db.is_healthy(docker, cloud_id, postfix)),
        )
        .await?;
        Ok(running.iter().copied().all(identity))
    }

    fn config(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<HashMap<String, Value>> {
        let shard_config = json!({
            "filecache": {
              "table": "filecache",
              "primary_key": "fileid",
              "shard_key": "storage",
              "companion_tables": ["filecache_extended", "files_metadata"],
              "shards": [
                {
                  "dbname": "haze",
                  "host": "db-1",
                  "user": "haze",
                  "password": "haze",
                },
                {
                  "dbname": "haze",
                  "host": "db-2",
                  "user": "haze",
                  "password": "haze",
                },
                {
                  "dbname": "haze",
                  "host": "db-3",
                  "user": "haze",
                  "password": "haze",
                },
                {
                  "dbname": "haze",
                  "host": "db-4",
                  "user": "haze",
                  "password": "haze",
                }
              ],
            }
        });
        Ok(hashmap! {String::from("dbsharding") => shard_config})
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SingleShard;

#[async_trait::async_trait]
impl ServiceTrait for SingleShard {
    fn name(&self) -> &str {
        "single-shard"
    }

    async fn spawn(
        &self,
        docker: &Docker,
        cloud_id: &str,
        network: &str,
        _config: &HazeConfig,
        options: &CloudOptions,
    ) -> Result<Vec<String>> {
        if options.db.family() == DatabaseFamily::Sqlite {
            return Err(Report::msg("Sharding is not supported with sqlite"));
        }

        let container = options
            .db
            .spawn(docker, cloud_id, network, "-shard")
            .await?;

        Ok(container.into_iter().collect())
    }

    async fn is_healthy(
        &self,
        docker: &Docker,
        cloud_id: &str,
        options: &CloudOptions,
    ) -> Result<bool> {
        options.db.is_healthy(docker, cloud_id, "-shard").await
    }

    fn config(
        &self,
        _docker: &Docker,
        _cloud_id: &str,
        _config: &HazeConfig,
    ) -> Result<HashMap<String, Value>> {
        let shard_config = json!({
            "filecache": {
              "table": "filecache",
              "primary_key": "fileid",
              "shard_key": "storage",
              "companion_tables": ["filecache_extended", "files_metadata"],
              "shards": [
                {
                  "dbname": "haze",
                  "host": "db-shard",
                  "user": "haze",
                  "password": "haze",
                }
              ],
            }
        });
        Ok(hashmap! {String::from("dbsharding") => shard_config})
    }
}
