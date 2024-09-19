use extism_pdk::{host_fn, Prost};
use hank_types::cron::{CronJob, OneShotJob};
use hank_types::database::{PreparedStatement, Results};
use hank_types::message::{Message, Reaction};
use hank_types::{
    CronInput, CronOutput, DbQueryInput, DbQueryOutput, OneShotInput, OneShotOutput, ReactInput,
    ReactOutput, SendMessageInput, SendMessageOutput,
};

#[host_fn]
extern "ExtismHost" {
    pub fn send_message(input: Prost<SendMessageInput>) -> Prost<SendMessageOutput>;
    pub fn react(input: Prost<ReactInput>) -> Prost<ReactOutput>;
    pub fn db_query(input: Prost<DbQueryInput>) -> Prost<DbQueryOutput>;
    pub fn cron(input: Prost<CronInput>) -> Prost<CronOutput>;
    pub fn one_shot(input: Prost<OneShotInput>) -> Prost<OneShotOutput>;
}

pub struct Hank;

impl Hank {
    pub fn new() -> Self {
        Self
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
}
