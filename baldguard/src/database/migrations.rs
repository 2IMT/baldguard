use futures::StreamExt;
use mongodb::{bson::doc, bson::Document, Collection, Database};
use std::{error::Error, future::Future, pin::Pin};

async fn move_filter_enabled_to_settings(db: Database) -> MigrationActionResult {
    let chats: Collection<Document> = db.collection("chats");
    let mut cursor = chats.find(doc! {}).await?;

    while let Some(doc) = cursor.next().await {
        let mut doc = doc?;
        if let Some(filter_enabled) = doc.remove("filter_enabled") {
            let mut settings = doc.get_document("settings")?.clone();
            settings.insert("filter_enabled", filter_enabled);

            chats
                .update_one(
                    doc! {
                        "_id": doc.get("_id").unwrap()
                    },
                    doc! {
                        "$set": {
                            "settings": settings.clone()
                        },
                        "$unset": {
                            "filter_enabled": ""
                        }
                    },
                )
                .await?;
        }
    }

    Ok(())
}

async fn add_report_command_success_to_settings(db: Database) -> MigrationActionResult {
    let chats: Collection<Document> = db.collection("chats");
    let mut cursor = chats.find(doc! {}).await?;

    while let Some(doc) = cursor.next().await {
        let doc = doc?;
        let mut settings = doc.get_document("settings")?.clone();
        settings.insert("report_command_success", true);

        chats
            .update_one(
                doc! {
                    "_id" : doc.get("_id").unwrap()
                },
                doc! {
                    "$set": {
                        "settings" : settings.clone()
                    }
                },
            )
            .await?;
    }

    Ok(())
}

pub fn get_vec() -> Vec<MigrationAction> {
    macro_rules! migration_action {
        ($name:ident) => {
            MigrationAction::new(stringify!($name).to_string(), $name)
        };
    }

    macro_rules! migration_actions {
        ($( $item:ident ),*) => {
            vec![
                $(
                    migration_action!($item)
                ),*
            ]
        };
    }

    migration_actions![
        move_filter_enabled_to_settings,
        add_report_command_success_to_settings
    ]
}

pub type MigrationActionResult = Result<(), Box<dyn Error + Send + Sync>>;

pub struct MigrationAction {
    pub name: String,
    pub action:
        Option<Box<dyn FnOnce(Database) -> Pin<Box<dyn Future<Output = MigrationActionResult>>>>>,
}

impl MigrationAction {
    fn new<F, Fut>(name: String, action: F) -> Self
    where
        F: FnOnce(Database) -> Fut + Send + 'static,
        Fut: Future<Output = MigrationActionResult> + 'static,
    {
        Self {
            name,
            action: Some(Box::new(move |db| Box::pin(action(db)))),
        }
    }

    pub async fn run(&mut self, db: Database) -> MigrationActionResult {
        let action = self
            .action
            .take()
            .expect("MigrationAction can only be run once");
        action(db).await
    }
}
