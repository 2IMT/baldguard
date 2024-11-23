mod migrations;

use super::error::GenericError;
use baldguard_language::tree::Expression;
use mongodb::{bson::doc, options::IndexOptions, Client, Collection, Database, IndexModel};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub debug_print: bool,
    pub report_filtered: bool,
    pub report_invalid_commands: bool,
    pub filter_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            debug_print: false,
            report_filtered: true,
            report_invalid_commands: true,
            filter_enabled: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Chat {
    pub chat_id: i64,
    pub filter: Option<Expression>,
    pub settings: Settings,
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            chat_id: 0,
            filter: None,
            settings: Settings::default(),
        }
    }
}

pub struct Db {
    chats: Collection<Chat>,
}

impl Db {
    pub async fn new(connection_string: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let client = Client::with_uri_str(connection_string).await?;
        let database = client.database("baldguard");
        let chats: Collection<Chat> = database.collection("chats");

        let index_keys = doc! { "chat_id": 1 };
        let index_options = IndexOptions::builder()
            .unique(true)
            .name(Some("chat_id_unique_ascending".to_string()))
            .build();
        let index_model = IndexModel::builder()
            .keys(index_keys)
            .options(index_options)
            .build();
        chats.create_index(index_model).await?;

        if let Err(e) = migrate(&database).await {
            return Err(Box::new(GenericError::from(format!(
                "database migration error: {e}"
            ))));
        }

        Ok(Db { chats })
    }

    pub async fn find_chat_by_id(&self, chat_id: i64) -> Result<Chat, Box<dyn Error>> {
        match self.chats.find_one(doc! { "chat_id": chat_id }).await? {
            Some(chat) => Ok(chat),
            None => {
                let mut chat = Chat::default();
                chat.chat_id = chat_id;
                self.chats.insert_one(&chat).await?;
                Ok(chat)
            }
        }
    }

    pub async fn insert_chat(&self, chat: &Chat) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.chats
            .replace_one(doc! { "chat_id": chat.chat_id }, chat)
            .upsert(true)
            .await?;

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Migration {
    name: String,
}

async fn migrate(db: &Database) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Migrating the database...");

    let migrations: Collection<Migration> = db.collection("migrations");

    let index_keys = doc! { "name": 1 };
    let index_options = IndexOptions::builder()
        .unique(true)
        .name(Some("name_unique_ascending".to_string()))
        .build();
    let index_model = IndexModel::builder()
        .keys(index_keys)
        .options(index_options)
        .build();
    migrations.create_index(index_model).await?;

    for mut migration_action in migrations::get_vec() {
        if let None = migrations
            .find_one(doc! { "name": migration_action.name.to_string() })
            .await?
        {
            log::info!("Applying migration {}...", migration_action.name);
            migration_action.run(db.clone()).await?;
            migrations
                .insert_one(Migration {
                    name: migration_action.name,
                })
                .await?;
        }
    }

    Ok(())
}
