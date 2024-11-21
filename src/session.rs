use super::database::{Chat, Db};
use super::language::{
    evaluation::{evaluate, Value, Variables},
    grammar::ExpressionParser,
};
use std::{
    error::Error,
    fmt::Display,
    sync::Arc,
    time::{Duration, Instant},
};
use teloxide::types::{ChatId, Message, MessageId};
use tokio::sync::Mutex;

pub enum SendUpdate {
    Message(String),
    DeleteMessage(MessageId),
}

pub struct Session {
    chat_id: ChatId,
    db: Arc<Mutex<Db>>,
    parser: ExpressionParser,
    chat: Chat,
    variables: Variables,
    last_active: Instant,
}

impl Session {
    pub async fn new(db: Arc<Mutex<Db>>, chat_id: ChatId) -> Result<Self, Box<dyn Error>> {
        let db_lock = db.lock().await;
        let chat = db_lock.find_chat_by_id(chat_id.0).await?;
        drop(db_lock);
        Ok(Session {
            chat_id,
            db,
            parser: ExpressionParser::new(),
            chat,
            variables: Variables::new(),
            last_active: Instant::now(),
        })
    }

    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    pub fn refresh(&mut self) {
        self.last_active = Instant::now();
    }

    pub fn is_timed_out(&self, timeout_duration: Duration) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_active) > timeout_duration {
            return true;
        }

        false
    }

    pub async fn handle_message(
        &mut self,
        message: Message,
        from_admin: bool,
    ) -> Result<Vec<SendUpdate>, Box<dyn Error + Send + Sync>> {
        self.refresh();

        let mut result = Vec::with_capacity(5);
        let mut is_valid_command = false;
        match message.text() {
            Some(text) => match Command::new(text) {
                Ok(command) => {
                    if let Some(command) = command {
                        if command.requires_admin_rights() && !from_admin {
                            result.push(SendUpdate::Message(format!("error: permission denied")))
                        } else {
                            is_valid_command = true;
                            match command {
                                Command::SetFilter(arg) => match self.parser.parse(&arg) {
                                    Ok(expression) => self.chat.filter = Some(*expression),
                                    Err(e) => result
                                        .push(SendUpdate::Message(format!("parse error: {e}"))),
                                },
                                Command::SetDebugPrint(arg) => match self.parser.parse(&arg) {
                                    Ok(expression) => {
                                        match evaluate(&expression, &self.variables) {
                                            Ok(value) => match value {
                                                Value::Bool(value) => {
                                                    self.chat.settings.debug_print = value;
                                                }
                                                _ => result.push(SendUpdate::Message(
                                                    "error: expression evaluated to non-bool value"
                                                        .to_string(),
                                                )),
                                            },
                                            Err(e) => {
                                                result.push(SendUpdate::Message(format!(
                                                    "error: failed to evaluate expression: {e}"
                                                )));
                                            }
                                        }
                                    }
                                    Err(e) => result
                                        .push(SendUpdate::Message(format!("parse error: {e}"))),
                                },
                                Command::SetReportInvalidCommands(arg) => {
                                    match self.parser.parse(&arg) {
                                        Ok(expression) => {
                                            match evaluate(&expression, &self.variables) {
                                                Ok(value) => match value {
                                                    Value::Bool(value) => {
                                                        self.chat.settings.report_invalid_commands = value;
                                                    }
                                                    _ => result.push(SendUpdate::Message(
                                                        "error: expression evaluated to non-bool value"
                                                            .to_string(),
                                                    )),
                                                },
                                                Err(e) => {
                                                    result.push(SendUpdate::Message(format!(
                                                        "error: failed to evaluate expression: {e}"
                                                    )));
                                                }
                                            }
                                        }
                                        Err(e) => result
                                            .push(SendUpdate::Message(format!("parse error: {e}"))),
                                    }
                                }
                                Command::SetReportFiltered(arg) => match self.parser.parse(&arg) {
                                    Ok(expression) => {
                                        match evaluate(&expression, &self.variables) {
                                            Ok(value) => match value {
                                                Value::Bool(value) => {
                                                    self.chat.settings.report_filtered = value;
                                                }
                                                _ => result.push(SendUpdate::Message(
                                                    "error: expression evaluated to non-bool value"
                                                        .to_string(),
                                                )),
                                            },
                                            Err(e) => {
                                                result.push(SendUpdate::Message(format!(
                                                    "error: failed to evaluate expression: {e}"
                                                )));
                                            }
                                        }
                                    }
                                    Err(e) => result
                                        .push(SendUpdate::Message(format!("parse error: {e}"))),
                                },
                                Command::GetVariables => {
                                    if let Some(message) = message.reply_to_message() {
                                        let variables = Variables::from(message);
                                        result.push(SendUpdate::Message(format!("{variables}")));
                                    } else {
                                        result.push(SendUpdate::Message(
                                            "error: no reply message".to_string(),
                                        ));
                                    }
                                }
                                Command::Help => result.push(SendUpdate::Message(
                                    "/set_filter <expr>
changes current filter. expr should evaluate to bool value.

/set_enabled <expr>
enables or disables the filter. expr should evaluate to bool value.

/set_debug_print <expr>
enables or disables debug print. expr should evaluate to bool value.

/set_report_invalid_commands <expr>
enables or disables reports about invalid commands. expr should evaluate to bool value.

/get_variables
retrieve variables from reply message.

/help
display this message."
                                        .to_string(),
                                )),
                                Command::SetEnabled(arg) => match self.parser.parse(&arg) {
                                    Ok(expression) => {
                                        match evaluate(&expression, &self.variables) {
                                            Ok(value) => match value {
                                                Value::Bool(value) => {
                                                    self.chat.filter_enabled = value;
                                                }
                                                _ => result.push(SendUpdate::Message(
                                                    "error: expression evaluated to non-bool value"
                                                        .to_string(),
                                                )),
                                            },
                                            Err(e) => {
                                                result.push(SendUpdate::Message(format!(
                                                    "error: failed to evaluate expression: {e}"
                                                )));
                                            }
                                        }
                                    }
                                    Err(e) => result
                                        .push(SendUpdate::Message(format!("parse error: {e}"))),
                                },
                            }
                        }
                    }
                }
                Err(e) => result.push(SendUpdate::Message(format!("error: {e}"))),
            },
            None => {}
        }

        if !is_valid_command && self.chat.filter_enabled {
            let variables = Variables::from(&message);
            if let Some(filter) = &self.chat.filter {
                match evaluate(filter, &variables) {
                    Ok(value) => match value {
                        Value::Bool(value) => {
                            if value {
                                result.push(SendUpdate::DeleteMessage(message.id));
                                if self.chat.settings.report_filtered {
                                    result.push(SendUpdate::Message("message filtered".to_string()))
                                }
                            }
                        }
                        _ => {
                            if self.chat.settings.debug_print {
                                result.push(SendUpdate::Message(
                                    "error: filter evaluated to non-bool value".to_string(),
                                ))
                            }
                        }
                    },
                    Err(e) => {
                        if self.chat.settings.debug_print {
                            result.push(SendUpdate::Message(format!(
                                "error: failed to evaluate filter: {e}"
                            )))
                        }
                    }
                }
            }
        }

        let db_lock = self.db.lock().await;
        db_lock.insert_chat(&self.chat).await?;
        drop(db_lock);

        Ok(result)
    }
}

#[derive(Clone, Debug)]
enum CommandError {
    InvalidCommand(String),
    InvalidArguments {
        command: String,
        argument_is_expected: bool,
    },
}

impl CommandError {
    fn new_invalid_command(command: String) -> CommandError {
        CommandError::InvalidCommand(command)
    }

    fn new_invalid_arguments(command: String, argument_is_expected: bool) -> CommandError {
        CommandError::InvalidArguments {
            command,
            argument_is_expected,
        }
    }
}

impl Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::InvalidCommand(command) => write!(f, "invalid command \"{command}\""),
            CommandError::InvalidArguments {
                command,
                argument_is_expected,
            } => {
                if *argument_is_expected {
                    write!(f, "command \"{command}\" expected an argument")
                } else {
                    write!(f, "command \"{command}\" was not expecting an argument")
                }
            }
        }
    }
}

type CommandResult = Result<Option<Command>, CommandError>;

enum Command {
    SetFilter(String),
    SetEnabled(String),
    SetDebugPrint(String),
    SetReportInvalidCommands(String),
    SetReportFiltered(String),
    GetVariables,
    Help,
}

fn split_first_word(text: &str) -> (&str, Option<&str>) {
    if let Some(pos) = text.find(char::is_whitespace) {
        let first_word = &text[..pos];
        let rest = &text[pos + 1..].trim_start();
        (first_word, if rest.is_empty() { None } else { Some(rest) })
    } else if !text.is_empty() {
        (text, None)
    } else {
        panic!("cannot split empty text")
    }
}

impl Command {
    fn new(text: &str) -> CommandResult {
        if let Some(ch) = text.chars().nth(0) {
            if ch == '/' {
                let (first, rest) = split_first_word(text);

                match first {
                    "/set_filter" => {
                        if let Some(arg) = rest {
                            Ok(Some(Command::SetFilter(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(first.to_string(), true))
                        }
                    }
                    "/set_enabled" => {
                        if let Some(arg) = rest {
                            Ok(Some(Command::SetEnabled(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(first.to_string(), true))
                        }
                    }
                    "/set_debug_print" => {
                        if let Some(arg) = rest {
                            Ok(Some(Command::SetDebugPrint(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(first.to_string(), true))
                        }
                    }
                    "/set_report_invalid_commands" => {
                        if let Some(arg) = rest {
                            Ok(Some(Command::SetReportInvalidCommands(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(first.to_string(), true))
                        }
                    }
                    "/set_report_filtered" => {
                        if let Some(arg) = rest {
                            Ok(Some(Command::SetReportFiltered(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(first.to_string(), true))
                        }
                    }
                    "/get_variables" => {
                        if let None = rest {
                            Ok(Some(Command::GetVariables))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                first.to_string(),
                                false,
                            ))
                        }
                    }
                    "/help" => {
                        if let None = rest {
                            Ok(Some(Command::Help))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                first.to_string(),
                                false,
                            ))
                        }
                    }
                    _ => Err(CommandError::new_invalid_command(first.to_string())),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn requires_admin_rights(&self) -> bool {
        match self {
            Command::SetFilter(_) => true,
            Command::SetEnabled(_) => true,
            Command::SetDebugPrint(_) => true,
            Command::SetReportInvalidCommands(_) => true,
            Command::GetVariables => false,
            Command::Help => false,
            Command::SetReportFiltered(_) => true,
        }
    }
}
