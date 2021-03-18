use crate::config::HazeConfig;
use camino::Utf8Path;
use color_eyre::Result;
use tokio::fs::{create_dir_all, write};

#[derive(Debug)]
pub struct Mapping<'a> {
    source_type: MappingSourceType,
    pub source: &'a Utf8Path,
    target: &'a Utf8Path,
    mapping_type: MappingType,
    read_only: bool,
    map: bool,
}

impl<'a> Mapping<'a> {
    pub fn new<Source, Target>(
        source_type: MappingSourceType,
        source: Source,
        target: Target,
    ) -> Self
    where
        Target: Into<&'a Utf8Path>,
        Source: Into<&'a Utf8Path>,
    {
        Mapping {
            source_type,
            source: source.into(),
            target: target.into(),
            mapping_type: MappingType::Folder,
            read_only: false,
            map: true,
        }
    }

    pub fn read_only(self) -> Self {
        Self {
            read_only: true,
            ..self
        }
    }

    pub fn dont_map(self) -> Self {
        Self { map: false, ..self }
    }

    pub fn file(self) -> Self {
        Self {
            mapping_type: MappingType::File,
            ..self
        }
    }

    pub async fn create(&self, id: &str, config: &HazeConfig) -> Result<()> {
        let source = match self.source_type {
            MappingSourceType::WorkDir => config.work_dir.join(id).join(self.source),
            MappingSourceType::GlobalWorkDir => config.work_dir.join(self.source),
            MappingSourceType::Sources => return Ok(()),
        };
        match self.mapping_type {
            MappingType::Folder => create_dir_all(source).await?,
            MappingType::File => write(source, "").await?,
        }

        Ok(())
    }

    pub fn get_volume_arg(&self, id: &str, config: &HazeConfig) -> Option<String> {
        if !self.map {
            return None;
        }
        let source = match self.source_type {
            MappingSourceType::WorkDir => config.work_dir.join(id).join(self.source),
            MappingSourceType::GlobalWorkDir => config.work_dir.join(self.source),
            MappingSourceType::Sources => config.sources_root.join(self.source),
        };
        Some(if self.read_only {
            format!("{}:{}:ro", source, self.target)
        } else {
            format!("{}:{}", source, self.target)
        })
    }
}

pub fn default_mappings() -> Vec<Mapping<'static>> {
    use MappingSourceType::*;

    vec![
        Mapping::new(Sources, "", "/var/www/html"),
        Mapping::new(WorkDir, "data", "/var/www/html/data"),
        Mapping::new(WorkDir, "config", "/var/www/html/config"),
        Mapping::new(WorkDir, "data-autotest", "/var/www/html/data-autotest"),
        Mapping::new(WorkDir, "skeleton", "/var/www/html/core/skeleton"),
        Mapping::new(
            Sources,
            "skeleton/welcome.txt",
            "/var/www/html/core/skeleton/welcome.txt",
        )
        .file()
        .read_only(),
        Mapping::new(
            WorkDir,
            "integration/vendor",
            "/var/www/html/build/integration/vendor",
        ),
        Mapping::new(
            WorkDir,
            "integration/work",
            "/var/www/html/build/integration/work",
        ),
        Mapping::new(
            WorkDir,
            "integration/output",
            "/var/www/html/build/integration/output",
        ),
        Mapping::new(
            WorkDir,
            "integration/composer.lock",
            "/var/www/html/build/integration/composer.lock",
        )
        .file(),
        Mapping::new(GlobalWorkDir, "composer/cache", "/var/www/.composer/cache"),
        Mapping::new(
            GlobalWorkDir,
            "phpunit-cache",
            "/var/www/html/tests/.phpunit.result.cache",
        )
        .file(),
        Mapping::new(WorkDir, "config/CAN_INSTALL", "")
            .file()
            .dont_map(),
        Mapping::new(Sources, ".htaccess", "/var/www/html/.htaccess")
            .file()
            .read_only(),
    ]
}

#[derive(Debug, Copy, Clone)]
pub enum MappingSourceType {
    Sources,
    WorkDir,
    GlobalWorkDir,
}

#[derive(Debug, Copy, Clone)]
pub enum MappingType {
    Folder,
    File,
}
