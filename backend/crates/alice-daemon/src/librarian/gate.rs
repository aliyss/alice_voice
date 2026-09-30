//! Write gate of the librarian.
//!
//! The memory reaches the prompt of a later turn, so a fact that must
//! never be repeated must never become one: a credential, a token, a
//! whole sentence, or a value that names a secret stays out. The gate is
//! deliberately blunt and cheap, because the reader of a turn is a model
//! and the memory is a store the daemon repeats in its own words.

use crate::librarian::extract::ExtractedFact;

/// The verdict of the gate on one fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The fact belongs in the memory.
    Keep,
    /// The fact stays out, for the reason.
    Drop(&'static str),
}

/// The longest value a fact may carry, in characters.
///
/// A value names a thing rather than describes it, so a long one is
/// either a mistake or text that belongs in the conversation.
const MAX_VALUE_CHARS: usize = 200;

/// The longest relation a fact may name, in characters.
const MAX_RELATION_CHARS: usize = 60;

/// The most words a value may hold.
const MAX_VALUE_WORDS: usize = 12;

/// The shortest run of characters that reads as a token.
const TOKEN_RUN_CHARS: usize = 32;

/// The words that name an act of keeping a secret.
///
/// A relation that names one of them describes a credential rather than a
/// fact about the user, and a value that holds one is the credential
/// itself, or the sentence around it.
const SECRET_WORDS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "api_key",
    "apikey",
    "access_key",
    "private_key",
    "credential",
];

/// Decide whether one fact belongs in the memory.
///
/// The gate answers for one fact at a time, so the caller drops the fact
/// and keeps the rest of the turn. A fact the gate drops is not an error:
/// a turn that says a secret is a turn whose other facts are still worth
/// keeping.
pub fn judge(fact: &ExtractedFact) -> Verdict {
    let relation = fact.relation.to_lowercase();
    let value = fact.value.trim();
    if names_a_secret(&relation) {
        return Verdict::Drop("the relation names a secret");
    }
    if value.chars().count() > MAX_VALUE_CHARS || fact.relation.chars().count() > MAX_RELATION_CHARS
    {
        return Verdict::Drop("the fact is too long");
    }
    if value.split_whitespace().count() > MAX_VALUE_WORDS {
        return Verdict::Drop("the value is a sentence");
    }
    if names_a_secret(&value.to_lowercase()) || value.to_lowercase().contains("-----begin") {
        return Verdict::Drop("the value names a secret");
    }
    if value.split_whitespace().any(looks_like_token) {
        return Verdict::Drop("the value reads as a token");
    }
    Verdict::Keep
}

/// Whether one text names an act of keeping a secret.
fn names_a_secret(text: &str) -> bool {
    SECRET_WORDS.iter().any(|word| text.contains(word))
}

/// Whether one word reads as a token rather than as a name.
///
/// A credential is a long run of letters, digits, and the separators of a
/// key, and it carries a digit. A long word of letters alone is a name,
/// so it passes: `Saskatchewan` is a place and
/// `sk-3f9a2c1b4d5e6f708192a3b4c5d6e7f8` is not.
fn looks_like_token(word: &str) -> bool {
    if word.chars().count() < TOKEN_RUN_CHARS {
        return false;
    }
    if !word
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '/' | '='))
    {
        return false;
    }
    word.chars().any(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one fact the way the reader answers it.
    fn fact(relation: &str, value: &str) -> ExtractedFact {
        ExtractedFact {
            subject: "user".to_string(),
            title: "User".to_string(),
            relation: relation.to_string(),
            value: value.to_string(),
            confidence: 0.6,
            importance: 0.5,
        }
    }

    #[test]
    fn a_fact_of_the_user_passes() {
        assert_eq!(judge(&fact("lives_in", "Zurich")), Verdict::Keep);
        assert_eq!(judge(&fact("prefers", "Neovim")), Verdict::Keep);
        assert_eq!(judge(&fact("uses", "NixOS")), Verdict::Keep);
    }

    #[test]
    fn a_relation_that_names_a_secret_stays_out() {
        assert_eq!(
            judge(&fact("api_key", "sk-live-1234")),
            Verdict::Drop("the relation names a secret")
        );
        assert_eq!(
            judge(&fact("password", "hunter2")),
            Verdict::Drop("the relation names a secret")
        );
    }

    #[test]
    fn a_token_stays_out() {
        // A key carries a digit and the separators of a key, so the memory
        // never repeats it in a later prompt.
        assert_eq!(
            judge(&fact("uses", "sk-3f9a2c1b4d5e6f708192a3b4c5d6e7f8")),
            Verdict::Drop("the value reads as a token")
        );
        assert_eq!(
            judge(&fact(
                "uses",
                "ghp_9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c"
            )),
            Verdict::Drop("the value reads as a token")
        );
    }

    #[test]
    fn a_long_name_passes() {
        // A long word of letters is a name rather than a key.
        assert_eq!(judge(&fact("lives_in", "Saskatchewan")), Verdict::Keep);
        assert_eq!(
            judge(&fact("lives_in", "Llanfairpwllgwyngyllgogerychwyrndrobwll")),
            Verdict::Keep
        );
    }

    #[test]
    fn a_value_that_names_a_secret_stays_out() {
        assert_eq!(
            judge(&fact("note", "my password is hunter2")),
            Verdict::Drop("the value names a secret")
        );
        assert_eq!(
            judge(&fact("note", "-----BEGIN OPENSSH PRIVATE KEY-----")),
            Verdict::Drop("the value names a secret")
        );
    }

    #[test]
    fn a_sentence_stays_out() {
        assert_eq!(
            judge(&fact(
                "asked_about",
                "the user asked me to look at the file and I read it"
            )),
            Verdict::Drop("the value is a sentence")
        );
    }

    #[test]
    fn an_enormous_value_stays_out() {
        let long = "a".repeat(MAX_VALUE_CHARS + 1);
        assert_eq!(
            judge(&fact("note", &long)),
            Verdict::Drop("the fact is too long")
        );
    }
}
