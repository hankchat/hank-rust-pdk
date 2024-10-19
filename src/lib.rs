use extism_pdk::{host_fn, Prost};
use hank_types::cron::{CronJob, OneShotJob};
use hank_types::database::{PreparedStatement, Results};
use hank_types::load_plugin_input::Wasm;
use hank_types::message::{Message, Reaction};
use hank_types::plugin::{CommandContext, Instruction, Metadata};
use hank_types::scheduled_job_input::ScheduledJob;
use hank_types::{
    ChatCommandInput, ChatCommandOutput, ChatMessageInput, ChatMessageOutput, CronInput,
    CronOutput, DbQueryInput, DbQueryOutput, GetMetadataInput, GetMetadataOutput, InitializeInput,
    InitializeOutput, InstallInput, InstallOutput, InstructPluginInput, InstructPluginOutput,
    LoadPluginInput, LoadPluginOutput, OneShotInput, OneShotOutput, ReactInput, ReactOutput,
    ReloadPluginInput, ReloadPluginOutput, ScheduledJobInput, ScheduledJobOutput, SendMessageInput,
    SendMessageOutput, UnloadPluginInput, UnloadPluginOutput,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

pub use extism_pdk::{
    debug, error, http, info, plugin_fn, warn, FnResult, HttpRequest, HttpResponse,
};
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
    pub fn unload_plugin(input: Prost<UnloadPluginInput>) -> Prost<UnloadPluginOutput>;
    pub fn instruct_plugin(input: Prost<InstructPluginInput>) -> Prost<InstructPluginOutput>;
}

#[derive(Default, Debug)]
pub struct Hank {
    metadata: Metadata,
    install_handler: Option<fn()>,
    initialize_handler: Option<fn()>,
    chat_message_handler: Option<fn(message: Message)>,
    chat_command_handler: Option<fn(context: CommandContext, message: Message)>,
    scheduled_jobs: HashMap<String, fn()>,
}

// @TODO error handling
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

    pub fn chat_message_handler(&self) -> Option<fn(message: Message)> {
        self.chat_message_handler
    }

    pub fn register_chat_message_handler(&mut self, handler: fn(message: Message)) {
        self.chat_message_handler = Some(handler);
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

    pub fn scheduled_job_handler(&self, uuid: String) {
        if let Some(job) = self.scheduled_jobs.get(&uuid) {
            job();
        }
    }

    pub(crate) fn add_cron(&mut self, cron: String, job: fn()) {
        let uuid = uuid::Uuid::new_v4();

        self.scheduled_jobs.insert(uuid.to_string(), job);

        let input = CronInput {
            cron_job: Some(CronJob {
                cron,
                job: uuid.to_string(),
            }),
        };

        let _ = unsafe { crate::cron(Prost(input)) };
    }

    pub(crate) fn add_one_shot(&mut self, duration: i32, job: fn()) {
        let uuid = uuid::Uuid::new_v4();

        self.scheduled_jobs.insert(uuid.to_string(), job);

        let input = OneShotInput {
            one_shot_job: Some(OneShotJob {
                duration,
                job: uuid.to_string(),
            }),
        };

        let _ = unsafe { one_shot(Prost(input)) };
    }

    pub fn start(self) -> FnResult<()> {
        let mut hank = HANK.write().unwrap();
        *hank = Some(self);
        Ok(())
    }

    pub fn send_message(message: Message) {
        let input = SendMessageInput {
            message: Some(message),
        };

        let _ = unsafe { send_message(Prost(input)) };
    }

    pub fn respond(response: String, message: Message) {
        let response = Message {
            content: response,
            ..message
        };
        Self::send_message(response);
    }

    pub fn react(reaction: Reaction) {
        let input = ReactInput {
            reaction: Some(reaction),
        };

        let _ = unsafe { react(Prost(input)) };
    }

    pub fn db_query(statement: PreparedStatement) -> Result<Results, String> {
        let input = DbQueryInput {
            prepared_statement: Some(statement),
        };

        let output = unsafe { db_query(Prost(input)) };
        let Prost(DbQueryOutput { results, error }) = output.unwrap();

        if let Some(error) = error {
            Err(error)
        } else {
            Ok(results.unwrap_or_default())
        }
    }

    pub fn db_fetch<T: for<'a> Deserialize<'a>>(
        statement: PreparedStatement,
    ) -> Result<Vec<T>, String> {
        let input = DbQueryInput {
            prepared_statement: Some(statement),
        };

        let output = unsafe { db_query(Prost(input)) };
        let Prost(DbQueryOutput { results, error }) = output.unwrap();

        if let Some(error) = error {
            Err(error)
        } else {
            Ok(results
                .unwrap_or_default()
                .rows
                .into_iter()
                .map(|s| serde_json::from_str(&s).unwrap())
                .collect())
        }
    }

    pub fn cron(cron: String, job: fn()) {
        let mut guard = HANK.write().unwrap();
        let hank = guard
            .as_mut()
            .expect("Plugin did not initialize global HANK");
        hank.add_cron(cron, job);
    }

    pub fn one_shot(duration: i32, job: fn()) {
        let mut guard = HANK.write().unwrap();
        let hank = guard
            .as_mut()
            .expect("Plugin did not initialize global HANK");
        hank.add_one_shot(duration, job);
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
            |Prost(LoadPluginOutput {
                 metadata, manifest, ..
             })| {
                (
                    serde_json::from_str(&manifest).expect("valid manifest"),
                    metadata.expect("ok result"),
                )
            },
        )
    }

    // Escalated privileges necessary for use.
    pub fn unload_plugin(plugin: impl Into<String>, cleanup: bool) {
        let input = UnloadPluginInput {
            plugin: plugin.into(),
            cleanup,
        };

        let _ = unsafe { unload_plugin(Prost(input)) };
    }

    // Escalated privileges necessary for use.
    pub fn instruct_plugin(instruction: Instruction) {
        let input = InstructPluginInput {
            instruction: Some(instruction),
        };

        let _ = unsafe { instruct_plugin(Prost(input)) };
    }
}

static HANK: LazyLock<RwLock<Option<Hank>>> = LazyLock::new(|| RwLock::new(None));

#[plugin_fn]
pub fn handle_chat_command(
    Prost(ChatCommandInput { context, message }): Prost<ChatCommandInput>,
) -> FnResult<Prost<ChatCommandOutput>> {
    let guard = HANK.read().unwrap();
    let hank = guard
        .as_ref()
        .expect("Plugin did not initialize global HANK");

    hank.chat_command_handler().map(|handler| {
        handler(
            context.expect("context should exist"),
            message.expect("message should exist"),
        )
    });

    Ok(Prost(ChatCommandOutput::default()))
}

#[plugin_fn]
pub fn handle_chat_message(
    Prost(ChatMessageInput { message }): Prost<ChatMessageInput>,
) -> FnResult<Prost<ChatMessageOutput>> {
    let guard = HANK.read().unwrap();
    let hank = guard
        .as_ref()
        .expect("Plugin did not initialize global HANK");

    hank.chat_message_handler()
        .map(|handler| handler(message.expect("message should exist")));

    Ok(Prost(ChatMessageOutput::default()))
}

#[plugin_fn]
pub fn handle_get_metadata(
    Prost(_input): Prost<GetMetadataInput>,
) -> FnResult<Prost<GetMetadataOutput>> {
    let guard = HANK.read().unwrap();
    let hank = guard
        .as_ref()
        .expect("Plugin did not initialize global HANK");

    Ok(Prost(GetMetadataOutput {
        metadata: Some(hank.metadata()),
    }))
}

#[plugin_fn]
pub fn handle_install(Prost(_input): Prost<InstallInput>) -> FnResult<Prost<InstallOutput>> {
    let guard = HANK.read().unwrap();
    let hank = guard
        .as_ref()
        .expect("Plugin did not initialize global HANK");

    if let Some(handler) = hank.install_handler() {
        handler();
    }

    Ok(Prost(InstallOutput::default()))
}

#[plugin_fn]
pub fn handle_initialize(
    Prost(_input): Prost<InitializeInput>,
) -> FnResult<Prost<InitializeOutput>> {
    // This needs to be in its own scope to ensure the guard is dropped before we actually run the
    // initialize handler. Otherwise the initialize handler can't mutate the global hank with the
    // Hank::cron and Hank::one_shot functions.
    {
        let guard = HANK.read().unwrap();
        let hank = guard
            .as_ref()
            .expect("Plugin did not initialize global HANK");
        hank.initialize_handler()
    }
    .map(|handler| handler());

    Ok(Prost(InitializeOutput::default()))
}

#[plugin_fn]
pub fn handle_scheduled_job(
    Prost(input): Prost<ScheduledJobInput>,
) -> FnResult<Prost<ScheduledJobOutput>> {
    if let Some(scheduled_job) = input.scheduled_job {
        let job = match scheduled_job {
            ScheduledJob::CronJob(cron) => cron.job,
            ScheduledJob::OneShotJob(oneshot) => oneshot.job,
        };

        let guard = HANK.write().unwrap();
        let hank = guard
            .as_ref()
            .expect("Plugin did not initialize global HANK");
        hank.scheduled_job_handler(job);
    }
    Ok(Prost(ScheduledJobOutput::default()))
}
