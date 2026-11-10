//! Indexed-file records.
use super::{db_error, file_from, file_props, Database, FILE};
use crate::errors::Result;
use crate::types::{FileKind, FileRecord};

impl Database {
    /// Inserts or replaces file records.
    pub async fn upsert_files(&self, files: &[FileRecord]) -> Result<()> {
        let mut new = Vec::new();
        for file in files {
            match self
                .find("path", &file.path, "upsert_files")?
                .into_iter()
                .next()
            {
                Some(gid) => {
                    for (key, value) in file_props(file) {
                        self.db
                            .set_node_property(gid, key.as_str(), value)
                            .map_err(|e| db_error("upsert_files", &e))?;
                    }
                }
                None => new.push(file_props(file)),
            }
        }
        if !new.is_empty() {
            self.db
                .batch_create_nodes_with_props(FILE, new)
                .map_err(|e| db_error("upsert_files", &e))?;
        }
        Ok(())
    }

    /// Inserts or replaces one file record.
    pub async fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.upsert_files(std::slice::from_ref(file)).await
    }

    /// Returns the record of one file.
    pub async fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let Some(gid) = self.find("path", path, "get_file")?.into_iter().next() else {
            return Ok(None);
        };
        Ok(self.graph_node(gid, "get_file")?.map(|n| file_from(&n)))
    }

    /// Returns every file record.
    pub async fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        self.files_matching(|_| true, "get_all_files")
    }

    /// Returns the records of source-code files.
    pub async fn get_code_files(&self) -> Result<Vec<FileRecord>> {
        self.files_matching(|f| f.kind == FileKind::Code, "get_code_files")
    }

    /// Deletes a file record and, first, its nodes, as the libSQL backend
    /// does.
    pub async fn delete_file(&self, path: &str) -> Result<()> {
        self.delete_nodes_by_file(path).await?;
        for gid in self.find("path", path, "delete_file")? {
            self.db
                .delete_node(gid)
                .map_err(|e| db_error("delete_file", &e))?;
        }
        Ok(())
    }

    fn files_matching(
        &self,
        keep: impl Fn(&FileRecord) -> bool,
        operation: &str,
    ) -> Result<Vec<FileRecord>> {
        Ok(self
            .scan_label(FILE, operation)?
            .iter()
            .map(|p| file_from(p.as_ref()))
            .filter(|f| keep(f))
            .collect())
    }
}
