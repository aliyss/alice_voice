//! Catalog of the built in models of the router.
//!
//! Two models are read. A bi-encoder turns one text into one vector, so a
//! message and an intent can be compared as meaning rather than as
//! spelling. A cross-encoder reads a message and one candidate together
//! and reports how well the two fit, which is a surer question and a
//! slower one, so it runs over the short list alone.
//!
//! Both are one ONNX graph and one tokenizer in a Hugging Face
//! repository, so the daemon downloads them itself and needs no Python
//! toolchain. The repository publishes a graph for the browser runtime,
//! which ONNX Runtime reads as it is.

/// What a built in model reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The model turns a text into one vector.
    Embedding,
    /// The model reads a message and a candidate together and scores the pair.
    Reranker,
}

impl Role {
    /// The name the settings page shows for the role.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Embedding => "embeddings",
            Self::Reranker => "reranker",
        }
    }
}

/// One file of a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelFile {
    /// Path of the file inside the model directory and inside the repository.
    pub path: &'static str,
    /// Size of the file in bytes, as the repository reports it.
    pub size_bytes: u64,
}

/// One model the daemon can download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelSpec {
    /// Identifier of the model, for example `bge-small-en-v1.5`.
    pub id: &'static str,
    /// Name the settings page shows.
    pub name: &'static str,
    /// One sentence about the model.
    pub note: &'static str,
    /// What the model reads.
    pub role: Role,
    /// Repository on Hugging Face.
    pub repo: &'static str,
    /// Files that belong to the model.
    pub files: &'static [ModelFile],
}

impl ModelSpec {
    /// Size of the whole download in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.size_bytes).sum()
    }

    /// Address one file is downloaded from.
    pub fn url(&self, file: &ModelFile) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            self.repo, file.path
        )
    }
}

/// The tokenizer of a model.
const TOKENIZER: ModelFile = ModelFile {
    path: "tokenizer.json",
    size_bytes: 711_396,
};

/// The graph of the embedding model.
const GRAPH_BGE_SMALL: ModelFile = ModelFile {
    path: "onnx/model.onnx",
    size_bytes: 133_093_490,
};

/// The graph of the reranker.
const GRAPH_RERANKER: ModelFile = ModelFile {
    path: "onnx/model.onnx",
    size_bytes: 90_992_115,
};

/// The files of the embedding model.
const FILES_BGE_SMALL: &[ModelFile] = &[TOKENIZER, GRAPH_BGE_SMALL];

/// The files of the reranker.
const FILES_RERANKER: &[ModelFile] = &[TOKENIZER, GRAPH_RERANKER];

/// The embedding model, and the default of the retrieval stage.
pub const BGE_SMALL: ModelSpec = ModelSpec {
    id: "bge-small-en-v1.5",
    name: "BGE small en v1.5",
    note: "Turns a message and an intent into vectors. About 133 MB, and one short text costs a few milliseconds on a processor.",
    role: Role::Embedding,
    repo: "Xenova/bge-small-en-v1.5",
    files: FILES_BGE_SMALL,
};

/// The reranker, and the default of a reranking decision stage.
pub const RERANKER: ModelSpec = ModelSpec {
    id: "ms-marco-MiniLM-L-6-v2",
    name: "ms-marco MiniLM L6 v2",
    note: "Reads a message and one candidate together and reports how well the two fit. About 91 MB, and one pair costs a few milliseconds on a processor.",
    role: Role::Reranker,
    repo: "Xenova/ms-marco-MiniLM-L-6-v2",
    files: FILES_RERANKER,
};

/// Every model the daemon can download, embeddings first.
pub const CATALOG: &[ModelSpec] = &[BGE_SMALL, RERANKER];

/// Read one model out of the catalog.
pub fn find(id: &str) -> Option<&'static ModelSpec> {
    let id = id.trim();
    CATALOG
        .iter()
        .find(|spec| spec.id == id)
        .or_else(|| CATALOG.iter().find(|spec| spec.id.eq_ignore_ascii_case(id)))
}

/// Read one model of one role out of the catalog.
///
/// A stage names the model it wants and the role it wants it for, so the
/// settings page cannot store an embedding model as a reranker.
pub fn find_role(id: &str, role: Role) -> Option<&'static ModelSpec> {
    find(id).filter(|spec| spec.role == role)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_holds_an_embedding_model_and_a_reranker() {
        let roles: Vec<Role> = CATALOG.iter().map(|spec| spec.role).collect();
        assert_eq!(roles, vec![Role::Embedding, Role::Reranker]);
    }

    #[test]
    fn find_matches_the_identifier_ignoring_case_and_space() {
        assert_eq!(
            find(" BGE-Small-EN-v1.5 ").map(|spec| spec.id),
            Some("bge-small-en-v1.5")
        );
        assert!(find("bge-large-en-v1.5").is_none());
    }

    #[test]
    fn a_stage_reads_a_model_of_its_own_role() {
        assert!(find_role("bge-small-en-v1.5", Role::Embedding).is_some());
        assert!(find_role("bge-small-en-v1.5", Role::Reranker).is_none());
        assert!(find_role("ms-marco-MiniLM-L-6-v2", Role::Reranker).is_some());
    }

    #[test]
    fn a_model_carries_a_tokenizer_and_a_graph() {
        for spec in CATALOG {
            assert!(
                spec.files.iter().any(|file| file.path == "tokenizer.json"),
                "{} has no tokenizer",
                spec.id
            );
            assert!(
                spec.files.iter().any(|file| file.path.ends_with(".onnx")),
                "{} has no graph",
                spec.id
            );
            assert!(spec.size_bytes() > 1_000_000);
        }
    }

    #[test]
    fn url_points_at_the_repository() {
        assert_eq!(
            BGE_SMALL.url(&BGE_SMALL.files[1]),
            "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/onnx/model.onnx"
        );
    }
}
