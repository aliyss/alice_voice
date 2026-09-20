//! Catalog of the built in GLiNER models.
//!
//! One model is two files: a tokenizer and one ONNX graph. Both live in a
//! Hugging Face repository, so the daemon downloads them itself and needs
//! no Python toolchain. The quantized graph is the default: it is about
//! three times smaller than the full graph, and on a small encoder the
//! accuracy it loses is small.

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
    /// Identifier of the model, for example `gliner_small-v2.1`.
    pub id: &'static str,
    /// Name the settings page shows.
    pub name: &'static str,
    /// One sentence about the model.
    pub note: &'static str,
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

/// The tokenizer of the English models.
const TOKENIZER_EN: ModelFile = ModelFile {
    path: "tokenizer.json",
    size_bytes: 8_657_198,
};

/// The tokenizer of the multilingual model.
const TOKENIZER_MULTI: ModelFile = ModelFile {
    path: "tokenizer.json",
    size_bytes: 16_331_948,
};

/// The quantized graph of the small model.
const GRAPH_SMALL: ModelFile = ModelFile {
    path: "onnx/model_quantized.onnx",
    size_bytes: 183_403_734,
};

/// The quantized graph of the medium model.
const GRAPH_MEDIUM: ModelFile = ModelFile {
    path: "onnx/model_quantized.onnx",
    size_bytes: 255_347_355,
};

/// The quantized graph of the multilingual model.
const GRAPH_MULTI: ModelFile = ModelFile {
    path: "onnx/model_quantized.onnx",
    size_bytes: 349_120_924,
};

/// The files of the small model.
const FILES_SMALL: &[ModelFile] = &[TOKENIZER_EN, GRAPH_SMALL];

/// The files of the medium model.
const FILES_MEDIUM: &[ModelFile] = &[TOKENIZER_EN, GRAPH_MEDIUM];

/// The files of the multilingual model.
const FILES_MULTI: &[ModelFile] = &[TOKENIZER_MULTI, GRAPH_MULTI];

/// The small model, and the default of the resolver.
pub const SMALL: ModelSpec = ModelSpec {
    id: "gliner_small-v2.1",
    name: "GLiNER small v2.1",
    note: "Small and fast. Reads a few intents on a processor in a few milliseconds.",
    repo: "onnx-community/gliner_small-v2.1",
    files: FILES_SMALL,
};

/// The medium model.
pub const MEDIUM: ModelSpec = ModelSpec {
    id: "gliner_medium-v2.1",
    name: "GLiNER medium v2.1",
    note: "Reads more reliably than small, and needs about 40 percent more time.",
    repo: "onnx-community/gliner_medium-v2.1",
    files: FILES_MEDIUM,
};

/// The multilingual model.
pub const MULTI: ModelSpec = ModelSpec {
    id: "gliner_multi-v2.1",
    name: "GLiNER multi v2.1",
    note: "Reads more than fifty languages. The largest download.",
    repo: "onnx-community/gliner_multi-v2.1",
    files: FILES_MULTI,
};

/// Every model the daemon can download, smallest first.
pub const CATALOG: &[ModelSpec] = &[SMALL, MEDIUM, MULTI];

/// Read one model out of the catalog.
pub fn find(id: &str) -> Option<&'static ModelSpec> {
    let id = id.trim();
    CATALOG
        .iter()
        .find(|spec| spec.id == id)
        .or_else(|| CATALOG.iter().find(|spec| spec.id.eq_ignore_ascii_case(id)))
}

/// Read the model the settings page offers first.
pub fn default_spec() -> &'static ModelSpec {
    &SMALL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_holds_the_three_models() {
        let ids: Vec<&str> = CATALOG.iter().map(|spec| spec.id).collect();
        assert_eq!(
            ids,
            vec![
                "gliner_small-v2.1",
                "gliner_medium-v2.1",
                "gliner_multi-v2.1"
            ]
        );
    }

    #[test]
    fn find_matches_the_identifier_ignoring_case_and_space() {
        assert_eq!(
            find(" gliner_medium-v2.1 ").map(|spec| spec.id),
            Some("gliner_medium-v2.1")
        );
        assert!(find("gliner_large-v2.1").is_none());
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
            assert!(spec.size_bytes() > 100_000_000);
        }
    }

    #[test]
    fn url_points_at_the_repository() {
        let spec = find("gliner_small-v2.1").expect("the model is in the catalog");
        assert_eq!(
            spec.url(&spec.files[0]),
            "https://huggingface.co/onnx-community/gliner_small-v2.1/resolve/main/tokenizer.json"
        );
    }

    #[test]
    fn the_default_model_is_the_smallest() {
        assert_eq!(default_spec().id, "gliner_small-v2.1");
    }
}
