use apalis::utils::AsyncStdExecutor;
use apalis_core::worker::Context;

pub(super) type WorkerCtx = Context<AsyncStdExecutor>;
