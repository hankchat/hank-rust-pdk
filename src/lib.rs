use extism_pdk::{host_fn, plugin_fn, FnResult, Prost};
use hank_types::access_check::{AccessCheck, AccessCheckChain, AccessCheckOperator};
use hank_types::cron::{CronJob, OneShotJob};
use hank_types::database::{PreparedStatement, Results};
use hank_types::message::{Message, Reaction};
use hank_types::plugin::{EscalatedPrivilege, Metadata};
use hank_types::{
    CronInput, CronOutput, DbQueryInput, DbQueryOutput, OneShotInput, OneShotOutput, ReactInput,
    ReactOutput, ReloadPluginInput, ReloadPluginOutput, SendMessageInput, SendMessageOutput,
};
use std::sync::OnceLock;

#[host_fn]
extern "ExtismHost" {
    pub fn send_message(input: Prost<SendMessageInput>) -> Prost<SendMessageOutput>;
    pub fn react(input: Prost<ReactInput>) -> Prost<ReactOutput>;
    pub fn db_query(input: Prost<DbQueryInput>) -> Prost<DbQueryOutput>;
    pub fn cron(input: Prost<CronInput>) -> Prost<CronOutput>;
    pub fn one_shot(input: Prost<OneShotInput>) -> Prost<OneShotOutput>;
    pub fn reload_plugin(input: Prost<ReloadPluginInput>) -> Prost<ReloadPluginOutput>;
}

#[derive(Default, Debug)]
pub struct Hank {
    metadata: Metadata,
    install_handler: Option<fn()>,
    initialize_handler: Option<fn()>,
    message_handler: Option<fn(message: Message)>,
    command_handler: Option<fn(command: Message)>,
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

    pub fn command_handler(&self) -> Option<fn(command: Message)> {
        self.command_handler
    }

    pub fn register_command_handler(&mut self, handler: fn(command: Message)) {
        self.command_handler = Some(handler);
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

    // @TODO make this generic
    pub fn db_query(statement: PreparedStatement) -> Results {
        let input = DbQueryInput {
            prepared_statement: Some(statement),
        };

        let output = unsafe { db_query(Prost(input)) };
        let Prost(output) = output.unwrap();

        output.results.unwrap()
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
}

static HANK: OnceLock<Hank> = OnceLock::new();

#[plugin_fn]
pub fn handle_command(Prost(message): Prost<Message>) -> FnResult<()> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");
    if let Some(handler) = hank.command_handler() {
        handler(message);
    }

    Ok(())
}
#[plugin_fn]
pub fn handle_message(Prost(message): Prost<Message>) -> FnResult<()> {
    let hank = HANK.get().expect("Plugin did not initialize global HANK");
    if let Some(handler) = hank.message_handler() {
        handler(message);
    }

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
    pub escalation_key: &'a str,
    /// A list of escalated privileges that this plugin requests to use.
    pub escalated_privileges: Vec<EscalatedPrivilege>,
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
            escalation_key: value.escalation_key.into(),
            escalated_privileges: value
                .escalated_privileges
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}
