use super::database::{Chat, Db, Filter};
use baldguard_language::{
    evaluation::{evaluate, ContainsVariable, SetFromAssignment, Value, Variables},
    grammar::{AssignmentParser, ExpressionParser, IdentifierParser},
};
use baldguard_macros::{ContainsVariable, ToVariables};
use std::{
    error::Error,
    fmt::Display,
    sync::Arc,
    time::{Duration, Instant},
};
use teloxide::types::{ChatId, Message, MessageId, MessageOrigin};
use tokio::sync::Mutex;

const HELP_STRING: &str = "/set_filter <expr>
change current filter. expr should evaluate to bool value.
requires admin rights.

/get_filter
display current filter.

/set_option <option> := <expr>
set an option.
available options:
- debug_print: bool
- report_filtered: bool
- report_invalid_commands: bool
- filter_enabled: bool
- report_command_success: bool
expr should evaluate to value of option's type.
requires admin rights.

/get_options
display current options.

/set_variable <variable> := <expr>
set a user variable.
requires admin rights.

/unset_variable <variable>
unset a user variable.
requires admin rights.

/get_variables
display user variables.

/get_message_variables
display variables from message.

/help
display this message.";

pub enum SendUpdate {
    Message(String),
    DeleteMessage(MessageId),
}

pub struct Session {
    chat_id: ChatId,
    bot_username: String,
    db: Arc<Mutex<Db>>,
    expression_parser: ExpressionParser,
    assignment_parser: AssignmentParser,
    identifier_parser: IdentifierParser,
    chat: Chat,
    last_active: Instant,
}

#[derive(Debug, Clone, ToVariables, ContainsVariable)]
struct MessageVariables {
    has_from: bool,
    from_id: Option<i64>,
    from_is_bot: Option<bool>,
    from_username: Option<String>,
    from_is_premium: Option<bool>,
    has_origin: bool,
    origin_type: Option<String>,
    origin_user_id: Option<i64>,
    origin_user_is_bot: Option<bool>,
    origin_user_username: Option<String>,
    origin_hidden_user_username: Option<String>,
    origin_chat_id: Option<i64>,
    origin_chat_author_signature: Option<String>,
    origin_channel_id: Option<i64>,
    origin_channel_message_id: Option<i64>,
    origin_channel_author_signature: Option<String>,
    has_text: bool,
    text: Option<String>,
    has_audio: bool,
    has_document: bool,
    has_animation: bool,
    has_game: bool,
    has_photo: bool,
    has_sticker: bool,
    has_story: bool,
    has_video: bool,
    has_voice: bool,
    has_caption: bool,
    caption: Option<String>,
}

impl Default for MessageVariables {
    fn default() -> Self {
        MessageVariables {
            has_from: false,
            from_id: None,
            from_is_bot: None,
            from_username: None,
            from_is_premium: None,
            has_origin: false,
            origin_type: None,
            origin_user_id: None,
            origin_user_is_bot: None,
            origin_user_username: None,
            origin_hidden_user_username: None,
            origin_chat_id: None,
            origin_chat_author_signature: None,
            origin_channel_id: None,
            origin_channel_message_id: None,
            origin_channel_author_signature: None,
            has_text: false,
            text: None,
            has_audio: false,
            has_document: false,
            has_animation: false,
            has_game: false,
            has_photo: false,
            has_sticker: false,
            has_story: false,
            has_video: false,
            has_voice: false,
            has_caption: false,
            caption: None,
        }
    }
}

impl From<&Message> for MessageVariables {
    fn from(value: &Message) -> Self {
        let mut result = MessageVariables::default();

        if let Some(from) = &value.from {
            result.has_from = true;
            result.from_id = Some(from.id.0 as i64);
            result.from_is_bot = Some(from.is_bot);
            if let Some(username) = &from.username {
                result.from_username = Some(username.to_string());
            }
            result.from_is_premium = Some(from.is_premium);
        }

        if let Some(origin) = &value.forward_origin() {
            result.has_origin = true;

            match origin {
                MessageOrigin::User {
                    date: _,
                    sender_user,
                } => {
                    result.origin_type = Some("user".to_string());
                    result.origin_user_id = Some(sender_user.id.0 as i64);
                    result.origin_user_is_bot = Some(sender_user.is_bot);
                    if let Some(username) = &sender_user.username {
                        result.origin_user_username = Some(username.to_string());
                    }
                }
                MessageOrigin::HiddenUser {
                    date: _,
                    sender_user_name,
                } => {
                    result.origin_type = Some("hidden_user".to_string());
                    result.origin_hidden_user_username = Some(sender_user_name.to_string());
                }
                MessageOrigin::Chat {
                    date: _,
                    sender_chat,
                    author_signature,
                } => {
                    result.origin_type = Some("chat".to_string());
                    result.origin_chat_id = Some(sender_chat.id.0 as i64);
                    if let Some(signature) = author_signature {
                        result.origin_chat_author_signature = Some(signature.to_string());
                    }
                }
                MessageOrigin::Channel {
                    date: _,
                    chat,
                    message_id,
                    author_signature,
                } => {
                    result.origin_type = Some("channel".to_string());
                    result.origin_channel_id = Some(chat.id.0 as i64);
                    result.origin_channel_message_id = Some(message_id.0 as i64);
                    if let Some(signature) = author_signature {
                        result.origin_channel_author_signature = Some(signature.to_string());
                    }
                }
            }
        }

        if let Some(text) = value.text() {
            result.has_text = true;
            result.text = Some(text.to_string());
        }

        if value.audio().is_some() {
            result.has_audio = true;
        }
        if value.document().is_some() {
            result.has_document = true;
        }
        if value.animation().is_some() {
            result.has_animation = true;
        }
        if value.game().is_some() {
            result.has_game = true;
        }
        if value.photo().is_some() {
            result.has_photo = true;
        }
        if value.sticker().is_some() {
            result.has_sticker = true;
        }
        if value.story().is_some() {
            result.has_story = true;
        }
        if value.video().is_some() {
            result.has_video = true;
        }
        if value.voice().is_some() {
            result.has_voice = true;
        }

        if let Some(caption) = value.caption() {
            result.has_caption = true;
            result.caption = Some(caption.to_string());
        }

        result
    }
}

impl Session {
    pub async fn new(
        db: Arc<Mutex<Db>>,
        chat_id: ChatId,
        bot_username: String,
    ) -> Result<Self, Box<dyn Error>> {
        let db_lock = db.lock().await;
        let chat = db_lock.find_chat_by_id(chat_id.0).await?;
        drop(db_lock);
        Ok(Session {
            chat_id,
            bot_username,
            db,
            expression_parser: ExpressionParser::new(),
            assignment_parser: AssignmentParser::new(),
            identifier_parser: IdentifierParser::new(),
            chat,
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
        let mut command_failed = false;
        let mut command_requires_success_report = false;
        match message.text() {
            Some(text) => match Command::new(text, &self.bot_username) {
                Ok(command) => {
                    if let Some(command) = command {
                        if command.requires_admin_rights() && !from_admin {
                            result.push(SendUpdate::Message(format!("error: permission denied")))
                        } else {
                            is_valid_command = true;
                            match command {
                                Command::SetFilter(arg) => {
                                    command_requires_success_report = true;

                                    match self.expression_parser.parse(&arg) {
                                        Ok(expression) => {
                                            self.chat.filter =
                                                Some(Filter::new(arg.clone(), *expression))
                                        }
                                        Err(e) => {
                                            command_failed = true;
                                            result.push(SendUpdate::Message(format!(
                                                "parse error: {e}"
                                            )))
                                        }
                                    }
                                }
                                Command::GetFilter => match &self.chat.filter {
                                    Some(filter) => {
                                        result.push(SendUpdate::Message(filter.text.clone()));
                                    }
                                    None => {
                                        command_failed = true;
                                        result
                                            .push(SendUpdate::Message("no filter set".to_string()));
                                    }
                                },
                                Command::SetOption(arg) => {
                                    command_requires_success_report = true;

                                    match self.assignment_parser.parse(&arg) {
                                        Ok(assignment) => {
                                            if let Err(e) = self.chat.settings.set_from_assignment(
                                                &assignment,
                                                &self.chat.variables,
                                            ) {
                                                command_failed = true;
                                                result.push(SendUpdate::Message(format!(
                                                    "failed to set option: {e}"
                                                )));
                                            }
                                        }
                                        Err(e) => {
                                            command_failed = true;
                                            result.push(SendUpdate::Message(format!(
                                                "parse error: {e}"
                                            )))
                                        }
                                    }
                                }
                                Command::GetOptions => {
                                    let variables = Variables::from(self.chat.settings.clone());
                                    result.push(SendUpdate::Message(variables.show(false)));
                                }
                                Command::SetVariable(arg) => {
                                    command_requires_success_report = true;

                                    match self.assignment_parser.parse(&arg) {
                                        Ok(assignment) => {
                                            if MessageVariables::default()
                                                .contains_variable(&assignment.identifier)
                                            {
                                                result.push(SendUpdate::Message(format!(
                                                    "failed to set variable: \"{}\" is reserved",
                                                    assignment.identifier
                                                )));

                                                command_failed = true;
                                            } else {
                                                if let Err(e) =
                                                    self.chat.variables.set_from_assignment(
                                                        &assignment,
                                                        &self.chat.variables.clone(),
                                                    )
                                                {
                                                    command_failed = true;
                                                    result.push(SendUpdate::Message(format!(
                                                        "failed to set variable: {e}"
                                                    )));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            command_failed = true;
                                            result.push(SendUpdate::Message(format!(
                                                "parse error: {e}"
                                            )))
                                        }
                                    }
                                }
                                Command::UnsetVariable(arg) => {
                                    command_requires_success_report = true;

                                    match self.identifier_parser.parse(&arg) {
                                        Ok(identifier) => {
                                            if !self.chat.variables.remove(&identifier) {
                                                result.push(SendUpdate::Message(format!(
                                                    "variable \"{identifier}\" does not exist"
                                                )));

                                                command_failed = true;
                                            }
                                        }
                                        Err(e) => {
                                            command_failed = true;
                                            result.push(SendUpdate::Message(format!(
                                                "parse error: {e}"
                                            )))
                                        }
                                    }
                                }
                                Command::GetVariables => {
                                    if self.chat.variables.count() > 0 {
                                        result.push(SendUpdate::Message(
                                            self.chat.variables.show(false),
                                        ));
                                    } else {
                                        command_failed = true;
                                        result.push(SendUpdate::Message("no variables".to_string()))
                                    }
                                }
                                Command::GetMessageVariables => {
                                    if let Some(message) = message.reply_to_message() {
                                        let variables = MessageVariables::from(message);
                                        let variables = Variables::from(variables);
                                        result.push(SendUpdate::Message(format!("{variables}")));
                                    } else {
                                        command_failed = true;
                                        result.push(SendUpdate::Message(
                                            "error: no reply message".to_string(),
                                        ));
                                    }
                                }
                                Command::Eval(arg) => match self.expression_parser.parse(&arg) {
                                    Ok(expression) => {
                                        match evaluate(&expression, &self.chat.variables) {
                                            Ok(value) => {
                                                result.push(SendUpdate::Message(value.to_string()))
                                            }
                                            Err(e) => {
                                                command_failed = true;
                                                result.push(SendUpdate::Message(format!(
                                                    "error: failed to evalute expression: {e}"
                                                )));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        command_failed = true;
                                        result
                                            .push(SendUpdate::Message(format!("parse error: {e}")))
                                    }
                                },
                                Command::Help => {
                                    result.push(SendUpdate::Message(HELP_STRING.to_string()))
                                }
                            }
                        }
                    }
                }
                Err(e) => result.push(SendUpdate::Message(format!("error: {e}"))),
            },
            None => {}
        }

        if is_valid_command
            && command_requires_success_report
            && !command_failed
            && self.chat.settings.report_command_success
        {
            result.push(SendUpdate::Message("success".to_string()));
        }

        if !is_valid_command && self.chat.settings.filter_enabled {
            let variables = MessageVariables::from(&message);
            let mut variables: Variables = Variables::from(variables);
            variables.extend(self.chat.variables.clone());
            if let Some(filter) = &self.chat.filter {
                match evaluate(&filter.expression, &variables) {
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
    GetFilter,
    SetOption(String),
    GetOptions,
    SetVariable(String),
    UnsetVariable(String),
    GetVariables,
    GetMessageVariables,
    Eval(String),
    Help,
}

fn split_first_word<P>(text: &str, pat: P) -> (&str, Option<&str>)
where
    P: FnMut(char) -> bool,
{
    if let Some(pos) = text.find(pat) {
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
    fn new(text: &str, bot_username: &str) -> CommandResult {
        if let Some(ch) = text.chars().nth(0) {
            if ch == '/' {
                let (command, arg) = split_first_word(text, char::is_whitespace);
                let (command, for_bot_username) = split_first_word(command, |c| c == '@');

                if let Some(for_bot_username) = for_bot_username {
                    if for_bot_username != bot_username {
                        return Ok(None);
                    }
                }

                match command {
                    "/set_filter" => {
                        if let Some(arg) = arg {
                            Ok(Some(Command::SetFilter(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                true,
                            ))
                        }
                    }
                    "/get_filter" => {
                        if let None = arg {
                            Ok(Some(Command::GetFilter))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                false,
                            ))
                        }
                    }
                    "/set_option" => {
                        if let Some(arg) = arg {
                            Ok(Some(Command::SetOption(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                true,
                            ))
                        }
                    }
                    "/get_options" => {
                        if let None = arg {
                            Ok(Some(Command::GetOptions))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                false,
                            ))
                        }
                    }
                    "/set_variable" => {
                        if let Some(arg) = arg {
                            Ok(Some(Command::SetVariable(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                true,
                            ))
                        }
                    }
                    "/unset_variable" => {
                        if let Some(arg) = arg {
                            Ok(Some(Command::UnsetVariable(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                true,
                            ))
                        }
                    }
                    "/get_variables" => {
                        if let None = arg {
                            Ok(Some(Command::GetVariables))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                false,
                            ))
                        }
                    }
                    "/get_message_variables" => {
                        if let None = arg {
                            Ok(Some(Command::GetMessageVariables))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                false,
                            ))
                        }
                    }
                    "/eval" => {
                        if let Some(arg) = arg {
                            Ok(Some(Command::Eval(arg.to_string())))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                true,
                            ))
                        }
                    }
                    "/help" => {
                        if let None = arg {
                            Ok(Some(Command::Help))
                        } else {
                            Err(CommandError::new_invalid_arguments(
                                command.to_string(),
                                false,
                            ))
                        }
                    }
                    _ => Err(CommandError::new_invalid_command(command.to_string())),
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
            Command::SetOption(_) => true,
            Command::GetMessageVariables => false,
            Command::Help => false,
            Command::SetVariable(_) => true,
            Command::UnsetVariable(_) => true,
            Command::GetVariables => false,
            Command::GetOptions => false,
            Command::GetFilter => false,
            Command::Eval(_) => false,
        }
    }
}
