use baldguard::{Database, Db, SendUpdate, Session};
use std::{collections::HashMap, process::exit, sync::Arc, time::Duration};
use teloxide::{
    prelude::Requester,
    types::{ChatId, ChatMemberStatus, Message},
    Bot,
};
use tokio::sync::Mutex;

type Sessions = Arc<Mutex<HashMap<ChatId, Session>>>;

async fn session_cleanup_routine(sessions: Sessions) {
    let timeout_duration = Duration::from_secs(600);
    let cleanup_interval = Duration::from_secs(60);
    loop {
        tokio::time::sleep(cleanup_interval).await;

        let mut sessions_lock = sessions.lock().await;

        sessions_lock.retain(|&_, session| !session.is_timed_out(timeout_duration));
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting baldguard...");

    let connection_str = match std::env::var("MONGODB_CONNECTION_STRING") {
        Ok(value) => value,
        Err(_) => {
            log::error!("MONGODB_CONNECTION_STRING not set");
            exit(1)
        }
    };

    let token = match std::env::var("BOT_TOKEN") {
        Ok(value) => value,
        Err(_) => {
            log::error!("BOT_TOKEN not set");
            exit(1)
        }
    };

    let sessions: Sessions = Arc::new(Mutex::new(HashMap::new()));
    let sessions_clone = sessions.clone();
    let database: Database = Arc::new(Mutex::new(match Db::new(&connection_str).await {
        Ok(db) => db,
        Err(e) => {
            log::error!("Failed to create database: {e}");
            exit(1)
        }
    }));

    tokio::spawn(async move { session_cleanup_routine(sessions_clone) });

    let bot = Bot::new(token);
    teloxide::repl(bot, move |bot: Bot, message: Message| {
        let sessions = Arc::clone(&sessions);
        let database = Arc::clone(&database);
        async move {
            let chat_id = message.chat.id;
            let mut sessions_lock = sessions.lock().await;

            let session = if sessions_lock.contains_key(&chat_id) {
                sessions_lock.get_mut(&chat_id).unwrap()
            } else {
                match Session::new(database, chat_id).await {
                    Ok(session) => {
                        sessions_lock.insert(chat_id, session);
                        sessions_lock.get_mut(&chat_id).unwrap()
                    }
                    Err(e) => {
                        log::error!("Failed to create session for {chat_id}: {e}");
                        return Ok(());
                    }
                }
            };

            let mut is_admin = false;
            if message.chat.is_private() {
                is_admin = true;
            } else {
                if let Some(user_id) = message.from.clone().map(|u| u.id) {
                    match bot.get_chat_administrators(chat_id).await {
                        Ok(admins) => {
                            is_admin = admins.iter().any(|member| {
                                member.user.id == user_id
                                    && matches!(
                                        member.status(),
                                        ChatMemberStatus::Administrator | ChatMemberStatus::Owner
                                    )
                            })
                        }
                        Err(e) => {
                            log::error!("Failed to get chat administrators for {chat_id}: {e}");
                        }
                    }
                }
            }

            match session.handle_message(message, is_admin).await {
                Ok(updates) => {
                    for update in updates {
                        match update {
                            SendUpdate::Message(text) => {
                                if let Err(e) = bot.send_message(chat_id, text).await {
                                    log::error!("Failed to send message: {e}");
                                }
                            }
                            SendUpdate::DeleteMessage(message_id) => {
                                if let Err(e) = bot.delete_message(chat_id, message_id).await {
                                    log::error!("Failed to delete message: {e}");
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to handle message from {chat_id}: {e}");
                }
            }
            Ok(())
        }
    })
    .await;
}
