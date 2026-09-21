//! Cooperative cancellation before activation; never drop an install transaction.

use std::future::Future;

use anyhow::Result;
use tokio::sync::watch;

#[derive(Debug, thiserror::Error)]
#[error("更新已取消，当前版本未更改。")]
pub struct UpdateCancelled;

pub const CANCELLED_EXIT_CODE: i32 = 130;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preparing,
    Cancelled,
    Activating,
}

#[derive(Clone)]
pub(crate) struct Cancellation(watch::Sender<Phase>);

tokio::task_local! {
    static CANCELLATION: Cancellation;
}

impl Cancellation {
    pub(crate) fn new() -> Self {
        Self(watch::channel(Phase::Preparing).0)
    }

    /// Returns false only when activation has already won the race.
    pub(crate) fn cancel(&self) -> bool {
        self.0.send_if_modified(|phase| {
            if *phase == Phase::Preparing {
                *phase = Phase::Cancelled;
                true
            } else {
                false
            }
        });
        *self.0.borrow() == Phase::Cancelled
    }

    pub(crate) async fn scope<F: Future>(&self, operation: F) -> F::Output {
        CANCELLATION.scope(self.clone(), operation).await
    }
}

/// Poll the signal first to register the OS handler before starting the update.
/// A signal only requests cancellation: the operation performs its own cleanup.
pub async fn with_ctrl_c<F: Future>(operation: F) -> F::Output {
    let cancellation = Cancellation::new();
    cancellation
        .scope(async {
            tokio::pin!(operation);
            let signal = tokio::signal::ctrl_c();
            tokio::pin!(signal);
            loop {
                tokio::select! {
                    biased;
                    result = &mut signal => {
                        if let Err(error) = result {
                            tracing::warn!("could not listen for update cancellation: {error}");
                            return operation.await;
                        }
                        if !cancellation.cancel() {
                            eprintln!("正在完成安装，请稍候…");
                        }
                        signal.set(tokio::signal::ctrl_c());
                    }
                    result = &mut operation => return result,
                }
            }
        })
        .await
}

pub(crate) fn check() -> Result<()> {
    if CANCELLATION
        .try_with(|cancellation| *cancellation.0.borrow() == Phase::Cancelled)
        .unwrap_or(false)
    {
        return Err(UpdateCancelled.into());
    }
    Ok(())
}

async fn cancelled() {
    let Ok(mut receiver) = CANCELLATION.try_with(|cancellation| cancellation.0.subscribe()) else {
        return std::future::pending().await;
    };
    let triggered = receiver
        .wait_for(|phase| *phase == Phase::Cancelled)
        .await
        .is_ok();
    if !triggered {
        std::future::pending().await
    }
}

/// Only wrap operations safe to drop, such as an HTTP request or the next chunk.
/// File writes, extraction and activation must finish before checking again.
pub(crate) async fn interruptible<F: Future>(operation: F) -> Result<F::Output> {
    check()?;
    tokio::select! {
        biased;
        () = cancelled() => Err(UpdateCancelled.into()),
        result = operation => Ok(result),
    }
}

pub(crate) fn begin_activation() -> Result<()> {
    CANCELLATION
        .try_with(|cancellation| {
            cancellation.0.send_if_modified(|phase| {
                if *phase == Phase::Preparing {
                    *phase = Phase::Activating;
                    true
                } else {
                    false
                }
            });
        })
        .ok();
    check()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pending_network_operation_cancels_without_polling_it_again() {
        let cancellation = Cancellation::new();
        let request = async {
            assert!(cancellation.cancel());
            std::future::pending::<()>().await
        };
        let error = cancellation
            .scope(interruptible(request))
            .await
            .unwrap_err();
        assert!(error.is::<UpdateCancelled>());
        assert_eq!(error.to_string(), "更新已取消，当前版本未更改。");
    }

    #[tokio::test]
    async fn cancellation_before_activation_prevents_commit() {
        let cancellation = Cancellation::new();
        cancellation.cancel();
        cancellation
            .scope(async {
                assert!(begin_activation().unwrap_err().is::<UpdateCancelled>());
            })
            .await;
    }

    #[tokio::test]
    async fn activation_and_its_bookkeeping_finish_after_late_ctrl_c() {
        let cancellation = Cancellation::new();
        cancellation
            .scope(async {
                begin_activation().unwrap();
                assert!(!cancellation.cancel());
                check().unwrap();
                assert_eq!(interruptible(async { 42 }).await.unwrap(), 42);
            })
            .await;
    }

    #[tokio::test]
    async fn callers_without_a_signal_scope_are_unchanged() {
        assert_eq!(interruptible(async { 42 }).await.unwrap(), 42);
        begin_activation().unwrap();
    }
}
