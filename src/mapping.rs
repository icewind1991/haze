use crate::config::{HazeConfig, HazeVolumeConfig};
use camino::Utf8Path;
use miette::{IntoDiagnostic, Result};
use tokio::fs::{create_dir_all, write};

#[derive(Debug)]
pub struct Mapping<'a> {
    source_type: MappingSourceType,
    pub source: &'a Utf8Path,
    target: &'a Utf8Path,
    mapping_type: MappingType,
    read_only: bool,
    map: bool,
    create: bool,
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
            create: true,
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

    pub fn dont_create(self) -> Self {
        Self {
            create: false,
            ..self
        }
    }

    pub fn file(self) -> Self {
        Self {
            mapping_type: MappingType::File,
            ..self
        }
    }

    pub async fn create(&self, id: &str, config: &HazeConfig) -> Result<()> {
        if !self.create {
            return Ok(());
        }
        let source = match self.source_type {
            MappingSourceType::WorkDir => config.work_dir.join(id).join(self.source),
            MappingSourceType::GlobalWorkDir => config.work_dir.join(self.source),
            MappingSourceType::Sources => return Ok(()),
            MappingSourceType::Absolute => self.source.into(),
        };
        match self.mapping_type {
            MappingType::Folder => create_dir_all(source).await.into_diagnostic()?,
            MappingType::File => write(source, "").await.into_diagnostic()?,
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
            MappingSourceType::Absolute => self.source.into(),
        };
        Some(if self.read_only {
            format!("{}:{}:ro", source, self.target)
        } else {
            format!("{}:{}", source, self.target)
        })
    }
}

pub fn default_mappings<'a>() -> impl IntoIterator<Item = Mapping<'a>> {
    use MappingSourceType::*;

    let mappings = [
        Mapping::new(Sources, "", "/var/www/html"),
        Mapping::new(WorkDir, "data", "/var/www/html/data"),
        Mapping::new(WorkDir, "config", "/var/www/html/config"),
        Mapping::new(WorkDir, "data-autotest", "/var/www/html/data-autotest"),
        Mapping::new(WorkDir, "skeleton", "/var/www/html/core/skeleton"),
        Mapping::new(
            Sources,
            "core/skeleton/welcome.txt",
            "/var/www/html/core/skeleton/welcome.txt",
        )
        .file()
        .read_only()
        .dont_create(),
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
        Mapping::new(
            GlobalWorkDir,
            "composer/cache",
            "/home/haze/.composer/cache",
        ),
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
        Mapping::new(Absolute, "/var/run/docker.sock", "/var/run/docker.sock"),
    ];
    IntoIterator::into_iter(mappings)
}

#[derive(Debug, Copy, Clone)]
pub enum MappingSourceType {
    Sources,
    WorkDir,
    GlobalWorkDir,
    Absolute,
}

#[derive(Debug, Copy, Clone)]
pub enum MappingType {
    Folder,
    File,
}

impl<'a> From<&'a HazeVolumeConfig> for Mapping<'a> {
    fn from(config: &'a HazeVolumeConfig) -> Self {
        let ty = if config.source.is_dir() {
            MappingType::Folder
        } else {
            MappingType::File
        };
        Mapping {
            source_type: MappingSourceType::Absolute,
            source: config.source.as_path(),
            target: config.target.as_path(),
            mapping_type: ty,
            read_only: config.read_only,
            map: true,
            create: config.create,
        }
    }
}
