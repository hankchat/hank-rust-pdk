use extism_pdk::{host_fn, Prost};
use hank_types::access_check::{AccessCheck, AccessCheckChain, AccessCheckOperator};
use hank_types::cron::{CronJob, OneShotJob};
use hank_types::database::PreparedStatement;
use hank_types::load_plugin_input::Wasm;
use hank_types::message::{Message, Reaction};
use hank_types::plugin::{Argument, Command, CommandContext, EscalatedPrivilege, Metadata};
use hank_types::{
    CronInput, CronOutput, DbQueryInput, DbQueryOutput, HandleChatCommandInput, LoadPluginInput,
    LoadPluginOutput, OneShotInput, OneShotOutput, ReactInput, ReactOutput, ReloadPluginInput,
    ReloadPluginOutput, SendMessageInput, SendMessageOutput,
};
use serde::Deserialize;
use std::sync::OnceLock;

pub use extism_pdk::{plugin_fn, FnResult};
pub use prost::Message as ProstMessage;

#[host_fn]
extern "ExtismHost" {
    pub fn send_message(input: Prost<SendMessageInput>) -> Prost<SendMessageOutput>;
    pub fn react(input: Prost<ReactInput>) -> Prost<ReactOutput>;
    pub fn db_query(input: Prost<DbQueryInput>) -> Prost<DbQueryOutput>;
    pub fn cron(input: Prost<CronInput>) -> Prost<CronOutput>;
    pub fn one_shot(input: Prost<OneShotInput>) -> Prost<OneShotOutput>;
    pub fn reload_plugin(input: Prost<ReloadPluginInput>) -> Prost<ReloadPluginOutput>;
    pub fn load_plugin(input: Prost<LoadPluginInput>) -> Prost<LoadPluginOutput>;
}

#[derive(Default, Debug)]
pub struct Hank {
    metadata: Metadata,
    install_handler: Option<fn()>,
    initialize_handler: Option<fn()>,
    message_handler: Option<fn(message: Message)>,
    chat_command_handler: Option<fn(context: CommandContext, message: Message)>,
}

impl Hank {
    pub fn new(metadata: impl Into<Metadata>) -> Self {
        Self {
            metadata: metadata.into(),
            ..Default::default()
        }
    }

    pub fn metadata(&self) -> Metadata {
        self.metadata.clone()
    }

    pub fn install_handler(&self) -> Option<fn()> {
        self.install_handler
    }

    pub fn register_install_handler(&mut self, handler: fn()) {
        self.install_handler = Some(handler);
    }

    pub fn initialize_handler(&self) -> Option<fn()> {
        self.initialize_handler
    }

    pub fn register_initialize_handler(&mut self, handler: fn()) {
        self.initialize_handler = Some(handler);
    }

    pub fn message_handler(&self) -> Option<fn(message: Message)> {
        self.message_handler
    }

    pub fn register_message_handler(&mut self, handler: fn(message: Message)) {
        self.message_handler = Some(handler);
    }

    pub fn chat_command_handler(&self) -> Option<fn(context: CommandContext, message: Message)> {
        self.chat_command_handler
    }

    pub fn register_chat_command_handler(
        &mut self,
        handler: fn(context: CommandContext, message: Message),
    ) {
        self.chat_command_handler = Some(handler);
    }

    pub fn start(self) -> FnResult<()> {
        HANK.set(self)
            .expect("Plugin failed to initialize global HANK");
        Ok(())
    }

    pub fn send_message(message: Message) {
        let input = SendMessageInput {
            message: Some(message),
        };

        let _ = unsafe { send_message(Prost(input)) };
    }

    pub fn react(reaction: Reaction) {
        let input = ReactInput {
            reaction: Some(reaction),
        };

        let _ = unsafe { react(Prost(input)) };
    }

    pub fn db_query(statement: PreparedStatement) {
        let input = DbQueryInput {
            prepared_statement: Some(statement),
        };

        let _ = unsafe { db_query(Prost(input)) };
    }

    pub fn db_fetch<T: for<'a> Deserialize<'a>>(statement: PreparedStatement) -> Vec<T> {
        let input = DbQueryInput {
            prepared_statement: Some(statement),
        };

        let output = unsafe { db_query(Prost(input)) };
        let Prost(DbQueryOutput { results }) = output.unwrap();

        results
            .unwrap()
            .rows
            .into_iter()
            .map(|s| serde_json::from_str(&s).unwrap())
            .collect()
    }

    pub fn cron(cronjob: CronJob) {
        let input = CronInput {
            cron_job: Some(cronjob),
        };

        let _ = unsafe { cron(Prost(input)) };
    }

    pub fn one_shot(oneshot: OneShotJob) {
        let input = OneShotInput {
            one_shot_job: Some(oneshot),
        };

        let _ = unsafe { one_shot(Prost(input)) };
    }

    // Escalated privileges necessary for use.
    pub fn reload_plugin(plugin: impl Into<String>) {
        let input = ReloadPluginInput {
            plugin: plugin.into(),
        };

        let _ = unsafe { reload_plugin(Prost(input)) };
    }

    // Escalated privileges necessary for use.
    pub fn load_plugin(
        wasm: impl Into<Wasm>,
    ) -> Result<(extism_manifest::Manifest, Metadata), extism_pdk::Error> {
        let input = LoadPluginInput {
            wasm: Some(wasm.into()),
        };

        unsafe { load_plugin(Prost(input)) }.map(
            |Prost(LoadPluginOutput { metadata, manifest })| {
                (
                    serde_json::from_str(&manifest).expect("valid manifest"),
                    metadata.expect("ok result"),
                )
            },
        )
    }
}

static HANK: OnceLock<Hank> = OnceLock::new();

#[plugin_fn]
pub fn handle_chat_command(
    Prost(HandleChatCommandInput { context, message }): Prost<HandleChatCommandInput>,
) -> FnResult<()> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");

    hank.chat_command_handler().map(|handler| {
        handler(
            context.expect("context should exist"),
            message.expect("message should exist"),
        )
    });

    Ok(())
}

#[plugin_fn]
pub fn handle_message(Prost(message): Prost<Message>) -> FnResult<()> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");

    hank.message_handler().map(|handler| handler(message));

    Ok(())
}

#[plugin_fn]
pub fn get_metadata() -> FnResult<Prost<Metadata>> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");
    Ok(Prost(hank.metadata()))
}

#[plugin_fn]
pub fn install() -> FnResult<()> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");
    if let Some(handler) = hank.install_handler() {
        handler();
    }

    Ok(())
}

#[plugin_fn]
pub fn initialize() -> FnResult<()> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");
    if let Some(handler) = hank.initialize_handler() {
        handler();
    }

    Ok(())
}

/// Wrapper for Access Check shorthands.
#[derive(Default, Debug)]
pub enum AccessChecks {
    #[default]
    None,
    Array(Vec<AccessCheck>),
    Single(AccessCheck),
    Full(AccessCheckChain),
}

/// Wrapper for Metadata protobuf that's more user friendly.
#[derive(Default, Debug)]
pub struct PluginMetadata<'a> {
    /// The plguins name.
    pub name: &'a str,
    /// A short description of the plugin.
    pub description: &'a str,
    /// A version string for the plugin. Should follow semver.
    ///
    /// @see: <https://semver.org/>
    pub version: &'a str,
    /// When true, a SQLite3 database will be created for the plugin.
    /// @deprecated All plugins get a database by default now.
    pub database: bool,
    /// Access checks
    ///
    /// All functionality of this plugin can optionally be gated by accses checks.
    pub access_checks: AccessChecks,
    /// A secret escalation key that grants this plugin specific escalated
    /// privileges.
    pub escalation_key: Option<&'a str>,
    /// A list of escalated privileges that this plugin requests to use.
    pub escalated_privileges: Vec<EscalatedPrivilege>,
    /// The author of the plugin.
    pub author: &'a str,
    /// Whether or not this plugin handles commands.
    pub handles_commands: bool,
    /// Whether or not this plugin handles messages.
    pub handles_messages: bool,
    /// Optionally override the plugin command name.
    pub command_name: Option<&'a str>,
    /// Optional aliases for the plugin command.
    pub aliases: Vec<&'a str>,
    /// Arguments for the plugin command.
    pub arguments: Vec<Argument>,
    /// Plugin subcommands.
    pub subcommands: Vec<Command>,
    /// Hosts that this plugin requests permissions to access via HTTP.
    pub allowed_hosts: Vec<String>,
    /// Pool size this plugin requests.
    pub pool_size: Option<i32>,
}

impl From<PluginMetadata<'_>> for Metadata {
    fn from(value: PluginMetadata) -> Self {
        use AccessChecks::*;

        Self {
            name: value.name.into(),
            description: value.description.into(),
            version: value.version.into(),
            database: false, // @deprecated
            access_checks: match value.access_checks {
                None => Option::None,
                Array(checks) => Some(AccessCheckChain {
                    operator: AccessCheckOperator::Or.into(),
                    checks,
                }),
                Single(check) => Some(AccessCheckChain {
                    operator: AccessCheckOperator::Or.into(),
                    checks: vec![check],
                }),
                Full(full) => Some(full),
            },
            escalation_key: value.escalation_key.map(String::from),
            escalated_privileges: value
                .escalated_privileges
                .into_iter()
                .map(i32::from)
                .collect(),
            author: value.author.into(),
            handles_commands: value.handles_commands,
            handles_messages: value.handles_messages,
            command_name: value.command_name.map(String::from),
            aliases: value.aliases.into_iter().map(String::from).collect(),
            arguments: value.arguments,
            subcommands: value.subcommands,
            allowed_hosts: value.allowed_hosts,
            pool_size: value.pool_size,
        }
    }
}
