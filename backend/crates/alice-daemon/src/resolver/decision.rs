//! The decision interface of the resolver.
//!
//! A decision reads an unstructured state and chooses one option from a
//! set the caller defines at runtime. The caller sends the options with
//! the request and reads the chosen one back, so the model never needs to
//! know the option set in advance.

/// One option of a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionOption {
    /// Identifier the caller reads back. The model never sees it.
    pub id: String,
    /// Short label the model reads.
    pub label: String,
    /// One sentence that explains the option.
    pub detail: String,
}

/// One decision the resolver asks the model to make.
#[derive(Clone, Debug)]
pub struct Decision {
    /// Unstructured context. The model reads it as the state.
    pub state: String,
    /// The question the model answers.
    pub question: String,
    /// The options the model chooses from.
    pub options: Vec<DecisionOption>,
}

/// The option the model chose.
#[derive(Clone, Debug, PartialEq)]
pub struct DecisionOutcome {
    /// Identifier of the chosen option.
    pub option_id: String,
    /// Probability of the chosen option, or null when the model sent none.
    pub confidence: Option<f32>,
}
