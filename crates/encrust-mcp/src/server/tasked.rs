//! A long call answered as a task where the client takes tasks: the call is handed a task id at
//! once and the client polls for the answer (the MCP Tasks extension, ADR 0467).

use std::time::{Duration, Instant};

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ContentBlock, CreateTaskResult, ErrorCode,
};
use rmcp::service::RequestContext;
use rmcp::task_manager::{TaskExit, TaskManager, TaskOptions};

use super::{Context, Failed, Server};
use crate::trace::{body_of, stamp_took};

/// The tools that answer from what is at hand, never worth a task.
const QUICK: [&str; 3] = ["list_profiles", "list_recipes", "show_in_window"];
/// How long a task's answer is kept past the longest run it may wait on.
const KEPT_PAST_RUN: Duration = Duration::from_mins(10);
/// How often a client is asked to poll a task, ms.
const POLL_MS: u64 = 2_000;

/// The tasks the server has handed out.
#[derive(Clone, Default)]
pub(super) struct Tasks(TaskManager);

impl std::fmt::Debug for Tasks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tasks")
    }
}

impl std::ops::Deref for Tasks {
    type Target = TaskManager;
    fn deref(&self) -> &TaskManager {
        &self.0
    }
}

/// Whether `request` is answered as a task: the client takes tasks and the tool runs the engine.
pub(super) fn as_task(request: &CallToolRequestParams, context: &Context) -> bool {
    let takes = (context.client_capabilities()).is_some_and(|caps| caps.supports_tasks());
    takes && !QUICK.contains(&request.name.as_ref())
}

impl Server {
    /// Hands `request` out as a task, answered as `called` answers it once it ends; a task
    /// cancelled stops its run.
    pub(super) fn tasked(
        &self,
        request: CallToolRequestParams,
        context: &Context,
    ) -> CallToolResponse {
        // The call's own token is cancelled once the task handle is answered: the run gets one
        // of its own, which only the task's cancel ends.
        let mut own = RequestContext::new(context.id.clone(), context.peer.clone());
        own.meta = context.meta.clone();
        own.extensions = context.extensions.clone();
        let stop = own.ct.clone();
        let server = self.clone();
        let kept = self.cli.timeout() + KEPT_PAST_RUN;
        let options = TaskOptions::new()
            .with_ttl_ms(u64::try_from(kept.as_millis()).ok())
            .with_poll_interval_ms(POLL_MS)
            .with_status_message(format!("encrust is running {}", request.name));
        let task = self.tasks.spawn(options, move |task| {
            Box::pin(async move {
                tokio::select! {
                    () = task.cancelled() => {
                        stop.cancel();
                        Err(TaskExit::Cancelled)
                    }
                    answered = server.called(request, own) => match answered {
                        Ok(CallToolResponse::Complete(result)) => Ok(result),
                        Ok(_) => Err(TaskExit::Error(Failed::new(
                            ErrorCode::INTERNAL_ERROR,
                            "the call answered nothing a task holds",
                            None,
                        ))),
                        Err(error) => Err(TaskExit::Error(error)),
                    },
                }
            })
        });
        CallToolResponse::Task(CreateTaskResult::new(task))
    }

    /// Runs the tool `request` names, noting who called and how long it took.
    pub(super) async fn called(
        &self,
        request: CallToolRequestParams,
        context: Context,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let started = Instant::now();
        let tool = request.name.clone();
        let client = client_name(&context);
        let named = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("job")?.as_str().map(str::to_owned));
        let call = (self.presence).begin(client.as_deref(), &tool, named.as_deref());
        let mut response = self
            .tool_router
            .call(ToolCallContext::new(self, request, context))
            .await;
        if let Ok(CallToolResponse::Complete(result)) = &mut response
            && let Some(note) = self.window.version_note().await
        {
            result.content.push(ContentBlock::text(note));
        }
        let answered = body_of(&response)["job"].as_str().map(str::to_owned);
        self.presence.end(call, answered.or(named).as_deref());
        let took = started.elapsed();
        self.trace.record(&tool, took, &response).await;
        stamp_took(&mut response, took);
        response
    }
}

/// The client's own name for itself: from the request's `_meta`, else its handshake.
fn client_name(context: &Context) -> Option<String> {
    context
        .meta
        .client_info()
        .map(|info| info.name)
        .or_else(|| {
            context
                .peer
                .peer_info()
                .map(|info| info.client_info.name.clone())
        })
}
