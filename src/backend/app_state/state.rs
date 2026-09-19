use crate::error::*;
use iced::{
    Subscription,
    advanced::subscription::{self, EventStream, Hasher, Recipe},
    futures::{SinkExt, stream::BoxStream},
    stream,
};
use std::{
    future::Future,
    hash::Hash,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Notify, watch},
    task::JoinHandle,
};
use tokio_stream::wrappers::WatchStream;
use tracing::{debug, info};

pub type Reader<T> = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'static>> + Send + Sync + 'static,
>;

pub type Setter<T> = Arc<dyn Fn(&T) -> Result<()> + Send + Sync + 'static>;

static NEXT_STATE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct State<T> {
    id: u64,
    sender: watch::Sender<T>,
    reader: Option<Reader<T>>,
    setter: Option<Setter<T>>,
    subscriber_notify: Arc<Notify>,
}

impl<T> State<T> {
    pub fn new(value: T) -> Self {
        let (sender, _) = watch::channel(value);

        let id = NEXT_STATE_ID.fetch_add(1, Ordering::Relaxed);

        debug!("Created state with id {}", id);

        let setter: Setter<T> = Arc::new(|_| Ok(()));

        Self {
            id,
            sender,
            reader: None,
            setter: Some(setter),
            subscriber_notify: Arc::new(Notify::new()),
        }
    }

    pub fn with_reader(mut self, reader: Reader<T>) -> Self {
        self.reader = Some(reader);
        self
    }

    pub fn with_setter<F>(mut self, setter: F) -> Self
    where
        F: Fn(&T) -> Result<()> + Send + Sync + 'static,
    {
        self.setter = Some(Arc::new(setter));
        self
    }

    pub fn without_setter(mut self) -> Self {
        self.setter = None;
        self
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.sender.borrow().clone()
    }

    pub async fn refresh(&self) -> Result<T>
    where
        T: Clone + PartialEq,
    {
        let reader = self
            .reader
            .as_ref()
            .ok_or_else(|| Error::new("state has no reader"))?;

        let value = reader().await?;

        self.publish(value.clone());

        Ok(value)
    }

    pub fn set(&self, value: T) -> Result<()>
    where
        T: PartialEq,
    {
        if *self.sender.borrow() == value {
            return Ok(());
        }

        let setter = self
            .setter
            .as_ref()
            .ok_or_else(|| Error::new("state has no setter"))?;

        setter(&value)?;

        self.publish(value);

        Ok(())
    }

    pub fn set_force(&self, value: T) -> Result<()> {
        let setter = self
            .setter
            .as_ref()
            .ok_or_else(|| Error::new("state has no setter"))?;

        setter(&value)?;

        let _ = self.sender.send_replace(value);

        Ok(())
    }

    pub fn subscribe(&self) -> watch::Receiver<T> {
        let receiver = self.sender.subscribe();

        self.subscriber_notify.notify_one();

        receiver
    }

    pub async fn changed(&self, receiver: &mut watch::Receiver<T>) -> Result<T>
    where
        T: Clone,
    {
        receiver.changed().await?;

        Ok(receiver.borrow_and_update().clone())
    }

    pub fn stream_raw(&self) -> WatchStream<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        WatchStream::new(self.subscribe())
    }

    pub fn subscription<M>(&self, map: fn(T) -> M) -> Subscription<M>
    where
        T: Clone + Send + Sync + 'static,
        M: Send + 'static,
    {
        let recipe = StateRecipe {
            id: self.id,
            receiver: self.subscribe(),
            map,
        };

        subscription::from_recipe(recipe)
    }

    pub fn spawn_poller(
        &self,
        poll_interval: Duration,
        only_with_subscribers: bool,
    ) -> Result<JoinHandle<()>>
    where
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        if poll_interval.is_zero() {
            return Err(Error::new("state poll interval must be greater than zero"));
        }

        let reader = self
            .reader
            .clone()
            .ok_or_else(|| Error::new("state has no reader"))?;

        let state = self.clone();
        let subscriber_notify = self.subscriber_notify.clone();

        info!("Spawning poller for state {}", self.id);

        Ok(tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);

            loop {
                if only_with_subscribers && state.sender.receiver_count() == 0 {
                    subscriber_notify.notified().await;
                    continue;
                }

                interval.tick().await;

                match reader().await {
                    Ok(value) => {
                        state.publish(value);
                    }

                    Err(err) => {
                        let _ = Error::new(format!("state poller error: {err}"));
                    }
                }
            }
        }))
    }

    fn publish(&self, value: T)
    where
        T: PartialEq,
    {
        self.sender.send_if_modified(|current| {
            if *current == value {
                false
            } else {
                *current = value;
                true
            }
        });
    }
}

impl<T> std::fmt::Debug for State<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("value", &self.sender.borrow())
            .field("has_reader", &self.reader.is_some())
            .field("has_setter", &self.setter.is_some())
            .finish()
    }
}

struct StateRecipe<T, M> {
    id: u64,
    receiver: watch::Receiver<T>,
    map: fn(T) -> M,
}

impl<T, M> Hash for StateRecipe<T, M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T, M> Recipe for StateRecipe<T, M>
where
    T: Clone + Send + Sync + 'static,
    M: Send + 'static,
{
    type Output = M;

    fn hash(&self, state: &mut Hasher) {
        self.id.hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Self::Output> {
        let mut receiver = self.receiver;
        let map = self.map;

        Box::pin(stream::channel(1, async move |mut output| {
            loop {
                if receiver.changed().await.is_err() {
                    let _ = Error::new("state subscription closed unexpectedly");
                    break;
                }

                let value = receiver.borrow_and_update().clone();

                if output.send(map(value)).await.is_err() {
                    let _ = Error::new("state subscription output closed unexpectedly");
                    break;
                }
            }
        }))
    }
}
