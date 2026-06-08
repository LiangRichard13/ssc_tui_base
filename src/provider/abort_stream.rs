use crate::message::StreamEvent;
use crate::provider::EventStream;
use anyhow::Result;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;

pub(crate) fn abort_on_drop_receiver_stream(
    rx: mpsc::Receiver<Result<StreamEvent>>,
    handle: JoinHandle<()>,
) -> EventStream {
    Box::pin(AbortOnDropReceiverStream {
        inner: ReceiverStream::new(rx),
        handle: Some(handle),
    })
}

struct AbortOnDropReceiverStream {
    inner: ReceiverStream<Result<StreamEvent>>,
    handle: Option<JoinHandle<()>>,
}

impl Stream for AbortOnDropReceiverStream {
    type Item = Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for AbortOnDropReceiverStream {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take()
            && !handle.is_finished()
        {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn dropping_stream_aborts_provider_task() {
        let (tx, rx) = mpsc::channel::<Result<StreamEvent>>(1);
        let task_started = Arc::new(tokio::sync::Notify::new());
        let task_dropped = Arc::new(AtomicBool::new(false));
        let task_started_clone = Arc::clone(&task_started);
        let task_dropped_clone = Arc::clone(&task_dropped);

        let handle = tokio::spawn(async move {
            let _drop_flag = DropFlag(task_dropped_clone);
            let _tx = tx;
            task_started_clone.notify_one();
            futures::future::pending::<()>().await;
        });

        task_started.notified().await;
        let stream = abort_on_drop_receiver_stream(rx, handle);
        drop(stream);

        let observed = tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                if task_dropped.load(Ordering::SeqCst) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;

        assert!(
            observed.is_ok(),
            "dropping stream should abort provider task"
        );
    }
}
