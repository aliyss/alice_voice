//! Pending script domain of the daemon.
//!
//! A message that no intent matched may be answered with a shell script the
//! model wrote. The daemon runs no script of its own accord, so it stores
//! the script and waits for the decision of the user. The store outlives a
//! closed socket and a restart, so a decision that arrives later still
//! finds the script it belongs to.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::pending_script::entity::pending_script;

pub mod entity;

/// Status of a script that waits for the decision of the user.
pub const STATUS_PENDING: &str = "pending";

/// Status of a script the user approved and the daemon may run.
pub const STATUS_APPROVED: &str = "approved";

/// Status of a script the user denied. The daemon runs it never.
pub const STATUS_DENIED: &str = "denied";

/// Status of a script the daemon ran after the approval.
pub const STATUS_RAN: &str = "ran";

/// One script that waits for the decision of the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingScript {
    /// Stable identifier of the script.
    pub id: Uuid,
    /// Conversation the turn belongs to, or null when the queue is off.
    pub conversation_id: Option<Uuid>,
    /// The message the model wrote the script for.
    pub request_text: String,
    /// One sentence about what the script does.
    pub summary: String,
    /// The shell script.
    pub script: String,
    /// How rough the script is on the machine, between 0 and 100.
    pub destructiveness: u8,
    /// The decision of the user.
    pub status: String,
    /// Time the daemon stored the script.
    pub created_at: DateTime<Utc>,
}

/// One script that enters the store.
#[derive(Clone, Debug)]
pub struct NewPendingScript {
    /// Stable identifier of the script.
    pub id: Uuid,
    /// Conversation the turn belongs to, or null when the queue is off.
    pub conversation_id: Option<Uuid>,
    /// The message the model wrote the script for.
    pub request_text: String,
    /// One sentence about what the script does.
    pub summary: String,
    /// The shell script.
    pub script: String,
    /// How rough the script is on the machine, between 0 and 100.
    pub destructiveness: u8,
    /// Time the daemon stored the script.
    pub created_at: DateTime<Utc>,
}

/// What a claim of one script found.
///
/// A claim moves the script out of the pending status, so two approvals
/// that arrive at once cannot both run the script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Claim {
    /// The script was pending, and the caller now owns its next step.
    Claimed(PendingScript),
    /// Another decision reached the script first.
    Decided(String),
    /// No script holds that identifier.
    Missing,
}

/// Service that stores the scripts which wait for approval.
#[derive(Clone, Debug)]
pub struct PendingScriptService {
    db: DatabaseConnection,
}

impl PendingScriptService {
    /// Create a new pending script service.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Store one script with the pending status.
    pub async fn store(&self, new: &NewPendingScript) -> Result<(), sea_orm::DbErr> {
        let active = pending_script::ActiveModel {
            id: Set(new.id),
            conversation_id: Set(new.conversation_id),
            request_text: Set(new.request_text.clone()),
            summary: Set(new.summary.clone()),
            script: Set(new.script.clone()),
            destructiveness: Set(i32::from(new.destructiveness)),
            status: Set(STATUS_PENDING.to_string()),
            created_at: Set(new.created_at.into()),
        };
        pending_script::Entity::insert(active)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// Find one script, or null when it does not exist.
    pub async fn get(&self, id: Uuid) -> Result<Option<PendingScript>, sea_orm::DbErr> {
        let row = pending_script::Entity::find_by_id(id).one(&self.db).await?;
        Ok(row.map(PendingScript::from_model))
    }

    /// Move one pending script to `next` and return what the move found.
    ///
    /// The move reads the row, checks that it still waits, and writes the
    /// new status with the pending status in the filter. Two callers that
    /// arrive together therefore claim the script once: the second write
    /// matches no row and reads as a decision that came first.
    pub async fn claim(&self, id: Uuid, next: &str) -> Result<Claim, sea_orm::DbErr> {
        let Some(found) = self.get(id).await? else {
            return Ok(Claim::Missing);
        };
        if found.status != STATUS_PENDING {
            return Ok(Claim::Decided(found.status));
        }

        let active = pending_script::ActiveModel {
            id: Set(id),
            status: Set(next.to_string()),
            ..Default::default()
        };
        let moved = pending_script::Entity::update_many()
            .set(active)
            .filter(pending_script::Column::Id.eq(id))
            .filter(pending_script::Column::Status.eq(STATUS_PENDING))
            .exec(&self.db)
            .await?;
        if moved.rows_affected == 0 {
            return Ok(Claim::Decided(found.status));
        }
        Ok(match self.get(id).await? {
            Some(script) => Claim::Claimed(script),
            None => Claim::Missing,
        })
    }

    /// Mark one approved script as run.
    ///
    /// A script the daemon ran keeps the reason of its run whether the
    /// command answered or failed, so the record never reads as pending.
    pub async fn mark_ran(&self, id: Uuid) -> Result<(), sea_orm::DbErr> {
        let active = pending_script::ActiveModel {
            id: Set(id),
            status: Set(STATUS_RAN.to_string()),
            ..Default::default()
        };
        pending_script::Entity::update_many()
            .set(active)
            .filter(pending_script::Column::Id.eq(id))
            .filter(pending_script::Column::Status.eq(STATUS_APPROVED))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

impl PendingScript {
    /// Read one stored script.
    fn from_model(model: pending_script::Model) -> Self {
        Self {
            id: model.id,
            conversation_id: model.conversation_id,
            request_text: model.request_text,
            summary: model.summary,
            script: model.script,
            destructiveness: model.destructiveness.clamp(0, 100) as u8,
            status: model.status,
            created_at: model.created_at.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use alice_core::config::CoreConfig;

    /// Build a store with an in memory database for the tests.
    async fn store() -> PendingScriptService {
        let config = CoreConfig {
            database: alice_core::config::DatabaseConfig {
                url: "sqlite::memory:".to_string(),
                ..CoreConfig::default().database
            },
            ..CoreConfig::default()
        };
        let connection = db::connect(&config).await.expect("the database opens");
        PendingScriptService::new(connection)
    }

    /// Build one script for the tests.
    fn script(id: Uuid) -> NewPendingScript {
        NewPendingScript {
            id,
            conversation_id: Some(Uuid::new_v4()),
            request_text: "list the largest files".to_string(),
            summary: "List the largest files".to_string(),
            script: "du -ah | sort -rh | head".to_string(),
            destructiveness: 5,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn a_stored_script_reads_back() {
        let service = store().await;
        let id = Uuid::new_v4();
        service.store(&script(id)).await.expect("the script stores");

        let found = service.get(id).await.expect("the read works");

        let found = found.expect("the script is stored");
        assert_eq!(found.status, STATUS_PENDING);
        assert_eq!(found.destructiveness, 5);
        assert_eq!(found.script, "du -ah | sort -rh | head");
    }

    #[tokio::test]
    async fn a_claim_moves_the_script_out_of_pending() {
        let service = store().await;
        let id = Uuid::new_v4();
        service.store(&script(id)).await.expect("the script stores");

        let claim = service.claim(id, STATUS_APPROVED).await.expect("the claim");

        let Claim::Claimed(found) = claim else {
            panic!("the pending script is claimed");
        };
        assert_eq!(found.status, STATUS_APPROVED);
        assert_eq!(
            service
                .get(id)
                .await
                .expect("the read works")
                .map(|s| s.status),
            Some(STATUS_APPROVED.to_string())
        );
    }

    #[tokio::test]
    async fn a_second_claim_reports_the_first_decision() {
        let service = store().await;
        let id = Uuid::new_v4();
        service.store(&script(id)).await.expect("the script stores");
        service
            .claim(id, STATUS_APPROVED)
            .await
            .expect("the first claim");

        let second = service.claim(id, STATUS_DENIED).await.expect("the claim");

        assert_eq!(second, Claim::Decided(STATUS_APPROVED.to_string()));
    }

    #[tokio::test]
    async fn a_missing_script_reads_as_missing() {
        let service = store().await;
        let claim = service.claim(Uuid::new_v4(), STATUS_APPROVED).await;
        assert_eq!(claim.expect("the claim"), Claim::Missing);
    }

    #[tokio::test]
    async fn a_run_marks_an_approved_script() {
        let service = store().await;
        let id = Uuid::new_v4();
        service.store(&script(id)).await.expect("the script stores");
        service.claim(id, STATUS_APPROVED).await.expect("the claim");

        service.mark_ran(id).await.expect("the run is marked");

        assert_eq!(
            service
                .get(id)
                .await
                .expect("the read works")
                .map(|s| s.status),
            Some(STATUS_RAN.to_string())
        );
    }
}
