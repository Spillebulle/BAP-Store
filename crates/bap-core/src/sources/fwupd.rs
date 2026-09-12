//! TODO: the fwupd source. See docs/architecture.md and CLAUDE.md.

use crate::model::*;
use crate::{Op, Query, Result, Source};

pub struct Fwupd;

impl Fwupd {
    pub fn new(_system: &SystemInfo) -> Fwupd {
        Fwupd
    }
}

impl Source for Fwupd {
    fn kind(&self) -> SourceKind {
        SourceKind::Fwupd
    }
    fn status(&self) -> SourceStatus {
        super::not_built(self.kind())
    }
    fn search(&self, _query: &Query) -> Result<Vec<Package>> {
        Ok(Vec::new())
    }
    fn installed(&self) -> Result<Vec<Package>> {
        Ok(Vec::new())
    }
    fn updates(&self) -> Result<Vec<Update>> {
        Ok(Vec::new())
    }
    fn details(&self, id: &str) -> Result<Package> {
        Err(crate::Error::from_source(
            self.kind(),
            format!("{id} is not known to this source."),
        ))
    }
    fn plan(&self, _op: &Op) -> Result<Vec<Step>> {
        Ok(Vec::new())
    }
}
