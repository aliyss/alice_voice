//! Evaluation of the memory over a scripted history.
//!
//! The unit tests of the store read one behaviour at a time. This module
//! replays a whole history instead: several sessions, each with what the
//! user said, what the assistant answered, and what the reader read out of
//! the turn. It uses the same functions a live turn uses, so a change that
//! breaks the memory over a history fails here rather than in production.
//!
//! The categories follow the questions a long history asks, the ones the
//! benchmarks of the field measure: a knowledge update, a fact repeated
//! across sessions, two sessions that name one thing, a question that has
//! to reach the session that answered it, and a turn that spills something
//! the memory must not keep.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::db;
use crate::librarian::extract::parse_extraction;
use crate::librarian::store::LibrarianStore;

/// One turn of the script.
struct Session {
    /// Days after the start of the script.
    day: i64,
    /// The words the user sent.
    text: &'static str,
    /// The answer the assistant gave.
    reply: &'static str,
    /// The answer the reader gave for the turn, as JSON.
    reading: &'static str,
}

/// The day every script starts on, so the age of a fact does not depend
/// on the day of the run.
fn base() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-01-01T09:00:00Z")
        .expect("the date reads")
        .with_timezone(&Utc)
}

/// Build a store on an in memory database with the librarian schema.
async fn store() -> LibrarianStore {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("an in memory database answers");
    db::init_schema(&db).await.expect("the schema is created");
    LibrarianStore::new(db)
}

/// Replay one script through the write path of the memory.
///
/// Each turn becomes an episode of its own, as it does in a live daemon,
/// and the reading of the turn goes through the gate and the resolution
/// exactly as the worker sends it.
async fn replay(sessions: &[Session]) -> LibrarianStore {
    let store = store().await;
    for session in sessions {
        let at = base() + Duration::days(session.day);
        let facts = parse_extraction(session.reading);
        store
            .apply_facts(Uuid::new_v4(), &facts, at)
            .await
            .expect("the reading is written");
        // The text and the reply of the turn do not reach the write path
        // of the facts, but the script keeps them, so the history reads
        // like the conversation it stands for.
        assert!(!session.text.trim().is_empty() && !session.reply.trim().is_empty());
    }
    store
}

/// Read the facts of the only concept the memory holds.
async fn the_only_node(store: &LibrarianStore) -> alice_core::dto::MemoryNodeDto {
    let nodes = store.query("", 10).await.expect("the memory is read");
    assert_eq!(nodes.len(), 1, "the memory holds one concept");
    nodes[0].clone()
}

#[tokio::test]
async fn a_later_session_replaces_what_an_earlier_one_taught() {
    let store = replay(&[
        Session {
            day: 0,
            text: "I moved to Singapore last week",
            reply: "Noted, Singapore it is.",
            reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"lives_in","value":"Singapore","confidence":0.9,"importance":0.9}]}"#,
        },
        Session {
            day: 40,
            text: "I live in Berlin now",
            reply: "Understood.",
            reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"lives_in","value":"Berlin","confidence":0.9,"importance":0.9}]}"#,
        },
    ])
    .await;

    let node = the_only_node(&store).await;
    assert_eq!(node.facts.len(), 2, "the history keeps what it replaced");
    let current = node
        .facts
        .iter()
        .find(|fact| fact.current)
        .expect("the memory holds a current fact");
    assert_eq!(current.value, "Berlin");
    // The replaced fact reports the time it stopped being true, so the
    // memory can still answer what it held in April.
    let replaced = node
        .facts
        .iter()
        .find(|fact| !fact.current)
        .expect("the memory keeps the older value");
    assert_eq!(replaced.value, "Singapore");
    assert!(replaced.invalid_at.is_some());

    // The counters tell now from the history: 2 edges, 1 of them true.
    assert_eq!(store.count_edges().await.expect("the count reads"), 2);
    assert_eq!(
        store.count_current_edges().await.expect("the count reads"),
        1
    );
}

#[tokio::test]
async fn a_fact_repeated_in_a_later_session_is_one_fact() {
    let store = replay(&[
        Session {
            day: 0,
            text: "I use Neovim",
            reply: "Noted.",
            reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"uses","value":"Neovim"}]}"#,
        },
        Session {
            day: 3,
            text: "which editor do I use again?",
            reply: "You use Neovim.",
            reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"uses","value":"neovim "}]}"#,
        },
    ])
    .await;

    let node = the_only_node(&store).await;
    assert_eq!(node.facts.len(), 1, "one fact, not two");
    // The second session is a sighting rather than a repeat to discard.
    assert_eq!(node.facts[0].confirmations, 1);
    assert!(node.facts[0].confidence > 0.6);
    assert_eq!(
        store
            .count_confirmed_edges()
            .await
            .expect("the count reads"),
        1
    );
}

#[tokio::test]
async fn two_sessions_that_name_one_thing_teach_one_concept() {
    let store = replay(&[
        Session {
            day: 0,
            text: "my name is Flurin",
            reply: "Hello Flurin.",
            reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"is_called","value":"Flurin"}]}"#,
        },
        Session {
            day: 1,
            text: "my editor is Neovim",
            reply: "Noted.",
            reading: r#"{"facts":[{"subject":"flurin","title":"Flurin","relation":"uses","value":"Neovim"}]}"#,
        },
    ])
    .await;

    let node = the_only_node(&store).await;
    assert_eq!(node.facts.len(), 2, "both turns landed on one concept");
    assert_eq!(node.key, "user", "the first key of a concept keeps it");
}

#[tokio::test]
async fn the_seed_reaches_the_session_that_answered_the_question() {
    let store = replay(&[
        Session {
            day: 0,
            text: "the project runs on Tokio",
            reply: "Noted.",
            reading: r#"{"facts":[{"subject":"project_alice","title":"Project Alice","relation":"runs_on","value":"Tokio","importance":0.9}]}"#,
        },
        Session {
            day: 5,
            text: "I use Neovim",
            reply: "Noted.",
            reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"uses","value":"Neovim","importance":0.9}]}"#,
        },
        Session {
            day: 9,
            text: "what does the alice project run on?",
            reply: "Tokio.",
            reading: r#"{"facts":[]}"#,
        },
    ])
    .await;

    let seed = store
        .seed("what does the alice project run on?", 1)
        .await
        .expect("the seed is read");

    // The newest concept of the store is the user, and the question is
    // about the project, so a seed that only read the newest would answer
    // the question with the wrong concept.
    assert!(seed.contains("Project Alice"));
    assert!(seed.contains("runs_on is Tokio"));
}

#[tokio::test]
async fn the_seed_of_an_old_fact_says_it_is_old() {
    let store = replay(&[Session {
        day: 0,
        text: "I live in Singapore",
        reply: "Noted.",
        reading: r#"{"facts":[{"subject":"user","title":"Flurin","relation":"lives_in","value":"Singapore"}]}"#,
    }])
    .await;

    let seed = store.seed("user", 1).await.expect("the seed is read");

    // The script starts well before today, so the fact nothing confirmed
    // reads as old and the model can question rather than trust it.
    assert!(
        seed.contains("last confirmed") && seed.contains("days ago"),
        "got {seed}"
    );
}

#[tokio::test]
async fn a_turn_that_spills_a_credential_still_teaches_the_rest() {
    let store = replay(&[Session {
        day: 0,
        text: "my editor is Neovim and my token is ghp_9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c",
        reply: "I would rather not keep the token.",
        reading: r#"{"facts":[
            {"subject":"user","title":"Flurin","relation":"uses","value":"Neovim"},
            {"subject":"user","title":"Flurin","relation":"has_token","value":"ghp_9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c"},
            {"subject":"user","title":"Flurin","relation":"api_key","value":"sk-live-2f9a"}
        ]}"#,
    }])
    .await;

    let node = the_only_node(&store).await;
    assert_eq!(node.facts.len(), 1, "only the fact about the editor stays");
    assert_eq!(node.facts[0].value, "Neovim");
}
